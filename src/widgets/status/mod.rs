//! Panel window status section.
//!
//! This section is very similar to the Windows 11 section with the wifi, battery, and volume
//! section. The style is very similar and on click it should open a popup with quick controls.

pub mod battery;
pub mod network;

use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

mod imp {

    use super::*;

    #[derive(Default, Debug)]
    pub struct StatusWidget {}

    #[glib::object_subclass]
    impl ObjectSubclass for StatusWidget {
        const NAME: &'static str = "StatusWidget";
        type Type = super::StatusWidget;
        type ParentType = gtk::Box;
    }

    impl ObjectImpl for StatusWidget {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();
            obj.append(&network::NetworkIcons::new());
            obj.append(&battery::BatteryIcon::new());
        }
    }

    impl WidgetImpl for StatusWidget {}
    impl BoxImpl for StatusWidget {}
}

glib::wrapper! {
    pub struct StatusWidget(ObjectSubclass<imp::StatusWidget>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl StatusWidget {
    pub fn new() -> Self {
        glib::Object::builder()
            // FIXME: Config
            .property("orientation", gtk::Orientation::Horizontal)
            .property("spacing", 5)
            .build()
    }
}
