/// Daemons for the shell.
///
/// Daemons are singletons that live for the entire program (IE. they life for 'static)
/// Other parts of the application get them and use channels to communicate from/to them.
use std::sync::OnceLock;

pub mod network_manager;
pub mod notifications;
pub mod upower;

/// Get the connection to the system bus
pub fn system_connection() -> &'static zbus::blocking::Connection {
    static RUNTIME: OnceLock<zbus::blocking::Connection> = OnceLock::new();
    RUNTIME.get_or_init(|| zbus::blocking::Connection::system().unwrap())
}

/// Get the connection to the session bus
#[allow(unused)]
fn session_connection() -> &'static zbus::blocking::Connection {
    static RUNTIME: OnceLock<zbus::blocking::Connection> = OnceLock::new();
    RUNTIME.get_or_init(|| zbus::blocking::Connection::session().unwrap())
}

/// Start all the daemons.
pub async fn start() -> anyhow::Result<()> {
    upower::start().await?;
    network_manager::start().await?;
    Ok(())
}
