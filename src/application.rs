use gtk::subclass::prelude::*;
use gtk::{gio, glib};

use crate::panel::PanelWindow;

mod imp {
    use std::cell::OnceCell;

    use adw::subclass::prelude::AdwApplicationImpl;
    use glib::object::{Cast, ObjectExt};
    use glib::WeakRef;
    use gtk::gdk;
    use gtk::prelude::{DisplayExt, GtkWindowExt, WidgetExt};
    use gtk4_layer_shell::{Edge, LayerShell};

    use super::*;
    use crate::sass::load_css_from_path;

    #[derive(Debug, Default)]
    pub struct Application {
        shells: OnceCell<Vec<OutputShell>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Application {
        const NAME: &'static str = "Application";
        type Type = super::Application;
        type ParentType = adw::Application;
    }

    impl ObjectImpl for Application {}

    impl ApplicationImpl for Application {
        fn activate(&self) {
            self.parent_activate();
            let app = self.obj();

            if let Some(shells) = self.shells.get() {
                for shell in shells {
                    let window = shell.panel.upgrade().unwrap();
                    window.present();
                }

                return;
            }

            // FIXME: Multi-monitor setups are not taken into account

            let display = gdk::Display::default().unwrap();
            let icon_theme = gtk::IconTheme::for_display(&display);
            icon_theme.add_resource_path("/fht/desktop/Shell/icons/scalable/actions/");
            icon_theme.add_resource_path("/fht/desktop/Shell/icons/");

            let mut shells = vec![];
            for monitor in display.monitors().into_iter() {
                let Some(monitor) = monitor
                    .ok()
                    .and_then(|obj| obj.downcast::<gdk::Monitor>().ok())
                else {
                    continue; // should not happen.
                };

                let panel_window = PanelWindow::new(&app);
                panel_window.init_layer_shell();
                panel_window.set_monitor(&monitor);
                panel_window.set_namespace("fht.desktop.Shell.Panel");
                panel_window.set_layer(gtk4_layer_shell::Layer::Top);
                panel_window.set_anchor(Edge::Bottom, true);
                panel_window.set_anchor(Edge::Right, true);
                panel_window.set_anchor(Edge::Left, true);
                panel_window.set_height_request(60);
                panel_window.set_opacity(0.93);
                panel_window.auto_exclusive_zone_enable();
                panel_window.present();

                shells.push(OutputShell {
                    panel: ObjectExt::downgrade(&panel_window),
                });
            }

            self.shells.set(shells).expect("Panels already set.");
        }

        fn startup(&self) {
            self.parent_startup();

            let provider = gtk::CssProvider::new();
            // We support SASS/SCSS, If the user don't want it, they can still write plain old css.

            let paths_to_try = [
                crate::BASE_DIRECTORIES.get_config_file("fht/shell/style.scss"),
                crate::BASE_DIRECTORIES.get_config_file("fht/shell/style.css"),
            ];

            let mut css = None;
            for path in paths_to_try {
                info!(?path, "Trying to load custom style");
                if path.exists() {
                    match load_css_from_path(&path) {
                        Ok(css_content) => {
                            css = Some(css_content);
                            break;
                        }
                        Err(err) => {
                            error!(?err, ?path, "Failed to load custom style from path");
                        }
                    }
                }
            }

            if let Some(css) = css {
                provider.load_from_string(&css);
                gtk::style_context_add_provider_for_display(
                    &gtk::gdk::Display::default().expect("No display?"),
                    &provider,
                    gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
                );
            }
        }
    }

    /// A shell for a single output/monitor.
    #[derive(Debug)]
    struct OutputShell {
        panel: WeakRef<PanelWindow>,
    }

    impl GtkApplicationImpl for Application {}
    impl AdwApplicationImpl for Application {}
}

glib::wrapper! {
    pub struct Application(ObjectSubclass<imp::Application>)
        @extends gtk::Application, adw::Application, gio::Application,
        @implements gio::ActionGroup, gio::ActionMap;
}

impl Application {
    pub fn new() -> Self {
        glib::Object::builder()
            .property("application-id", "fht.desktop.Shell")
            .property("resource-base-path", "/fht/desktop/Shell/")
            .build()
    }
}
