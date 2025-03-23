#![allow(unused)]
use std::sync::{Arc, OnceLock};

pub mod device;
pub mod service;

/// A `UPower` daemon, used to monitor power devices.
/// See freedesktop's page: <https://upower.freedesktop.org/>
pub struct Daemon {
    proxy: service::UPowerProxy<'static>,
    devices: Vec<Device>,
}

impl Daemon {
    /// Get all the devices registered for this [`Daemon`].
    /// **NOTE**: The device list gets populated only on startup!
    pub fn devices(&self) -> &[Device] {
        &self.devices
    }

    /// Get the underlying [`zbus::Proxy`] powering this daemon.
    pub fn proxy(&self) -> &service::UPowerProxy<'_> {
        &self.proxy
    }
}

/// A single `UPower` device, can represent a lid, battery, AC unit, and other things.
///
/// The unique ID of the device is the end of it's D-Bus path, IE.
/// `/org/freedesktop/UPower/devices/battery_BAT0` -> `battery_BAT0`
pub struct Device {
    id: Arc<str>,
    proxy: device::DeviceProxy<'static>,
}

impl Device {
    /// Get the unique ID of this [`Device`].
    pub fn id(&self) -> Arc<str> {
        Arc::clone(&self.id)
    }

    /// Get the underlying [`zbus::Proxy`] behind this interface.
    pub fn proxy(&self) -> &device::DeviceProxy<'_> {
        &self.proxy
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
        proxy: service::UPowerProxy::new(conn).await?,
        devices: vec![],
    };

    let devices = daemon.proxy.enumerate_devices().await?;
    for device_path in devices {
        let device_id: Arc<str> = device_path
            .split('/')
            .last()
            .expect("Invalid device path")
            .into();
        let device_proxy = device::DeviceProxy::new(conn, device_path).await?;

        daemon.devices.push(Device {
            id: device_id,
            proxy: device_proxy,
        });
    }

    // NOTE: If we already started he handled it above.
    let _ = INSTANCE.set(daemon);

    Ok(())
}
