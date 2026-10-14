#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::{
    ffi::OsString,
    io::{self, ErrorKind},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use chrono::{DateTime, Duration as ChronoDuration, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

/// OAuth credentials containing the JWT tokens issued by the remote OAuth service.
/// The `access_token` is short-lived; `refresh_token` allows minting a new pair.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Credentials {
    pub access_token: Option<String>,
    pub refresh_token: String,
    pub expires_at: Option<DateTime<Utc>>,
}

impl Credentials {
    pub fn expires_soon(&self, leeway: ChronoDuration) -> bool {
        match (self.access_token.as_ref(), self.expires_at.as_ref()) {
            (Some(_), Some(exp)) => Utc::now() + leeway >= *exp,
            _ => true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredCredentials {
    refresh_token: String,
}

impl From<StoredCredentials> for Credentials {
    fn from(value: StoredCredentials) -> Self {
        Self {
            access_token: None,
            refresh_token: value.refresh_token,
            expires_at: None,
        }
    }
}

/// Service for managing OAuth credentials (JWT tokens) in memory and persistent storage.
/// The token is loaded into memory on startup and persisted to disk on save.
pub struct OAuthCredentials {
    path: PathBuf,
    inner: RwLock<Option<Credentials>>,
}

impl OAuthCredentials {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            inner: RwLock::new(None),
        }
    }

    pub async fn load(&self) -> io::Result<()> {
        let mut guard = self.inner.write().await;
        let creds = self.load_from_file()?.map(Credentials::from);
        *guard = creds;
        Ok(())
    }

    pub async fn save(&self, creds: &Credentials) -> io::Result<()> {
        let stored = StoredCredentials {
            refresh_token: creds.refresh_token.clone(),
        };
        // Join login and the agent-task watcher both persist tokens at startup.
        // Serialize so they cannot share one temp file.
        let mut guard = self.inner.write().await;
        self.save_to_file(&stored)?;
        *guard = Some(creds.clone());
        Ok(())
    }

    pub async fn clear(&self) -> io::Result<()> {
        let _ = std::fs::remove_file(&self.path);
        *self.inner.write().await = None;
        Ok(())
    }

    pub async fn get(&self) -> Option<Credentials> {
        self.inner.read().await.clone()
    }

    fn load_from_file(&self) -> io::Result<Option<StoredCredentials>> {
        let bytes = match std::fs::read(&self.path) {
            Ok(bytes) => bytes,
            // Missing credentials are optional. `exists()` then `read()` races, and on
            // macOS a path can vanish between the two calls with ENOENT.
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(None),
            Err(err) => return Err(io_at("reading", &self.path, err)),
        };
        match serde_json::from_slice::<StoredCredentials>(&bytes) {
            Ok(creds) => Ok(Some(creds)),
            Err(err) => {
                tracing::warn!(
                    ?err,
                    path = %self.path.display(),
                    "failed to parse credentials file, renaming to .bad"
                );
                let bad = self.path.with_extension("bad");
                if let Err(rename_err) = std::fs::rename(&self.path, &bad)
                    && rename_err.kind() != ErrorKind::NotFound
                {
                    tracing::warn!(
                        error = %rename_err,
                        path = %self.path.display(),
                        "failed to rename unreadable credentials file"
                    );
                }
                Ok(None)
            }
        }
    }

    fn save_to_file(&self, creds: &StoredCredentials) -> io::Result<()> {
        install_credentials_file(&self.path, creds)
    }
}

/// How many distinct temp files to try before giving up.
/// A vanished temp is retried once with a new name. The destination is only
/// replaced by `rename`, so a failed attempt cannot truncate `credentials.json`.
const TEMP_REPLACE_ATTEMPTS: u32 = 2;

/// Persist credentials.
///
/// `join` and a background token refresh used to share `credentials.tmp`. The
/// winner's `rename` removes that name, and the loser's `rename`/`open` then
/// returns ENOENT (`No such file or directory (os error 2)`) with no path.
/// macOS can also drop a just-created temp file before `rename`. Each attempt
/// uses a new unique temp file and publishes it with `rename`.
fn install_credentials_file(dest: &Path, creds: &StoredCredentials) -> io::Result<()> {
    if let Some(parent) = dest.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|err| io_at("creating directory", parent, err))?;
    }

    replace_via_temp_files(dest, |tmp| write_private(tmp, creds))
}

