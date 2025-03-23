use glib::prelude::*;
use gtk::glib;

mod imp {

    use adw::prelude::BinExt;
    use adw::subclass::bin::BinImpl;
    use anyhow::Context;
    use futures_util::StreamExt;
    use glib::subclass::object::{ObjectImpl, ObjectImplExt};
    use glib::subclass::types::{ObjectSubclass, ObjectSubclassExt};
    use gtk::prelude::BoxExt;
    use gtk::subclass::widget::WidgetImpl;

    use super::*;
    use crate::daemons::network_manager::{self, access_point, device, wireless};
    use crate::daemons::{self};

    #[derive(Default, Debug)]
    pub struct NetworkIcons;

    #[glib::object_subclass]
    impl ObjectSubclass for NetworkIcons {
        const NAME: &'static str = "NetworkIcons";
        type Type = super::NetworkIcons;
        type ParentType = adw::Bin;
    }

    impl ObjectImpl for NetworkIcons {
        fn constructed(&self) {
            self.parent_constructed();
            let obj = self.obj();

            // Updating the wifi icon is twofold.
            // 1. Get the state from NetworkManager about general network state: Connected,
            //    disonnected, connecting, and whether internet access is available at all.
            //
            // 2. When our main wifi device connects, we track its connected access point. Whenever
            //    there's a state update on the wifi device, we re-start a new future that tracks
            //    strength changes for the new active access point.

            let nm_daemon = network_manager::get();
            let icons_box = gtk::Box::builder()
                .spacing(5)
                .orientation(gtk::Orientation::Horizontal)
                .build();
            obj.set_child(Some(&icons_box));

            if let anyhow::Result::Err(err) = async_io::block_on(async {
                for device in nm_daemon.devices() {
                    let device_type = device.device_type().await?;
                    match device_type {
                        device::DeviceType::Wifi => {
                            let wifi_icon = add_wifi_device(device)
                                .await
                                .context("Failed to add Wi-Fi device")?;
                            icons_box.append(&wifi_icon);
                        }
                        device::DeviceType::Ethernet => {
                            let wired_icon = add_wired_device(device)
                                .await
                                .context("Failed to add Wired device")?;
                            icons_box.append(&wired_icon);
                        }
                        _ => (), // dont care
                    }
                    // if device_type ==  {
                    // }
                }

                anyhow::Result::<_, anyhow::Error>::Ok(())
            }) {
                error!(?err, "Failed to add network devices to NetworkIcons");
            }
        }
    }

    async fn add_wired_device(
        device_proxy: &'static device::DeviceProxy<'static>,
    ) -> anyhow::Result<gtk::Image> {
        let wired_icon = gtk::Image::builder()
            .icon_name("network-wired-disconnected-symbolic")
            .icon_size(gtk::IconSize::Normal)
            .build();

        let mut eth_state_changed_incoming = device_proxy.receive_state_changed_impl().await?;
        let weak_wired_icon = wired_icon.downgrade();
        glib::spawn_future_local(async move {
            while let Some(changed) = eth_state_changed_incoming.next().await {
                let Ok(args) = changed.args() else { continue };

                let Some(wired_icon) = weak_wired_icon.upgrade() else {
                    continue;
                };

                match args.new_state {
                    device::DeviceState::Unknown => {
                        wired_icon.set_from_icon_name(Some("network-wired-error-symbolic"))
                    }
                    device::DeviceState::Unmanaged
                    | device::DeviceState::Unavailable
                    | device::DeviceState::Disconnected => {
                        wired_icon.set_from_icon_name(Some("network-wired-disconnected-symbolic"))
                    }
                    device::DeviceState::Prepare
                    | device::DeviceState::Config
                    | device::DeviceState::IpConfig
                    | device::DeviceState::IpCheck
                    | device::DeviceState::Secondaries
                    | device::DeviceState::Deactivating => {
                        wired_icon.set_from_icon_name(Some("network-wired-acquiring-symbolic"));
                    }
                    device::DeviceState::NeedAuth => {
                        wired_icon.set_from_icon_name(Some("network-wired-no-route-symbolic"))
                    }
                    device::DeviceState::Activated => {
                        wired_icon.set_from_icon_name(Some("network-wired-activated-symbolic"))
                    }
                    device::DeviceState::Failed => {
                        wired_icon.set_from_icon_name(Some("network-wired-error-symbolic"))
                    }
                }
            }
        });

        Ok(wired_icon)
    }

