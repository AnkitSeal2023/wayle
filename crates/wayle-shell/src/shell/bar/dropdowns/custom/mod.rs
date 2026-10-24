//! User-defined dropdown panel, opened from any module click action with
//! `dropdown:<id>`.
//!
//! Content comes from static `items` in the config and/or a shell command
//! whose output replaces the static items. See
//! [`wayle_config::schemas::modules::CustomDropdownDefinition`].

mod factory;
mod helpers;
mod item;
mod messages;
mod methods;
mod watchers;

use gtk::prelude::*;
use relm4::{gtk, prelude::*};
use wayle_config::schemas::modules::{CustomDropdownDefinition, ExecutionMode};
use wayle_widgets::{WatcherToken, prelude::*};

use self::item::DropdownListItem;
pub(crate) use self::{
    factory::create_for_id,
    messages::{CustomDropdownCmd, CustomDropdownInit, CustomDropdownMsg},
};
use crate::i18n::t;

/// Default panel width in pixels (before scale).
pub(crate) const BASE_WIDTH: f32 = 320.0;
/// Maximum panel height in pixels (before scale); longer lists scroll.
pub(crate) const BASE_MAX_HEIGHT: f32 = 480.0;

pub(crate) struct CustomDropdown {
    definition: CustomDropdownDefinition,
    definition_present: bool,
    removed: bool,
    items: FactoryVecDeque<DropdownListItem>,
    header_title: Option<String>,
    header_icon: Option<String>,
    width_base: f32,
    scaled_width: i32,
    scaled_max_height: i32,
    scale: f32,
    poller_token: WatcherToken,
    watcher_token: WatcherToken,
    command_token: WatcherToken,
    last_output: Option<String>,
}

#[relm4::component(pub(crate))]
impl Component for CustomDropdown {
    type Init = CustomDropdownInit;
    type Input = CustomDropdownMsg;
    type Output = ();
    type CommandOutput = CustomDropdownCmd;

    view! {
        #[root]
        gtk::Popover {
            set_css_classes: &["dropdown", "custom-dropdown"],
            set_has_arrow: false,
            #[watch]
            set_width_request: model.scaled_width,

            #[template]
            Dropdown {
                set_overflow: gtk::Overflow::Hidden,

                #[template]
                DropdownHeader {
                    #[watch]
                    set_visible: model.header_title.is_some()
                        || model
                            .header_icon
                            .is_some(),

                    #[template_child]
                    icon {
                        #[watch]
                        set_visible: model.header_icon.is_some(),
                        #[watch]
                        set_icon_name: Some(
                            model
                                .header_icon
                                .as_deref()
                                .unwrap_or_default(),
                        ),
                    },
                    #[template_child]
                    label {
                        #[watch]
                        set_visible: model.header_title.is_some(),
                        #[watch]
                        set_label: model
                            .header_title
                            .as_deref()
                            .unwrap_or_default(),
                    },
                },

                #[template]
                DropdownContent {
                    add_css_class: "custom-content",

                    gtk::ScrolledWindow {
                        add_css_class: "custom-scroll",
                        set_hscrollbar_policy: gtk::PolicyType::Never,
                        set_vexpand: true,
                        set_propagate_natural_height: true,
                        #[watch]
                        set_max_content_height: model.scaled_max_height,

                        gtk::Box {
                            set_orientation: gtk::Orientation::Vertical,

                            #[local_ref]
                            items_widget -> gtk::Box {
                                set_orientation: gtk::Orientation::Vertical,
                            },

                            #[name = "empty_items"]
                            #[template]
                            EmptyState {
                                #[watch]
                                set_visible: !model.removed
                                    && model.items.is_empty(),
                                #[template_child]
                                icon {
                                    set_icon_name: Some(
                                        "ld-panel-top-symbolic",
                                    ),
                                },
                                #[template_child]
                                title {
                                    set_label: &t!(
                                        "dropdown-custom-empty-title"
                                    ),
                                },
                                #[template_child]
                                description {
                                    set_label: &t!(
                                        "dropdown-custom-empty-description"
                                    ),
                                },
                            },

                            #[name = "removed_state"]
                            #[template]
                            EmptyState {
                                #[watch]
                                set_visible: model.removed,
                                #[template_child]
                                icon {
                                    set_icon_name: Some(
                                        "ld-x-circle-symbolic",
                                    ),
                                },
                                #[template_child]
                                title {
                                    set_label: &t!(
                                        "dropdown-custom-removed-title"
                                    ),
                                },
                                #[template_child]
                                description {
                                    set_label: &t!(
                                        "dropdown-custom-removed-description"
                                    ),
                                },
                            },
                        },
                    },
                },
            },
        }
    }

    fn init(
        init: Self::Init,
        _root: Self::Root,
        sender: ComponentSender<Self>,
    ) -> ComponentParts<Self> {
        let definition = init.definition;
        let scale = init.config.config().styling.scale.get().value();

        let items = Self::build_item_list(&sender);

        watchers::spawn_definition_watcher(
            &sender,
            &init.config.config().modules.dropdowns,
            definition.id.clone(),
        );
        watchers::spawn_scale_watcher(&sender, &init.config);

        let mut model = Self {
            definition: definition.clone(),
            definition_present: true,
            removed: false,
            items,
            header_title: None,
            header_icon: None,
            width_base: 0.0,
            scaled_width: 0,
            scaled_max_height: 0,
            scale,
            poller_token: WatcherToken::new(),
            watcher_token: WatcherToken::new(),
            command_token: WatcherToken::new(),
            last_output: None,
        };

        model.apply_definition_baseline();

        if definition.command.is_some() {
            match definition.mode {
                ExecutionMode::Poll => {
                    watchers::spawn_command_poller(
                        &sender,
                        &definition,
                        model.poller_token.reset(),
                    );
                    watchers::run_definition_command(
                        &sender,
                        &definition,
                        model.command_token.reset(),
                    );
                }
                ExecutionMode::Watch => {
                    watchers::spawn_command_watcher(
                        &sender,
                        &definition,
                        model.watcher_token.reset(),
                    );
                }
            }
        }

        let items_widget = model.items.widget();
        let widgets = view_output!();

        ComponentParts { model, widgets }
    }

    fn update(&mut self, msg: Self::Input, _sender: ComponentSender<Self>, root: &Self::Root) {
        match msg {
            CustomDropdownMsg::ItemActivated(item) => {
                self.handle_item_activated(*item, root);
            }
        }
    }

    fn update_cmd(
        &mut self,
        msg: Self::CommandOutput,
        sender: ComponentSender<Self>,
        _root: &Self::Root,
    ) {
        match msg {
            CustomDropdownCmd::PollTrigger => {
                if let Some(command) = self.definition.command.clone() {
                    watchers::run_command(
                        &sender,
                        &self.definition.id,
                        command,
                        self.command_token.reset(),
                    );
                }
            }
            CustomDropdownCmd::CommandOutput(output) | CustomDropdownCmd::WatchOutput(output) => {
                self.apply_output(&output);
            }
            CustomDropdownCmd::CommandCancelled => {}
            CustomDropdownCmd::ScaleChanged(scale) => {
                self.handle_scale_changed(scale);
            }
            CustomDropdownCmd::DefinitionChanged(definition) => {
                self.handle_definition_changed(&sender, *definition);
            }
            CustomDropdownCmd::DefinitionRemoved => {
                self.handle_definition_removed();
            }
        }
    }
}