/// Write `dest` by creating a sibling temp file and renaming it into place.
/// `write_tmp` must only create `tmp`. A missing temp is retried with a new
/// path. `ErrorKind::NotFound` from any other step (including `sync_all`) is
/// returned as-is so the existing destination stays untouched.
fn replace_via_temp_files(
    dest: &Path,
    mut write_tmp: impl FnMut(&Path) -> Result<(), PersistError>,
) -> io::Result<()> {
    let mut temps = Vec::with_capacity(TEMP_REPLACE_ATTEMPTS as usize);
    let mut outcome =
        ReplaceOutcome::Missing(io::Error::new(ErrorKind::NotFound, "temp file was missing"));
    for _ in 0..TEMP_REPLACE_ATTEMPTS {
        let tmp = unique_temp_path(dest);
        temps.push(tmp.clone());
        outcome = match write_tmp(&tmp) {
            Ok(()) => match std::fs::rename(&tmp, dest) {
                Ok(()) => ReplaceOutcome::Done,
                Err(err) if err.kind() == ErrorKind::NotFound => {
                    tracing::warn!(
                        tmp = %tmp.display(),
                        dest = %dest.display(),
                        error = %err,
                        "credentials temp file disappeared before rename; retrying with a new temp file"
                    );
                    ReplaceOutcome::Missing(io_rename(&tmp, dest, err))
                }
                Err(err) => ReplaceOutcome::Fatal(io_rename(&tmp, dest, err)),
            },
            Err(PersistError::Missing(err)) => {
                tracing::warn!(
                    path = %tmp.display(),
                    error = %err,
                    "credentials temp path was missing; retrying with a new temp file"
                );
                ReplaceOutcome::Missing(err)
            }
            Err(PersistError::Other(err)) => ReplaceOutcome::Fatal(err),
        };
        if !matches!(outcome, ReplaceOutcome::Missing(_)) {
            break;
        }
    }
    // `rename` consumes the successful temp name. Drop any attempt that did not publish.
    for tmp in &temps {
        let _ = std::fs::remove_file(tmp);
    }
    match outcome {
        ReplaceOutcome::Done => Ok(()),
        ReplaceOutcome::Fatal(err) => Err(err),
        ReplaceOutcome::Missing(err) => Err(io::Error::new(
            ErrorKind::NotFound,
            format!(
                "replacing {} failed after {TEMP_REPLACE_ATTEMPTS} missing temp files: {err}",
                dest.display()
            ),
        )),
    }
}

enum ReplaceOutcome {
    Done,
    Fatal(io::Error),
    Missing(io::Error),
}

enum PersistError {
    /// `open` could not find the temp path. Safe to try a different name.
    Missing(io::Error),
    /// Any other failure, including `sync_all` returning ENOENT.
    Other(io::Error),
}

fn write_private(path: &Path, creds: &StoredCredentials) -> Result<(), PersistError> {
    let mut opts = std::fs::OpenOptions::new();
    opts.create(true).write(true).truncate(true);
    #[cfg(unix)]
    {
        opts.mode(0o600);
    }
    let mut file = match opts.open(path) {
        Ok(file) => file,
        Err(err) if err.kind() == ErrorKind::NotFound => {
            return Err(PersistError::Missing(io_at("opening", path, err)));
        }
        Err(err) => return Err(PersistError::Other(io_at("opening", path, err))),
    };
    if let Err(err) = serde_json::to_writer_pretty(&mut file, creds) {
        return Err(PersistError::Other(io_at(
            "writing",
            path,
            io::Error::from(err),
        )));
    }
    if let Err(err) = file.sync_all() {
        // Only the macOS case is ignored: sync reports ENOENT, but the file
        // we just wrote is still there. A missing file, or ENOENT anywhere
        // else, fails the attempt and must not be renamed into place.
        if sync_not_found_is_macos_file_still_present(path, &err) {
            tracing::warn!(
                path = %path.display(),
                error = %err,
                "macOS sync reported a missing credentials temp file that is still present"
            );
        } else {
            return Err(PersistError::Other(io_at("syncing", path, err)));
        }
    }
    drop(file);
    restrict_permissions(path).map_err(PersistError::Other)
}

