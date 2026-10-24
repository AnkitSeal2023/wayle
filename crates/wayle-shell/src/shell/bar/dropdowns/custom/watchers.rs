//! Config and command watchers for the user-defined dropdown panel.
//!
//! Thin wrappers over the shared plumbing in [`crate::watchers::commands`]
//! plus the definition watcher that mirrors filter-by-id behavior from the
//! custom bar modules.

use std::sync::Arc;

use relm4::ComponentSender;
use tokio_util::sync::CancellationToken;
use wayle_config::{ConfigProperty, ConfigService, schemas::modules::CustomDropdownDefinition};
use wayle_widgets::watch;

use super::{CustomDropdown, helpers, messages::CustomDropdownCmd};
use crate::watchers::{report_outcome, run_command_oneshot, spawn_poller, spawn_watch_supervisor};

/// One-shot command run at component startup: seeds panel content without
/// waiting for the first poll tick.
pub(crate) fn run_definition_command(
    sender: &ComponentSender<CustomDropdown>,
    definition: &CustomDropdownDefinition,
    cancel_token: CancellationToken,
) {
    let Some(command) = definition.command.clone() else {
        return;
    };

    run_command(sender, &definition.id, command, cancel_token);
}

/// Runs a one-shot command with timeout and single-flight cancellation.
///
/// Reset `cancel_token` before calling to cancel any in-flight command.
pub(crate) fn run_command(
    sender: &ComponentSender<CustomDropdown>,
    dropdown_id: &str,
    command: String,
    cancel_token: CancellationToken,
) {
    run_command_oneshot(sender, dropdown_id, command, cancel_token, |id, outcome| {
        match report_outcome(id, outcome) {
            Some(output) => CustomDropdownCmd::CommandOutput(output),
            None => CustomDropdownCmd::CommandCancelled,
        }
    });
}

/// Spawns the poll-mode ticker. A `0` interval or missing command opts out.
pub(crate) fn spawn_command_poller(
    sender: &ComponentSender<CustomDropdown>,
    definition: &CustomDropdownDefinition,
    token: CancellationToken,
) {
    if definition.command.is_none() {
        return;
    }

    spawn_poller(sender, definition.interval_ms, token, || {
        CustomDropdownCmd::PollTrigger
    });
}

/// Spawns the watch-mode supervisor: one complete panel payload per stdout
/// line, restarted according to the configured policy.
pub(crate) fn spawn_command_watcher(
    sender: &ComponentSender<CustomDropdown>,
    definition: &CustomDropdownDefinition,
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
        |_surface_id, line| CustomDropdownCmd::WatchOutput(line),
    );
}

/// Watches the `[[modules.dropdowns]]` array for edits that affect this id.
pub(crate) fn spawn_definition_watcher(
    sender: &ComponentSender<CustomDropdown>,
    dropdowns: &ConfigProperty<Vec<CustomDropdownDefinition>>,
    dropdown_id: String,
) {
    let dropdowns = dropdowns.clone();

    watch!(sender, [dropdowns.watch()], |out| {
        if let Some(definition) = helpers::find_definition(&dropdowns.get(), &dropdown_id) {
            let _ = out.send(CustomDropdownCmd::DefinitionChanged(Box::new(definition)));
        } else {
            let _ = out.send(CustomDropdownCmd::DefinitionRemoved);
        }
    });
}

/// Watches the styling scale so panel dimensions stay crisp at any zoom.
pub(crate) fn spawn_scale_watcher(
    sender: &ComponentSender<CustomDropdown>,
    config: &Arc<ConfigService>,
) {
    let scale = config.config().styling.scale.clone();

    watch!(sender, [scale.watch()], |out| {
        let _ = out.send(CustomDropdownCmd::ScaleChanged(scale.get().value()));
    });
}
