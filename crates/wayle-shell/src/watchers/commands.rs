//! Shared shell-command execution watchers for shell-backed UI surfaces.
//!
//! Custom bar modules ([`crate::shell::bar::modules::custom`]) and custom
//! dropdown panels ([`crate::shell::bar::dropdowns::custom`]) both run
//! user-configured shell commands and feed their output into Relm4
//! components. This module owns that plumbing once: the 30-second one-shot
//! runner, the ticker that drives poll mode, and the restart-aware
//! supervisor for watch mode.

use std::{
    process::{ExitStatus, Stdio},
    time::Duration,
};

use relm4::{Component, ComponentSender};
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::Command,
    time::{Instant, MissedTickBehavior, interval_at, timeout},
};
use tokio_stream::wrappers::IntervalStream;
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};
use wayle_config::schemas::modules::RestartPolicy;
use wayle_widgets::watch_cancellable;

const COMMAND_TIMEOUT: Duration = Duration::from_secs(30);

/// Outcome of a one-shot command attempt.
pub(crate) enum ExecOutcome {
    /// Command produced stdout (trimmed).
    Output(String),
    /// Command was cancelled before completing (config change or shutdown).
    Cancelled,
    /// Command exceeded the 30-second timeout.
    TimedOut,
    /// Command could not be spawned.
    Failed(std::io::Error),
}

