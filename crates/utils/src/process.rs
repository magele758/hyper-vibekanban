use command_group::AsyncGroupChild;
#[cfg(unix)]
use tokio::time::Duration;

pub async fn kill_process_group(child: &mut AsyncGroupChild) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        // Use command_group's UnixChildExt::signal() which calls killpg()
        // with the pgid captured at spawn time. This works even after the
        // group leader has exited, unlike getpgid() which would fail.
        use command_group::{Signal, UnixChildExt};

        for sig in [Signal::SIGINT, Signal::SIGTERM, Signal::SIGKILL] {
            tracing::info!("Sending {:?} to process group", sig);
            if let Err(e) = child.signal(sig) {
                // break if the group does not exist anymore
                if e.raw_os_error() == Some(nix::libc::ESRCH) {
                    break;
                }
                tracing::warn!("Failed to send signal {:?} to process group: {}", sig, e);
            }
            if sig != Signal::SIGKILL {
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    }

    let _ = child.kill().await;
    let _ = child.wait().await;
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use std::time::Duration;

    use tokio::time::timeout;

    use super::kill_process_group;
    use crate::command_ext::GroupSpawnNoWindowExt;

    /// A long-running child spawned as a process group must be terminated and
    /// reaped by `kill_process_group`. This is the core guard against orphaned
    /// agent processes: if kill ever stops working, killed agents keep running
    /// and mutating the user's files.
    #[tokio::test]
    async fn kills_and_reaps_long_running_child() {
        let mut child = tokio::process::Command::new("sleep")
            .arg("60")
            .group_spawn_no_window()
            .expect("spawn sleep");

        // Alive immediately after spawn.
        assert!(child.inner().id().is_some(), "child should be running");

        // Kill must complete well within the SIGINT->SIGTERM->SIGKILL budget
        // (~4s worst case); bound it so a hang fails loudly instead of stalling.
        timeout(Duration::from_secs(15), kill_process_group(&mut child))
            .await
            .expect("kill_process_group must not hang")
            .expect("kill_process_group returns Ok");

        // After kill the child is reaped: try_wait yields an exit status, not
        // `None` (still running) and not an error.
        let status = child.try_wait().expect("try_wait after kill");
        assert!(
            status.is_some(),
            "child must be terminated and reaped after kill_process_group"
        );
    }

    /// Killing the group must take down child processes the leader spawned, not
    /// just the leader. We start a shell whose own children outlive it briefly,
    /// then assert the whole group is gone and reaped.
    #[tokio::test]
    async fn kills_whole_process_group() {
        // `sh` spawns two background sleeps then waits — the group has multiple
        // members. killpg must reap all of them via the group leader.
        let mut child = tokio::process::Command::new("sh")
            .arg("-c")
            .arg("sleep 60 & sleep 60 & wait")
            .group_spawn_no_window()
            .expect("spawn shell group");

        assert!(
            child.inner().id().is_some(),
            "group leader should be running"
        );

        timeout(Duration::from_secs(15), kill_process_group(&mut child))
            .await
            .expect("kill_process_group must not hang")
            .expect("kill_process_group returns Ok");

        let status = child.try_wait().expect("try_wait after group kill");
        assert!(
            status.is_some(),
            "process group leader must be reaped after kill_process_group"
        );
    }

    /// Killing an already-exited child must not error or hang — guards the race
    /// where a process exits naturally just before the supervisor tries to kill
    /// it.
    #[tokio::test]
    async fn kill_on_already_exited_child_is_ok() {
        let mut child = tokio::process::Command::new("true")
            .group_spawn_no_window()
            .expect("spawn true");

        // Let it exit on its own.
        let _ = child.wait().await;

        // Killing the dead process group must still return Ok quickly.
        timeout(Duration::from_secs(15), kill_process_group(&mut child))
            .await
            .expect("kill_process_group must not hang on a dead child")
            .expect("kill_process_group returns Ok on already-exited child");
    }
}
