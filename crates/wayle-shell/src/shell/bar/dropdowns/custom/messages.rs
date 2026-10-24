//! Messages exchanged by the user-defined dropdown panel component.

use std::sync::Arc;

use wayle_config::{ConfigService, schemas::modules::CustomDropdownDefinition};

use super::item::DropdownListItem;

pub(crate) struct CustomDropdownInit {
    pub config: Arc<ConfigService>,
    pub definition: CustomDropdownDefinition,
}

#[derive(Debug)]
pub(crate) enum CustomDropdownMsg {
    /// An item was clicked; its command should run.
    ItemActivated(Box<DropdownListItem>),
}

#[derive(Debug)]
pub(crate) enum CustomDropdownCmd {
    /// Poll tick: run the configured command for fresh panel content.
    PollTrigger,
    /// A one-shot command was cancelled, timed out, or failed to spawn.
    CommandCancelled,
    /// A one-shot command executed successfully.
    CommandOutput(String),
    /// A watch command emitted one complete panel payload.
    WatchOutput(String),
    /// Styling scale changed; panel dimensions are rescaled.
    ScaleChanged(f32),
    /// The user rewrote the definition for this dropdown in config.
    DefinitionChanged(Box<CustomDropdownDefinition>),
    /// The definition was removed from config entirely.
    DefinitionRemoved,
}
