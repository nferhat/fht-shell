use gtk::subclass::prelude::*;
use gtk::{gio, glib};

use crate::application::Application;

mod imp {
    use std::sync::LazyLock;

    use adw::subclass::prelude::AdwApplicationWindowImpl;
    use glib::object::ObjectExt;
    use glib::subclass::Signal;
    use gtk::prelude::{BoxExt, ButtonExt, WidgetExt};

    use super::*;
    use crate::widgets;

    #[derive(Default, Debug, gtk::CompositeTemplate)]
    #[template(resource = "/fht/desktop/Shell/ui/panel-window.ui")]
    pub struct PanelWindow {
        #[template_child]
        centerbox: TemplateChild<gtk::CenterBox>,
        #[template_child]
        left_box: TemplateChild<gtk::Box>,
        #[template_child]
        right_box: TemplateChild<gtk::Box>,
        #[template_child]
        middle_box: TemplateChild<gtk::Box>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for PanelWindow {
        const NAME: &'static str = "PanelWindow";
        type Type = super::PanelWindow;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
            klass.bind_template_callbacks();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for PanelWindow {
        fn constructed(&self) {
            self.parent_constructed();
            self.obj().add_css_class("panel-window");

            self.right_box.append(
                &gtk::Label::builder()
                    .use_markup(true)
                    .label("<tt>I am watching you...</tt>")
                    .css_classes(["funny-text"])
                    .build(),
            );
            self.right_box
                .append(&gtk::Separator::new(gtk::Orientation::Vertical));

            let status_widget = widgets::status::StatusWidget::new();
            let status_widget_button = gtk::Button::builder()
                .css_classes(["flat"])
                .can_shrink(true)
                .child(&status_widget)
                .overflow(gtk::Overflow::Hidden)
                .build();
            status_widget_button.connect_clicked(glib::clone!(@weak self as this => move |_| {
                this.obj().emit_by_name::<()>("toggle-controls", &[]);
            }));
            self.right_box.append(&status_widget_button);

            self.right_box
                .append(&gtk::Separator::new(gtk::Orientation::Vertical));
            self.right_box.append(&widgets::time::TimeWidget::new());
        }

        fn signals() -> &'static [glib::subclass::Signal] {
            static SIGNALS: LazyLock<Vec<Signal>> =
                LazyLock::new(|| vec![Signal::builder("toggle-controls").build()]);
            &SIGNALS
        }
    }

    impl WidgetImpl for PanelWindow {}
    impl WindowImpl for PanelWindow {
        // Save window state on delete event
        fn close_request(&self) -> glib::Propagation {
            self.parent_close_request()
        }
    }

    impl ApplicationWindowImpl for PanelWindow {}
    impl AdwApplicationWindowImpl for PanelWindow {}

    #[gtk::template_callbacks]
    impl PanelWindow {
        #[template_callback]
        fn on_cancel_clicked(&self) {}
    }
}

glib::wrapper! {
    pub struct PanelWindow(ObjectSubclass<imp::PanelWindow>)
        @extends adw::ApplicationWindow, gtk::Widget, gtk::Window, gtk::ApplicationWindow,
        @implements gio::ActionMap, gio::ActionGroup, gtk::Root;
}

impl PanelWindow {
    pub fn new(app: &Application) -> Self {
        glib::Object::builder().property("application", app).build()
    }
}
