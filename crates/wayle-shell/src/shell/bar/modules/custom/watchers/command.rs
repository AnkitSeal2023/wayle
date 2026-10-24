//! Command execution for custom bar modules.
//!
//! Wraps the shared plumbing in [`crate::watchers::commands`], mapping the
//! outcomes into [`CustomCmd`] messages.

use relm4::ComponentSender;
use tokio_util::sync::CancellationToken;
use wayle_config::schemas::modules::CustomModuleDefinition;

use super::super::{CustomModule, messages::CustomCmd};
use crate::watchers::{ExecOutcome, report_outcome, run_command_oneshot, spawn_watch_supervisor};

/// Runs the definition's command once, feeding [`CustomCmd::CommandOutput`].
pub(crate) fn run_definition_command(
    sender: &ComponentSender<CustomModule>,
    definition: &CustomModuleDefinition,
    cancel_token: CancellationToken,
) {
    let Some(command) = definition.command.clone() else {
        return;
    };

    run_command_async(sender, &definition.id, command, cancel_token);
}

/// Runs a command asynchronously with timeout and single-flight cancellation.
///
/// If `cancel_token` is triggered, the command is cancelled.
/// Reset the token before calling to cancel any in-flight command.
pub(crate) fn run_command_async(
    sender: &ComponentSender<CustomModule>,
    module_id: &str,
    command: String,
    cancel_token: CancellationToken,
) {
    run_command_oneshot(sender, module_id, command, cancel_token, map_exec_outcome);
}

/// Spawns the definition's long-running watch command under the shared
/// restart-aware supervisor ([`spawn_watch_supervisor`]).
///
/// On each exit the policy is checked: `Never` stops unconditionally,
/// `OnFailure` stops on clean exits, and `OnExit` always restarts.
/// Before restarting, the loop waits the user-configured
/// `restart-interval-ms`. Each stdout line produces one
/// [`CustomCmd::WatchOutput`] payload.
pub(crate) fn run_command_watcher(
    sender: &ComponentSender<CustomModule>,
    definition: &CustomModuleDefinition,
    token: CancellationToken,
) {
    let Some(command) = definition.command.clone() else {
        return;
    };

    spawn_watch_supervisor(
        sender,
        definition.id.clone(),
        command,
        definition.restart_policy,
        definition.restart_interval_ms.value(),
        token,
        |_surface_id, line| CustomCmd::WatchOutput(line),
    );
}

fn map_exec_outcome(module_id: &str, outcome: ExecOutcome) -> CustomCmd {
    match report_outcome(module_id, outcome) {
        Some(output) => CustomCmd::CommandOutput(output),
        None => CustomCmd::CommandCancelled,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn map_exec_outcome_output() {
        let cmd = map_exec_outcome("test", ExecOutcome::Output(String::from("ok")));
        assert!(matches!(cmd, CustomCmd::CommandOutput(output) if output == "ok"));
    }

    #[test]
    fn map_exec_outcome_cancelled() {
        let cmd = map_exec_outcome("test", ExecOutcome::Cancelled);
        assert!(matches!(cmd, CustomCmd::CommandCancelled));
    }

    #[test]
    fn map_exec_outcome_timeout() {
        let cmd = map_exec_outcome("test", ExecOutcome::TimedOut);
        assert!(matches!(cmd, CustomCmd::CommandCancelled));
    }

    #[test]
    fn map_exec_outcome_failed() {
        let error = std::io::Error::other("boom");
        let cmd = map_exec_outcome("test", ExecOutcome::Failed(error));
        assert!(matches!(cmd, CustomCmd::CommandCancelled));
    }
}
