use anyhow::Context;
use std::sync::{Arc, OnceLock};
use tokio::sync::broadcast;
use zbus::zvariant;

static INSTANCE: OnceLock<Daemon> = OnceLock::new();

pub fn get() -> &'static Daemon {
    INSTANCE.get().expect("daemons::start() must be called")
}

pub async fn start() -> anyhow::Result<()> {
    let conn = super::system_connection().inner();
    let channel = broadcast::channel(1024);

    let proxy = zbus::Proxy::new(
        conn,
        "org.freedesktop.UPower",
        "/org/freedesktop/UPower",
        "org.freedesktop.UPower",
    )
    .await?;

    let mut daemon = Daemon {
        proxy,
        devices: vec![],
        channel,
    };

    // First get the devices available. For each one, create a new
    let devices: Vec<zvariant::OwnedObjectPath> = daemon
        .proxy
        .call("EnumerateDevices", &())
        .await
        .context("Failed to get devices")?;

    for device_path in devices {
        let device_id: Arc<str> = device_path
            .split('/')
            .last()
            .expect("Invalid device path")
            .into();

        let device_proxy = zbus::Proxy::new(
            conn,
            "org.freedesktop.UPower",
            device_path,
            "org.freedesktop.UPower.Device",
        )
        .await?;

        let percentage: f64 = device_proxy.get_property("Percentage").await?;
        info!(?device_id, ?percentage);

        daemon.devices.push(Device {
            id: device_id,
            proxy: device_proxy,
        });
    }

    INSTANCE
        .set(daemon)
        .map_err(|_| anyhow::anyhow!("Instance already set?"))?;

    Ok(())
}

pub struct Daemon {
    proxy: zbus::Proxy<'static>,
    devices: Vec<Device>,
    channel: (broadcast::Sender<Message>, broadcast::Receiver<Message>),
}

pub struct Device {
    id: Arc<str>,
    proxy: zbus::Proxy<'static>,
}

#[derive(Clone, Debug)]
pub struct Message {}
