use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;

mod imp {
    use std::cell::OnceCell;

    use glib::WeakRef;

    pub use super::*;

    #[derive(Default, Debug)]
    pub struct TimeWidget {
        label: OnceCell<WeakRef<gtk::Label>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for TimeWidget {
        const NAME: &'static str = "TimeWidget";
        type Type = super::TimeWidget;
        type ParentType = gtk::Box;
    }

    fn current_time() -> String {
        format!("{}", chrono::Local::now().format("%a %d, %H:%M"))
    }

    impl ObjectImpl for TimeWidget {
        fn constructed(&self) {
            self.parent_constructed();

            let label = gtk::Label::new(Some(&current_time()));
            label.set_halign(gtk::Align::Center);
            label.set_valign(gtk::Align::Center);
            label.set_xalign(-1.0);

            let obj = self.obj();
            obj.set_orientation(gtk::Orientation::Vertical);
            obj.set_baseline_position(gtk::BaselinePosition::Center);
            obj.set_hexpand(true);
            obj.set_halign(gtk::Align::BaselineCenter);
            obj.set_valign(gtk::Align::BaselineCenter);
            obj.append(&label);

            let downgrade = label.downgrade();
            self.label.set(downgrade).unwrap();

            // Now add the ticking.
            glib::timeout_add_seconds_local(30, move || {
                let time = current_time();
                label.set_text(&time);
                glib::ControlFlow::Continue
            });
        }
    }

    impl WidgetImpl for TimeWidget {}
    impl BoxImpl for TimeWidget {}
}

glib::wrapper! {
    pub struct TimeWidget(ObjectSubclass<imp::TimeWidget>)
        @extends gtk::Box, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget, gtk::Orientable;
}

impl TimeWidget {
    pub fn new() -> Self {
        glib::Object::new()
    }
}
