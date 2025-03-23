#![allow(unused)]
use std::sync::OnceLock;

pub mod access_point;
pub mod device;
pub mod service;
pub mod wired;
pub mod wireless;

/// A `NetworkManager` daemon, used to monitor power devices.
/// See NM's page: <https://networkmanager.dev/docs/api/latest/>
pub struct Daemon {
    proxy: service::NetworkManagerProxy<'static>,
    devices: Vec<device::DeviceProxy<'static>>,
}

impl Daemon {
    /// Get the underlying [`zbus::Proxy`] powering this daemon.
    pub fn proxy(&self) -> &service::NetworkManagerProxy<'_> {
        &self.proxy
    }

    /// Get the devices found by NetworkManager
    pub fn devices(&self) -> &[device::DeviceProxy<'_>] {
        &self.devices
    }
}

static INSTANCE: OnceLock<Daemon> = OnceLock::new();

pub fn get() -> &'static Daemon {
    INSTANCE.get().expect("daemons::start() must be called")
}

pub async fn start() -> anyhow::Result<()> {
    if INSTANCE.get().is_some() {
        return Ok(());
    }

    let conn = super::system_connection().inner();
    let mut daemon = Daemon {
        proxy: service::NetworkManagerProxy::new(conn).await?,
        devices: vec![],
    };

    let devices = daemon.proxy.get_devices().await?;
    for device_path in devices {
        let device_proxy = device::DeviceProxy::new(conn, device_path.clone()).await?;
        daemon.devices.push(device_proxy);
    }

    // NOTE: If we already started he handled it above.
    let _ = INSTANCE.set(daemon);

    Ok(())
}
