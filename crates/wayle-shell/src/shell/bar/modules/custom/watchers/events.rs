use std::time::Duration;

use relm4::ComponentSender;
use tokio_util::sync::CancellationToken;
use wayle_config::{ConfigProperty, schemas::modules::CustomModuleDefinition};
use wayle_widgets::watch;

use super::super::{CustomModule, helpers, messages::CustomCmd};
use crate::watchers::spawn_poller;

const SCROLL_DEBOUNCE: Duration = Duration::from_millis(50);

pub(crate) fn spawn_command_poller(
    sender: &ComponentSender<CustomModule>,
    definition: &CustomModuleDefinition,
    token: CancellationToken,
) {
    if definition.command.is_none() {
        return;
    }

    spawn_poller(sender, definition.interval_ms, token, || {
        CustomCmd::PollTrigger
    });
}

pub(crate) fn spawn_config_watcher(
    sender: &ComponentSender<CustomModule>,
    custom_modules: &ConfigProperty<Vec<CustomModuleDefinition>>,
    module_id: String,
) {
    let custom_modules = custom_modules.clone();

    watch!(sender, [custom_modules.watch()], |out| {
        if let Some(definition) = helpers::find_definition(&custom_modules.get(), &module_id) {
            let _ = out.send(CustomCmd::DefinitionChanged(Box::new(definition)));
        } else {
            let _ = out.send(CustomCmd::DefinitionRemoved);
        }
    });
}

/// Spawns a debounced scroll action that fires after a quiet period.
///
/// If `cancel_token` is triggered before the debounce period expires, the
/// action is cancelled. Reset the token before each scroll to coalesce
/// rapid scrolls into a single on_action execution.
pub(crate) fn spawn_scroll_debounce(
    sender: &ComponentSender<CustomModule>,
    cancel_token: CancellationToken,
) {
    sender.oneshot_command(async move {
        tokio::select! {
            biased;
            () = cancel_token.cancelled() => CustomCmd::CommandCancelled,
            () = tokio::time::sleep(SCROLL_DEBOUNCE) => CustomCmd::ScrollDebounceExpired,
        }
    });
}
