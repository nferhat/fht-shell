use glib::prelude::*;
use gtk::glib;

mod imp {
    use std::cell::RefCell;

    use adw::prelude::BinExt;
    use adw::subclass::bin::BinImpl;
    use glib::subclass::object::{DerivedObjectProperties, ObjectImpl, ObjectImplExt};
    use glib::subclass::types::{ObjectSubclass, ObjectSubclassExt};
    use gtk::subclass::widget::WidgetImpl;

    use super::*;
    use crate::daemons::upower;

    #[derive(glib::Properties, Default, Debug)]
    #[properties(wrapper_type = super::BatteryIcon)]
    pub struct BatteryIcon {
        #[property(get, set, name = "battery-id", type = String)]
        battery_id: RefCell<String>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for BatteryIcon {
        const NAME: &'static str = "BatteryIcon";
        type Type = super::BatteryIcon;
        type ParentType = adw::Bin;
    }

    impl ObjectImpl for BatteryIcon {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();

            let upower_daemon = upower::get();
            let battery_icon = gtk::Image::from_icon_name("battery-missing-symbolic");
            obj.set_child(Some(&battery_icon));

            if let Some(device) = upower_daemon
                .devices()
                .iter()
                // FIXME: Config
                .find(|device| &*device.id() == "battery_BAT0")
            {
                // We found the needed device, now start listening to it
                let device_icon_name_changes = device.proxy().receive_icon_name_changed();
                let battery_icon_weak = ObjectExt::downgrade(&battery_icon);
                glib::spawn_future_local(async move {
                    use futures_util::StreamExt;
                    let mut device_icon_name_changes = device_icon_name_changes.await;
                    while let Some(changed) = device_icon_name_changes.next().await {
                        let icon_name = match changed.get().await {
                            Err(err) => {
                                error!(?err, "Failed to get new battery icon name");
                                continue;
                            }
                            Ok(icon_name) => icon_name,
                        };

                        let Some(battery_icon) = battery_icon_weak.upgrade() else {
                            break; // label does not exist anymore?
                        };

                        // Custom our own resources since I have overriden the icons.
                        let resource_path =
                            format!("/fht/desktop/Shell/icons/scalable/actions/{icon_name}.svg");
                        battery_icon.set_from_resource(Some(&resource_path));
                    }
                });
            }
        }

        fn properties() -> &'static [glib::ParamSpec] {
            Self::derived_properties()
        }

        fn set_property(&self, id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            self.derived_set_property(id, value, pspec);
        }

        fn property(&self, id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            self.derived_property(id, pspec)
        }
    }

    impl WidgetImpl for BatteryIcon {}
    impl BinImpl for BatteryIcon {}
}

glib::wrapper! {
    pub struct BatteryIcon(ObjectSubclass<imp::BatteryIcon>)
        @extends adw::Bin, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl BatteryIcon {
    pub fn new() -> Self {
        glib::Object::new()
    }
}