    async fn add_wifi_device(
        device_proxy: &'static device::DeviceProxy<'static>,
    ) -> anyhow::Result<gtk::Image> {
        let conn = daemons::system_connection().inner();
        let path = device_proxy.inner().path().to_owned();
        let wireless_proxy = wireless::WirelessProxy::new(&conn, path).await?;

        let wifi_icon = gtk::Image::builder()
            .icon_name("network-wireless-signal-none-symbolic")
            .icon_size(gtk::IconSize::Normal)
            .build();

        // We keep around the future that tracks the current active access point's
        // strength. If we change state or the access point is not active anymore, we stop it.
        let mut strength_future_join_handle = Option::<glib::JoinHandle<()>>::None;

        // Do an initial update
        match update_wireless_icon(&wifi_icon, &wireless_proxy).await {
            Ok(handle) => strength_future_join_handle = Some(handle),
            Err(_) => {} /* If there's nothing, this may mean that there's no
                          * access point for now */
        }
        let mut wifi_state_changed_incoming = device_proxy.receive_state_changed_impl().await?;

        let weak_wifi_icon = wifi_icon.downgrade();
        glib::spawn_future_local(async move {
            while let Some(changed) = wifi_state_changed_incoming.next().await {
                // Stop the previous future that was responsible for tracking the strength property
                // on the previously (or not) active wireless access point.
                //
                // The active hotspot might have changed, or might not even exist anymore when the
                // state changes. So, we must stop it.
                if let Some(handle) = strength_future_join_handle.take() {
                    handle.abort();
                }
                let Ok(args) = changed.args() else { continue };

                let Some(wifi_icon) = weak_wifi_icon.upgrade() else {
                    continue;
                };

                match args.new_state {
                    device::DeviceState::Unknown => {
                        wifi_icon.set_from_icon_name(Some("network-wireless-error-symbolic"))
                    }
                    device::DeviceState::Unmanaged
                    | device::DeviceState::Unavailable
                    | device::DeviceState::Disconnected => {
                        wifi_icon.set_from_icon_name(Some("network-wireless-offline-symbolic"))
                    }
                    device::DeviceState::Prepare
                    | device::DeviceState::Config
                    | device::DeviceState::IpConfig
                    | device::DeviceState::IpCheck
                    | device::DeviceState::Secondaries
                    | device::DeviceState::Deactivating => {
                        wifi_icon.set_from_icon_name(Some("network-wireless-acquiring-symbolic"));
                    }
                    device::DeviceState::NeedAuth => {
                        wifi_icon.set_from_icon_name(Some("network-wireless-no-route-symbolic"))
                    }
                    device::DeviceState::Activated => {
                        match update_wireless_icon(&wifi_icon, &wireless_proxy).await {
                            Ok(handle) => strength_future_join_handle = Some(handle),
                            Err(err) => {
                                error!(?err, "Failed to update Wi-Fi icon, a generic icon will be displayed instead");
                                wifi_icon.set_from_icon_name(Some("network-wireless-symbolic"));
                            }
                        }
                    }
                    device::DeviceState::Failed => {
                        wifi_icon.set_from_icon_name(Some("network-wireless-error-symbolic"))
                    }
                }
            }
        });

        Ok(wifi_icon)
    }

    async fn update_wireless_icon(
        icon: &gtk::Image,
        wireless_proxy: &wireless::WirelessProxy<'_>,
    ) -> anyhow::Result<glib::JoinHandle<()>> {
        let access_point = wireless_proxy
            .active_access_point()
            .await
            .context("Failed to get active access point")?;
        let conn = daemons::system_connection();
        let access_point = access_point::AccessPointProxy::new(conn.inner(), access_point).await?;
        let mut strength_changes = access_point.receive_strength_changed().await;

        let icon = icon.downgrade();
        let handle = glib::spawn_future_local(async move {
            while let Some(changed) = strength_changes.next().await {
                let Ok(strength) = changed.get().await else {
                    continue; // should not happen but still;
                };

                let icon_name = match strength {
                    0..20 => "network-wireless-signal-none-symbolic",
                    20..40 => "network-wireless-signal-weak-symbolic",
                    40..60 => "network-wireless-signal-ok-symbolic",
                    60..80 => "network-wireless-signal-good-symbolic",
                    80.. => "network-wireless-symbolic",
                };

                if let Some(icon) = icon.upgrade() {
                    icon.set_from_icon_name(Some(icon_name));
                }
            }
        });

        Ok(handle)
    }

    impl WidgetImpl for NetworkIcons {}
    impl BinImpl for NetworkIcons {}
}

glib::wrapper! {
    pub struct NetworkIcons(ObjectSubclass<imp::NetworkIcons>)
        @extends adw::Bin, gtk::Widget,
        @implements gtk::Accessible, gtk::Buildable, gtk::ConstraintTarget;
}

impl NetworkIcons {
    pub fn new() -> Self {
        glib::Object::new()
    }
}
