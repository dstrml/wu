pub use std::process::Stdio;
use std::time::Duration;
pub use tokio::process::{Child, ChildStdin, ChildStdout, Command};

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// Wu has no console on Windows, so without this every agent process opens its own console window.
#[cfg(windows)]
pub(crate) fn command(program: impl AsRef<std::ffi::OsStr>) -> Command {
    let mut command = Command::new(program);
    command.creation_flags(CREATE_NO_WINDOW);
    command
}

#[cfg(not(windows))]
pub(crate) fn command(program: impl AsRef<std::ffi::OsStr>) -> Command {
    Command::new(program)
}

#[cfg(unix)]
pub(crate) fn signal_target(child: &Child) -> Option<i32> {
    let pid = child.id()? as i32;
    // SAFETY: getpgid only inspects the owned, unreaped child.
    Some(if unsafe { libc::getpgid(pid) } == pid {
        -pid
    } else {
        pid
    })
}

/// Abort the returned task once the child is reaped.
pub(crate) fn escalate_interrupt(
    child: &Child,
    interrupt_grace: Duration,
    kill_grace: Duration,
) -> Option<tokio::task::JoinHandle<()>> {
    #[cfg(unix)]
    {
        use crate::{Signal, send_signal};
        let pid = signal_target(child)?;
        Some(tokio::spawn(async move {
            tokio::time::sleep(interrupt_grace).await;
            send_signal(&pid, Signal::Term);
            tokio::time::sleep(kill_grace).await;
            send_signal(&pid, Signal::Kill);
        }))
    }
    #[cfg(not(unix))]
    {
        let pid = child.id()?;
        Some(tokio::spawn(async move {
            tokio::time::sleep(interrupt_grace + kill_grace).await;
            let mut taskkill = command("taskkill");
            taskkill
                .args(["/F", "/T", "/PID", &pid.to_string()])
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            if let Err(error) = taskkill.status().await {
                log::warn!("failed to stop agent process {pid}: {error}");
            }
        }))
    }
}
