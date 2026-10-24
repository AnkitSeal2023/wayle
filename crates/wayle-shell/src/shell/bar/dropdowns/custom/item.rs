//! A single row of the user-defined dropdown panel.

use gtk::prelude::*;
use relm4::{gtk, prelude::*};
use wayle_config::schemas::modules::CustomDropdownItem;

/// Output from a panel row: the clicked item's data bubbles up so the
/// parent component can run its command.
#[derive(Debug)]
pub(crate) enum DropdownListItemOutput {
    Activated(Box<DropdownListItem>),
}

/// Rendered data for one panel row.
///
/// Derived from either a static config item (`[[modules.dropdowns.items]]`)
/// or one parsed from the command's JSON/plain output.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct DropdownListItem {
    pub label: String,
    pub icon: Option<String>,
    pub subtitle: Option<String>,
    pub command: Option<String>,
    pub active: bool,
    pub class: Option<String>,
}

impl From<&CustomDropdownItem> for DropdownListItem {
    fn from(item: &CustomDropdownItem) -> Self {
        Self {
            label: item.label.clone(),
            icon: item.icon.clone().filter(|icon| !icon.is_empty()),
            subtitle: item.subtitle.clone().filter(|sub| !sub.is_empty()),
            command: item.command.clone().filter(|command| !command.is_empty()),
            active: item.active,
            class: item.class.clone().filter(|class| !class.is_empty()),
        }
    }
}

impl From<CustomDropdownItem> for DropdownListItem {
    fn from(item: CustomDropdownItem) -> Self {
        Self::from(&item)
    }
}

#[relm4::factory(pub(crate))]
impl FactoryComponent for DropdownListItem {
    type Init = DropdownListItem;
    type Input = ();
    type Output = DropdownListItemOutput;
    type CommandOutput = ();
    type ParentWidget = gtk::Box;

    view! {
        gtk::Box {
            add_css_class: "custom-dropdown-item",
            set_cursor_from_name: Some("pointer"),
            #[watch]
            set_css_classes: &self.root_css_classes(),

            #[name = "icon_container"]
            gtk::Box {
                add_css_class: "custom-dropdown-item-icon",
                set_valign: gtk::Align::Center,
                #[watch]
                set_visible: self.icon.is_some(),

                gtk::Image {
                    add_css_class: "custom-dropdown-icon",
                    set_valign: gtk::Align::Center,
                    #[watch]
                    set_icon_name: self.icon.as_deref(),
                },
            },

            #[name = "info_column"]
            gtk::Box {
                add_css_class: "custom-dropdown-item-info",
                set_orientation: gtk::Orientation::Vertical,
                set_valign: gtk::Align::Center,
                set_halign: gtk::Align::Start,
                set_hexpand: true,

                gtk::Label {
                    add_css_class: "custom-dropdown-item-label",
                    set_halign: gtk::Align::Start,
                    set_ellipsize: gtk::pango::EllipsizeMode::End,
                    #[watch]
                    set_label: &self.label,
                },

                gtk::Label {
                    add_css_class: "custom-dropdown-item-subtitle",
                    set_halign: gtk::Align::Start,
                    set_ellipsize: gtk::pango::EllipsizeMode::End,
                    #[watch]
                    set_label: self.subtitle.as_deref().unwrap_or_default(),
                    #[watch]
                    set_visible: self.subtitle.is_some(),
                },
            },

            gtk::Image {
                add_css_class: "custom-dropdown-item-active-icon",
                set_valign: gtk::Align::Center,
                set_icon_name: Some("tb-check-symbolic"),
                #[watch]
                set_visible: self.active,
            },
        }
    }

    fn init_model(init: Self::Init, _index: &Self::Index, _sender: FactorySender<Self>) -> Self {
        init
    }

    fn init_widgets(
        &mut self,
        _index: &Self::Index,
        root: Self::Root,
        _returned_widget: &<Self::ParentWidget as relm4::factory::FactoryView>::ReturnedWidget,
        sender: FactorySender<Self>,
    ) -> Self::Widgets {
        let widgets = view_output!();

        if let Some(class) = &self.class {
            root.add_css_class(class);
        }

        let click = gtk::GestureClick::new();
        let item = self.clone();
        click.connect_released(move |gesture, _, _, _| {
            gesture.set_state(gtk::EventSequenceState::Claimed);
            let _ = sender.output(DropdownListItemOutput::Activated(Box::new(item.clone())));
        });
        root.add_controller(click);

        widgets
    }
}

impl DropdownListItem {
    fn root_css_classes(&self) -> Vec<&'static str> {
        let mut classes = vec!["custom-dropdown-item"];

        if self.active {
            classes.push("active");
        }

        classes
    }
}
