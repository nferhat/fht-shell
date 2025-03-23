#[macro_use]
extern crate tracing;

mod application;
mod daemons;
mod panel;
mod sass;
mod widgets;

use std::sync::LazyLock;

use gtk::gio;
use gtk::prelude::ApplicationExtManual;

static BASE_DIRECTORIES: LazyLock<xdg::BaseDirectories> =
    LazyLock::new(|| xdg::BaseDirectories::new().expect("No HOME?"));

fn main() -> glib::ExitCode {
    // let only_message = tracing_subscriber::fmt::format::debug_fn(|writer, field, value| {
    //     if field.name() == "message" {
    //         write!(writer, "{value:?}")
    //     } else {
    //         Ok(())
    //     }
    // });

    tracing_subscriber::fmt()
        .compact()
        .with_target(false)
        // .fmt_fields(only_message)
        .init();

    async_io::block_on(daemons::start()).unwrap();

    glib::set_application_name("fht-shell");
    glib::log_set_default_handler(glib::rust_log_handler);
    gio::resources_register_include!("fht.desktop.Shell.gresource").unwrap();
    gio::resources_register_include!("fht.desktop.Shell.icons.gresource").unwrap();

    let app = application::Application::new();
    app.run()
}