async fn run_sh_command(command: &str) -> Result<String, std::io::Error> {
    let output = Command::new("sh")
        .arg("-c")
        .arg(command)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .output()
        .await?;

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Runs a one-shot command with timeout and cancellation, mapping the
/// outcome into the component's command-output stream.
///
/// The mapping closure receives the surface identifier (for log context)
/// and the outcome, and returns the message to send. Reset `cancel_token`
/// before calling to cancel any in-flight command.
pub(crate) fn run_command_oneshot<Comp>(
    sender: &ComponentSender<Comp>,
    surface_id: &str,
    command: String,
    cancel_token: CancellationToken,
    map_outcome: impl FnOnce(&str, ExecOutcome) -> Comp::CommandOutput + Send + Sync + 'static,
) where
    Comp: Component,
    Comp::CommandOutput: Send + 'static,
{
    let surface_id = surface_id.to_owned();

    sender.oneshot_command(async move {
        let outcome = tokio::select! {
            biased;
            () = cancel_token.cancelled() => ExecOutcome::Cancelled,
            result = timeout(COMMAND_TIMEOUT, run_sh_command(&command)) => match result {
                Ok(Ok(output)) => ExecOutcome::Output(output),
                Ok(Err(error)) => ExecOutcome::Failed(error),
                Err(_) => ExecOutcome::TimedOut,
            },
        };

        map_outcome(&surface_id, outcome)
    });
}

/// Common handling for a one-shot outcome when output is the only payload
/// the surface needs: returns trimmed stdout, logs and swallows the rest.
pub(crate) fn report_outcome(surface_id: &str, outcome: ExecOutcome) -> Option<String> {
    match outcome {
        ExecOutcome::Output(output) => Some(output),
        ExecOutcome::Cancelled => None,
        ExecOutcome::TimedOut => {
            warn!(
                surface_id = %surface_id,
                timeout_secs = COMMAND_TIMEOUT.as_secs(),
                "command timed out"
            );
            None
        }
        ExecOutcome::Failed(error) => {
            warn!(surface_id = %surface_id, error = %error, "command execution failed");
            None
        }
    }
}

/// Spawns a ticker that maps every tick into a command-output message.
///
/// Used by poll mode: the tick itself is cheap; the owning component reacts
/// by running its command (which keeps single-flight cancellation intact).
/// An `interval_ms` of `0` produces no messages.
pub(crate) fn spawn_poller<Comp>(
    sender: &ComponentSender<Comp>,
    interval_ms: u64,
    cancel_token: CancellationToken,
    map_tick: impl Fn() -> Comp::CommandOutput + Send + Sync + 'static,
) where
    Comp: Component,
    Comp::CommandOutput: Send + 'static,
{
    if interval_ms == 0 {
        return;
    }

    let interval = Duration::from_millis(interval_ms);
    let start = Instant::now() + interval;
    let mut tick = interval_at(start, interval);
    tick.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let interval_stream = IntervalStream::new(tick);

    watch_cancellable!(sender, cancel_token, [interval_stream], |out| {
        let _ = out.send(map_tick());
    });
}

/// Spawns a long-running child process and restarts it according to the
/// configured [`RestartPolicy`].
///
/// On each exit the policy is checked: `Never` stops unconditionally,
/// `OnFailure` stops on clean exits, and `OnExit` always restarts.
/// Before restarting, the loop waits `restart_delay_ms`.
///
/// Every stdout line produces a full payload (e.g., one complete panel
/// description per line in watch mode), mapped into a command-output
/// message via `map_line`.
///
/// The supervisor exits when `stop_token` is cancelled, the component
/// shuts down, or the restart policy denies a restart.
pub(crate) fn spawn_watch_supervisor<Comp>(
    sender: &ComponentSender<Comp>,
    surface_id: String,
    command: String,
    restart_policy: RestartPolicy,
    restart_delay_ms: u64,
    stop_token: CancellationToken,
    map_line: impl Fn(&str, String) -> Comp::CommandOutput + Send + Sync + 'static,
) where
    Comp: Component,
    Comp::CommandOutput: Send + 'static,
{
    sender.command(move |out, shutdown| async move {
        loop {
            if stop_token.is_cancelled() {
                debug!(
                    surface_id = %surface_id,
                    "watch command stopped (watcher cancelled)"
                );
                return;
            }

            let run_outcome = run_child(
                &command,
                &surface_id,
                &out,
                &shutdown,
                &stop_token,
                &map_line,
            )
            .await;

            let RunOutcome::Exited(exit_status) = run_outcome else {
                return;
            };

            debug!(
                surface_id = %surface_id,
                exit_code = ?exit_status.code(),
                "watch command exited"
            );

            if !should_restart(restart_policy, exit_status) {
                debug!(
                    surface_id = %surface_id,
                    policy = ?restart_policy,
                    "watch command will not restart due to restart-policy"
                );
                return;
            }

            debug!(
                surface_id = %surface_id,
                policy = ?restart_policy,
                restart_delay_ms,
                "restarting watch command after exit"
            );

            let delay_expired = tokio::select! {
                () = shutdown.clone().wait() => false,
                () = stop_token.cancelled() => false,
                () = tokio::time::sleep(Duration::from_millis(restart_delay_ms)) => true,
            };

            if !delay_expired {
                return;
            }
        }
    });
}

enum RunOutcome {
    Stop,
    Exited(ExitStatus),
}

#[allow(clippy::cognitive_complexity)]
async fn run_child<LineOut>(
    command: &str,
    surface_id: &str,
    out: &relm4::Sender<LineOut>,
    shutdown: &relm4::ShutdownReceiver,
    stop_token: &CancellationToken,
    map_line: &impl Fn(&str, String) -> LineOut,
) -> RunOutcome
where
    LineOut: Send + 'static,
{
    let mut child = match spawn_child(command) {
        Ok(child) => child,
        Err(error) => {
            warn!(surface_id = %surface_id, error = %error, "failed to spawn watch command");
            return RunOutcome::Stop;
        }
    };

    debug!(surface_id = %surface_id, "watch command started");

    let Some(stdout) = child.stdout.take() else {
        warn!(surface_id = %surface_id, "watch command started without stdout");
        terminate_child(&mut child).await;
        return RunOutcome::Stop;
    };

    let stopped =
        read_stdout_until_stop(stdout, surface_id, out, shutdown, stop_token, map_line).await;

    if stopped {
        terminate_child(&mut child).await;
        return RunOutcome::Stop;
    }

    match child.wait().await {
        Ok(status) => RunOutcome::Exited(status),
        Err(error) => {
            warn!(
                surface_id = %surface_id,
                error = %error,
                "failed waiting for watch command exit"
            );
            RunOutcome::Stop
        }
    }
}

fn spawn_child(command: &str) -> Result<tokio::process::Child, std::io::Error> {
    Command::new("sh")
        .arg("-c")
        .arg(command)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
}

async fn read_stdout_until_stop<LineOut: 'static + Send>(
    stdout: tokio::process::ChildStdout,
    surface_id: &str,
    out: &relm4::Sender<LineOut>,
    shutdown: &relm4::ShutdownReceiver,
    stop_token: &CancellationToken,
    map_line: &impl Fn(&str, String) -> LineOut,
) -> bool {
    let mut reader = BufReader::new(stdout).lines();

    tokio::select! {
        () = shutdown.clone().wait() => {
            debug!(surface_id = %surface_id, "watch command stopped (shutdown)");
            true
        }
        () = stop_token.cancelled() => {
            debug!(surface_id = %surface_id, "watch command stopped (watcher cancelled)");
            true
        }
        () = async {
            while let Ok(Some(line)) = reader.next_line().await {
                let _ = out.send(map_line(surface_id, line));
            }
        } => {
            false
        }
    }
}

fn should_restart(policy: RestartPolicy, exit_status: ExitStatus) -> bool {
    match policy {
        RestartPolicy::Never => false,
        RestartPolicy::OnExit => true,
        RestartPolicy::OnFailure => !exit_status.success(),
    }
}

async fn terminate_child(child: &mut tokio::process::Child) {
    let _ = child.kill().await;
    let _ = child.wait().await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_restart_never_returns_false() {
        let status = exit_status(1);
        assert!(!should_restart(RestartPolicy::Never, status));
    }

    #[test]
    fn should_restart_on_exit_returns_true_for_success_and_failure() {
        assert!(should_restart(RestartPolicy::OnExit, exit_status(0)));
        assert!(should_restart(RestartPolicy::OnExit, exit_status(1)));
    }

    #[test]
    fn should_restart_on_failure_returns_true_only_for_failures() {
        assert!(!should_restart(RestartPolicy::OnFailure, exit_status(0)));
        assert!(should_restart(RestartPolicy::OnFailure, exit_status(1)));
    }

    fn exit_status(code: i32) -> ExitStatus {
        std::process::Command::new("sh")
            .arg("-c")
            .arg(format!("exit {code}"))
            .status()
            .expect("failed to run shell to produce exit status")
    }
}