/// macOS can return ENOENT from `sync_all` after a successful write while the
/// file is still on disk. That is the only case where the error is ignored.
fn sync_not_found_is_macos_file_still_present(path: &Path, err: &io::Error) -> bool {
    ignore_sync_not_found(
        cfg!(target_os = "macos"),
        err.kind(),
        std::fs::metadata(path).is_ok_and(|meta| meta.is_file()),
    )
}

fn ignore_sync_not_found(is_macos: bool, kind: ErrorKind, file_still_present: bool) -> bool {
    is_macos && kind == ErrorKind::NotFound && file_still_present
}

fn restrict_permissions(path: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
            .map_err(|err| io_at("setting permissions on", path, err))?;
    }
    #[cfg(not(unix))]
    {
        let _ = path;
    }
    Ok(())
}

fn unique_temp_path(dest: &Path) -> PathBuf {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let tick = COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let mut name = OsString::from(".");
    name.push(dest.file_name().unwrap_or_default());
    name.push(format!(".{nanos}.{tick}.partial"));
    match dest.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.join(name),
        _ => PathBuf::from(name),
    }
}

fn io_at(action: &str, path: &Path, err: io::Error) -> io::Error {
    io::Error::new(err.kind(), format!("{action} {}: {err}", path.display()))
}

