//! Behavior for the user-defined dropdown panel.

use gtk::prelude::*;
use relm4::{gtk, prelude::*};
use tracing::debug;
use wayle_config::schemas::modules::{CustomDropdownDefinition, ExecutionMode};

use super::{
    CustomDropdown, helpers,
    item::{DropdownListItem, DropdownListItemOutput},
    messages::CustomDropdownMsg,
    watchers,
};
use crate::{process, shell::bar::dropdowns::scaled_dimension};

impl CustomDropdown {
    /// Builds the item factory, forwarding row clicks into the component.
    pub(super) fn build_item_list(
        sender: &ComponentSender<Self>,
    ) -> FactoryVecDeque<DropdownListItem> {
        FactoryVecDeque::builder()
            .launch(gtk::Box::default())
            .forward(sender.input_sender(), |output| match output {
                DropdownListItemOutput::Activated(item) => CustomDropdownMsg::ItemActivated(item),
            })
    }

    /// Runs the clicked item's command, closing the panel when asked.
    pub(super) fn handle_item_activated(&self, item: DropdownListItem, root: &gtk::Popover) {
        let Some(command) = item.command.as_deref() else {
            return;
        };

        process::run_if_set(command);

        if self.definition.close_on_click {
            root.popdown();
        }
    }

    /// Applies a config edit for this dropdown id.
    ///
    /// Execution settings (mode, interval, restart policy, command) restart
    /// the underlying process plumbing; everything else re-renders only.
    pub(super) fn handle_definition_changed(
        &mut self,
        sender: &ComponentSender<Self>,
        new_definition: CustomDropdownDefinition,
    ) {
        let was_removed = !self.definition_present;
        if !was_removed && self.definition == new_definition {
            return;
        }

        let needs_restart =
            was_removed || Self::execution_settings_changed(&self.definition, &new_definition);

        debug!(dropdown = %new_definition.id, "custom dropdown definition changed in config");

        self.stop_execution_watchers();
        self.cancel_inflight_commands();

        self.definition = new_definition;
        self.definition_present = true;
        self.removed = false;

        if needs_restart {
            self.last_output = None;
            self.restart_execution_watchers(sender);
        }

        if let Some(output) = self.last_output.clone() {
            self.apply_output(&output);
        } else {
            self.apply_definition_baseline();
        }
    }

    /// Applies removal of the definition from config.
    pub(super) fn handle_definition_removed(&mut self) {
        if !self.definition_present {
            return;
        }

        debug!(dropdown = %self.definition.id, "custom dropdown definition removed from config");

        self.definition_present = false;
        self.stop_execution_watchers();
        self.cancel_inflight_commands();
        self.last_output = None;
        self.removed = true;

        {
            let mut guard = self.items.guard();
            guard.clear();
        }
    }

    /// Rescales panel dimensions on styling scale changes.
    pub(super) fn handle_scale_changed(&mut self, scale: f32) {
        self.scale = scale;
        self.refresh_scaled_sizes();
    }

    /// Applies a fresh command output as panel content.
    pub(super) fn apply_output(&mut self, output: &str) {
        if !self.definition_present {
            return;
        }

        self.last_output = Some(output.to_owned());

        let parsed = helpers::parse_panel(output);

        self.header_title = parsed.title.or(self.definition.title.clone());
        self.header_icon = parsed.icon.or(self.definition.icon.clone());
        let width = parsed
            .width
            .or(self.definition.width)
            .unwrap_or(super::BASE_WIDTH as u32);
        self.set_width_base(width as f32);

        self.apply_items(parsed.items);
    }

    /// Applies the static config state: header fields from the definition and
    /// static items, unless a command output is already in charge.
    pub(super) fn apply_definition_baseline(&mut self) {
        self.header_title = self
            .definition
            .title
            .clone()
            .filter(|title| !title.trim().is_empty());
        self.header_icon = self
            .definition
            .icon
            .clone()
            .filter(|icon| !icon.trim().is_empty());

        let width = self.definition.width.unwrap_or(super::BASE_WIDTH as u32);
        self.set_width_base(width as f32);

        if self.last_output.is_none() {
            let items: Vec<DropdownListItem> = self
                .definition
                .items
                .iter()
                .map(DropdownListItem::from)
                .collect();
            self.apply_items(items);
        }
    }

    /// Replaces the full item list.
    ///
    /// The factory guard applies its clear + pushes as one batch on drop.
    pub(super) fn apply_items(&mut self, list: Vec<DropdownListItem>) {
        {
            let mut guard = self.items.guard();
            guard.clear();
            for item in list {
                guard.push_back(item);
            }
        }
    }

    fn set_width_base(&mut self, base: f32) {
        self.width_base = base;
        self.refresh_scaled_sizes();
    }

    pub(super) fn refresh_scaled_sizes(&mut self) {
        self.scaled_width = scaled_dimension(self.width_base, self.scale);
        self.scaled_max_height = scaled_dimension(super::BASE_MAX_HEIGHT, self.scale);
    }

    fn execution_settings_changed(
        current: &CustomDropdownDefinition,
        next: &CustomDropdownDefinition,
    ) -> bool {
        current.mode != next.mode
            || current.interval_ms != next.interval_ms
            || current.restart_policy != next.restart_policy
            || current.restart_interval_ms != next.restart_interval_ms
            || current.command != next.command
    }

    fn stop_execution_watchers(&mut self) {
        self.poller_token.reset();
        self.watcher_token.reset();
    }

    fn cancel_inflight_commands(&mut self) {
        self.command_token.reset();
    }

    fn restart_execution_watchers(&mut self, sender: &ComponentSender<Self>) {
        match self.definition.mode {
            ExecutionMode::Poll => {
                self.watcher_token.reset();
                watchers::spawn_command_poller(sender, &self.definition, self.poller_token.reset());
                watchers::run_definition_command(
                    sender,
                    &self.definition,
                    self.command_token.reset(),
                );
            }
            ExecutionMode::Watch => {
                self.poller_token.reset();
                watchers::spawn_command_watcher(
                    sender,
                    &self.definition,
                    self.watcher_token.reset(),
                );
            }
        }
    }
}
