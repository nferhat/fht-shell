//! # D-Bus interface proxy for: `org.freedesktop.NetworkManager.Device.Statistics`
//!
//! This code was generated by `zbus-xmlgen` `5.1.0` from D-Bus introspection data.
//! Source: `Interface '/org/freedesktop/NetworkManager/Devices/3' from service 'org.freedesktop.NetworkManager' on system bus`.
//!
//! You may prefer to adapt it, instead of using it verbatim.
//!
//! More information can be found in the [Writing a client proxy] section of the zbus
//! documentation.
//!
//! This type implements the [D-Bus standard interfaces], (`org.freedesktop.DBus.*`) for which the
//! following zbus API can be used:
//!
//! * [`zbus::fdo::PropertiesProxy`]
//! * [`zbus::fdo::IntrospectableProxy`]
//! * [`zbus::fdo::PeerProxy`]
//!
//! Consequently `zbus-xmlgen` did not generate code for the above interfaces.
//!
//! [Writing a client proxy]: https://dbus2.github.io/zbus/client.html
//! [D-Bus standard interfaces]: https://dbus.freedesktop.org/doc/dbus-specification.html#standard-interfaces,
use zbus::proxy;
#[proxy(
    interface = "org.freedesktop.NetworkManager.Device.Statistics",
    default_service = "org.freedesktop.NetworkManager",
    default_path = "/org/freedesktop/NetworkManager/Devices/3"
)]
pub trait Statistics {
    /// RefreshRateMs property
    #[zbus(property)]
    fn refresh_rate_ms(&self) -> zbus::Result<u32>;
    #[zbus(property)]
    fn set_refresh_rate_ms(&self, value: u32) -> zbus::Result<()>;

    /// RxBytes property
    #[zbus(property)]
    fn rx_bytes(&self) -> zbus::Result<u64>;

    /// TxBytes property
    #[zbus(property)]
    fn tx_bytes(&self) -> zbus::Result<u64>;
}
