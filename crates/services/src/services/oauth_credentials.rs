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

/// Persist credentials.
///
/// `join` and a background token refresh used to share `credentials.tmp`. The
/// winner's `rename` removes that name, and the loser's `rename`/`open` then
/// returns ENOENT (`No such file or directory (os error 2)`) with no path.
/// macOS can also drop a just-created temp file before `rename`. A missing
/// temp file is not fatal when the destination itself can be written.
fn install_credentials_file(dest: &Path, creds: &StoredCredentials) -> io::Result<()> {
    if let Some(parent) = dest.parent()
        && !parent.as_os_str().is_empty()
    {
        std::fs::create_dir_all(parent).map_err(|err| io_at("creating directory", parent, err))?;
    }

    let tmp = unique_temp_path(dest);
    let result = match write_private(&tmp, creds) {
        Ok(()) => commit_temp_file(&tmp, dest, creds),
        Err(err) if err.kind() == ErrorKind::NotFound => {
            tracing::warn!(
                path = %tmp.display(),
                error = %err,
                "credentials temp path was missing; writing the destination directly"
            );
            write_private(dest, creds)
        }
        Err(err) => Err(err),
    };
    // Rename consumes the temp name. A failed or fallback write must not leave it behind.
    let _ = std::fs::remove_file(&tmp);
    result
}

fn commit_temp_file(tmp: &Path, dest: &Path, creds: &StoredCredentials) -> io::Result<()> {
    match std::fs::rename(tmp, dest) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == ErrorKind::NotFound => {
            tracing::warn!(
                tmp = %tmp.display(),
                dest = %dest.display(),
                error = %err,
                "credentials temp file disappeared before rename; writing the destination directly"
            );
            write_private(dest, creds).map_err(|write_err| {
                io::Error::new(
                    write_err.kind(),
                    format!(
                        "renaming {} to {} failed ({err}); direct write failed: {write_err}",
                        tmp.display(),
                        dest.display()
                    ),
                )
            })
        }
        Err(err) => Err(io_rename(tmp, dest, err)),
    }
}

fn write_private(path: &Path, creds: &StoredCredentials) -> io::Result<()> {
    let mut opts = std::fs::OpenOptions::new();
    opts.create(true).write(true).truncate(true);
    #[cfg(unix)]
    {
        opts.mode(0o600);
    }
    let mut file = opts.open(path).map_err(|err| io_at("opening", path, err))?;
    serde_json::to_writer_pretty(&mut file, creds)
        .map_err(|err| io_at("writing", path, io::Error::from(err)))?;
    if let Err(err) = file.sync_all() {
        // The bytes are already in the file. ENOENT from sync (seen when the
        // path disappears under a macOS write) should not discard them.
        if err.kind() != ErrorKind::NotFound {
            return Err(io_at("syncing", path, err));
        }
        tracing::warn!(
            path = %path.display(),
            error = %err,
            "syncing credentials reported a missing file; continuing with the written data"
        );
    }
    drop(file);
    restrict_permissions(path)?;
    Ok(())
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
    fn missing_temp_file_falls_back_to_a_direct_write() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("credentials.json");
        let missing_tmp = dir.path().join("already-renamed.partial");
        let stored = StoredCredentials {
            refresh_token: "from-memory".to_string(),
        };
        commit_temp_file(&missing_tmp, &dest, &stored).unwrap();
        let body = std::fs::read_to_string(&dest).unwrap();
        assert!(body.contains("from-memory"));
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
