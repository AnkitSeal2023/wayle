//! Factory for `[[modules.dropdowns]]`-defined panels.

use relm4::prelude::*;

use super::{CustomDropdown, messages::CustomDropdownInit};
use crate::shell::{bar::dropdowns::DropdownInstance, services::ShellServices};

/// Creates the config-defined dropdown registered under `id`.
///
/// Returns `None` when no `[[modules.dropdowns]]` entry matches; the
/// registry logs that as a dropped click.
pub(crate) fn create_for_id(id: &str, services: &ShellServices) -> Option<DropdownInstance> {
    let definitions = services.config.config().modules.dropdowns.get();
    let definition = definitions.iter().find(|def| def.id == id)?.clone();

    let init = CustomDropdownInit {
        config: services.config.clone(),
        definition,
    };
    let controller = CustomDropdown::builder().launch(init).detach();

    let popover = controller.widget().clone();
    Some(DropdownInstance::new(popover, Box::new(controller)))
}