fn io_rename(from: &Path, to: &Path, err: io::Error) -> io::Error {
    io::Error::new(
        err.kind(),
        format!("renaming {} to {}: {err}", from.display(), to.display()),
    )
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use std::sync::Arc;

    use super::*;

    fn sample(token: &str) -> Credentials {
        Credentials {
            access_token: Some("access".to_string()),
            refresh_token: token.to_string(),
            expires_at: None,
        }
    }

    #[tokio::test]
    async fn missing_credentials_file_is_absent() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("credentials.json");
        let store = OAuthCredentials::new(path);
        store.load().await.unwrap();
        assert!(store.get().await.is_none());
    }

    #[tokio::test]
    async fn save_creates_missing_parent_and_roundtrips() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("credentials.json");
        let store = OAuthCredentials::new(path.clone());
        store.save(&sample("refresh-1")).await.unwrap();

        let loaded = OAuthCredentials::new(path);
        loaded.load().await.unwrap();
        let creds = loaded.get().await.unwrap();
        assert_eq!(creds.refresh_token, "refresh-1");
        assert!(creds.access_token.is_none());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn saved_credentials_are_owner_readable_only() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("credentials.json");
        let store = OAuthCredentials::new(path.clone());
        store.save(&sample("refresh-1")).await.unwrap();
        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "mode was {mode:o}");
    }

    #[tokio::test]
    async fn storage_error_includes_the_failing_path() {
        let dir = tempfile::tempdir().unwrap();
        let blocked = dir.path().join("not-a-directory");
        std::fs::write(&blocked, b"file").unwrap();
        let path = blocked.join("credentials.json");
        let store = OAuthCredentials::new(path);
        let err = store.save(&sample("refresh-1")).await.unwrap_err();
        let message = err.to_string();
        assert!(
            message.contains(blocked.to_str().unwrap()),
            "error should name the failing path, got {message}"
        );
        assert!(
            message.contains("os error")
                || message.contains("Not a directory")
                || message.contains("File exists")
                || message.contains("Is a directory"),
            "error should keep the OS failure, got {message}"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn broken_shared_temp_symlink_does_not_fail_save() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("credentials.json");
        let trap = dir.path().join("credentials.tmp");
        std::os::unix::fs::symlink("no-such-temp-target", &trap).unwrap();
        let store = OAuthCredentials::new(dest.clone());
        store
            .save(&sample("refresh-1"))
            .await
            .expect("dangling credentials.tmp must not fail login persistence");
        assert!(dest.is_file());
        assert!(trap.symlink_metadata().unwrap().file_type().is_symlink());
    }

    #[test]
    fn vanished_temp_is_retried_with_a_new_file_then_renamed() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("credentials.json");
        let original = b"{\"refresh_token\":\"keep-me\"}\n";
        std::fs::write(&dest, original).unwrap();
        let mut attempts = Vec::new();
        let mut saw_original = Vec::new();

        replace_via_temp_files(&dest, |tmp| {
            saw_original.push(std::fs::read(&dest).unwrap());
            attempts.push(tmp.to_path_buf());
            if attempts.len() == 1 {
                return Err(PersistError::Missing(io::Error::new(
                    ErrorKind::NotFound,
                    format!("opening {} failed", tmp.display()),
                )));
            }
            std::fs::write(tmp, b"{\"refresh_token\":\"from-memory\"}\n").unwrap();
            Ok(())
        })
        .unwrap();

        assert_eq!(attempts.len(), 2);
        assert_ne!(attempts[0], attempts[1]);
        assert!(
            saw_original.iter().all(|bytes| bytes == original),
            "destination was rewritten before the successful rename"
        );
        let body = std::fs::read_to_string(&dest).unwrap();
        let stored: StoredCredentials = serde_json::from_str(&body).unwrap();
        assert_eq!(stored.refresh_token, "from-memory");
    }

    #[test]
    fn rename_of_a_missing_temp_retries_without_truncating_destination() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("credentials.json");
        let original = b"{\"refresh_token\":\"keep-me\"}\n";
        std::fs::write(&dest, original).unwrap();
        let mut calls = 0;

        replace_via_temp_files(&dest, |tmp| {
            calls += 1;
            std::fs::write(tmp, b"{\"refresh_token\":\"next\"}\n").unwrap();
            if calls == 1 {
                std::fs::remove_file(tmp).unwrap();
            }
            Ok(())
        })
        .unwrap();

        assert_eq!(calls, 2);
        assert_eq!(
            std::fs::read(&dest).unwrap(),
            b"{\"refresh_token\":\"next\"}\n"
        );
    }

    #[test]
    fn exhausted_missing_temps_leave_existing_credentials_intact() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("credentials.json");
        let original = b"{\"refresh_token\":\"keep-me\"}\n";
        std::fs::write(&dest, original).unwrap();

        let err = replace_via_temp_files(&dest, |_tmp| {
            Err(PersistError::Missing(io::Error::new(
                ErrorKind::NotFound,
                "opening temp failed: No such file or directory (os error 2)",
            )))
        })
        .unwrap_err();

        let message = err.to_string();
        assert!(
            message.contains(dest.to_str().unwrap()),
            "error should name the destination, got {message}"
        );
        assert_eq!(std::fs::read(&dest).unwrap(), original);
        let mut names: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        assert_eq!(names, vec!["credentials.json".to_string()]);
    }

    #[test]
    fn sync_failure_does_not_retry_or_replace_destination() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("credentials.json");
        let original = b"{\"refresh_token\":\"keep-me\"}\n";
        std::fs::write(&dest, original).unwrap();
        let mut calls = 0;

        let err = replace_via_temp_files(&dest, |_tmp| {
            calls += 1;
            Err(PersistError::Other(io::Error::new(
                ErrorKind::NotFound,
                "syncing credentials temp: No such file or directory (os error 2)",
            )))
        })
        .unwrap_err();

        assert_eq!(
            calls, 1,
            "sync ENOENT must not be treated as a missing temp"
        );
        assert!(err.to_string().contains("syncing"));
        assert_eq!(std::fs::read(&dest).unwrap(), original);
    }

    #[test]
    fn sync_not_found_is_ignored_only_when_macos_file_remains() {
        assert!(ignore_sync_not_found(true, ErrorKind::NotFound, true));
        assert!(
            !ignore_sync_not_found(false, ErrorKind::NotFound, true),
            "ENOENT from sync is fatal off macOS even if the file is still there"
        );
        assert!(
            !ignore_sync_not_found(true, ErrorKind::NotFound, false),
            "macOS ENOENT is fatal when the written file is actually gone"
        );
        assert!(!ignore_sync_not_found(
            true,
            ErrorKind::PermissionDenied,
            true
        ));
    }

    #[tokio::test]
    async fn concurrent_saves_all_succeed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("credentials.json");
        let store = Arc::new(OAuthCredentials::new(path.clone()));
        let mut tasks = Vec::new();
        for index in 0..8 {
            let store = store.clone();
            tasks.push(tokio::spawn(async move {
                store
                    .save(&sample(&format!("token-{index}")))
                    .await
                    .unwrap();
            }));
        }
        for task in tasks {
            task.await.unwrap();
        }
        let loaded = OAuthCredentials::new(path);
        loaded.load().await.unwrap();
        assert!(
            loaded
                .get()
                .await
                .unwrap()
                .refresh_token
                .starts_with("token-")
        );
    }
}
