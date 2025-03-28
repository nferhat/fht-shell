//! # D-Bus interface proxy for: `org.freedesktop.UPower.Device`
//!
//! This code was generated by `zbus-xmlgen` `5.1.0` from D-Bus introspection data.
//! Source: `Interface '/org/freedesktop/UPower/devices/battery_BAT0' from service
//! 'org.freedesktop.UPower' on system bus`.
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

use zbus::{proxy, zvariant};

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    serde_repr::Deserialize_repr,
    serde_repr::Serialize_repr,
    zvariant::Type,
    zvariant::OwnedValue,
)]
#[repr(u32)]
pub enum State {
    Unknown = 0,
    Charging = 1,
    Discharging = 2,
    Empty = 3,
    FullyCharged = 4,
    PendingCharge = 5,
    PendingDischarge = 6,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    serde_repr::Deserialize_repr,
    serde_repr::Serialize_repr,
    zvariant::Type,
    zvariant::OwnedValue,
)]
#[repr(u32)]
pub enum Technology {
    Unknown = 0,
    LithiumIon = 1,
    LithiumPolymer = 2,
    LithiumIronPhosphate = 3,
    LeadAcid = 4,
    NickelCadmium = 5,
    NickelMetalHydride = 6,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    serde_repr::Deserialize_repr,
    serde_repr::Serialize_repr,
    zvariant::Type,
    zvariant::OwnedValue,
)]
#[repr(u32)]
pub enum WarningLevel {
    Unknown = 0,
    None = 1,
    Discharging = 2,
    Low = 3,
    Critical = 4,
    Action = 5,
}

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    serde_repr::Deserialize_repr,
    serde_repr::Serialize_repr,
    zvariant::Type,
    zvariant::OwnedValue,
)]
#[repr(u32)]
pub enum Type {
    Unknown = 0,
    LinePower = 1,
    Battery = 2,
    Ups = 3,
    Monitor = 4,
    Mouse = 5,
    Keyboard = 6,
    Pda = 7,
    Phone = 8,
    MediaPlayer = 9,
    Tablet = 10,
    Computer = 11,
    GamingInput = 12,
    Pen = 13,
    Touchpad = 14,
    Modem = 15,
    Network = 16,
    Headset = 17,
    Speakers = 18,
    Headphones = 19,
    Video = 20,
    OtherAudio = 21,
    RemoteControl = 22,
    Printer = 23,
    Scanner = 24,
    Camera = 25,
    Wearable = 26,
    Toy = 27,
    BluetoothGeneric = 28,
}

#[proxy(
    interface = "org.freedesktop.UPower.Device",
    default_service = "org.freedesktop.UPower"
)]
pub trait Device {
    /// EnableChargeThreshold method
    fn enable_charge_threshold(&self, charge_threshold: bool) -> zbus::Result<()>;

    /// GetHistory method
    fn get_history(
        &self,
        type_: &str,
        timespan: u32,
        resolution: u32,
    ) -> zbus::Result<Vec<(u32, f64, u32)>>;

    /// GetStatistics method
    fn get_statistics(&self, type_: &str) -> zbus::Result<Vec<(f64, f64)>>;

    /// Refresh method
    fn refresh(&self) -> zbus::Result<()>;

    /// BatteryLevel property
    #[zbus(property)]
    fn battery_level(&self) -> zbus::Result<u32>;

    /// Capacity property
    #[zbus(property)]
    fn capacity(&self) -> zbus::Result<f64>;

    /// ChargeCycles property
    #[zbus(property)]
    fn charge_cycles(&self) -> zbus::Result<i32>;

    /// ChargeEndThreshold property
    #[zbus(property)]
    fn charge_end_threshold(&self) -> zbus::Result<u32>;

    /// ChargeStartThreshold property
    #[zbus(property)]
    fn charge_start_threshold(&self) -> zbus::Result<u32>;

    /// ChargeThresholdEnabled property
    #[zbus(property)]
    fn charge_threshold_enabled(&self) -> zbus::Result<bool>;

    /// ChargeThresholdSupported property
    #[zbus(property)]
    fn charge_threshold_supported(&self) -> zbus::Result<bool>;

    /// Energy property
    #[zbus(property)]
    fn energy(&self) -> zbus::Result<f64>;

    /// EnergyEmpty property
    #[zbus(property)]
    fn energy_empty(&self) -> zbus::Result<f64>;

    /// EnergyFull property
    #[zbus(property)]
    fn energy_full(&self) -> zbus::Result<f64>;

    /// EnergyFullDesign property
    #[zbus(property)]
    fn energy_full_design(&self) -> zbus::Result<f64>;

    /// EnergyRate property
    #[zbus(property)]
    fn energy_rate(&self) -> zbus::Result<f64>;

    /// HasHistory property
    #[zbus(property)]
    fn has_history(&self) -> zbus::Result<bool>;

    /// HasStatistics property
    #[zbus(property)]
    fn has_statistics(&self) -> zbus::Result<bool>;

    /// IconName property
    #[zbus(property)]
    fn icon_name(&self) -> zbus::Result<String>;

    /// IsPresent property
    #[zbus(property)]
    fn is_present(&self) -> zbus::Result<bool>;

    /// IsRechargeable property
    #[zbus(property)]
    fn is_rechargeable(&self) -> zbus::Result<bool>;

    /// Luminosity property
    #[zbus(property)]
    fn luminosity(&self) -> zbus::Result<f64>;

    /// Model property
    #[zbus(property)]
    fn model(&self) -> zbus::Result<String>;

    /// NativePath property
    #[zbus(property)]
    fn native_path(&self) -> zbus::Result<String>;

    /// Online property
    #[zbus(property)]
    fn online(&self) -> zbus::Result<bool>;

    /// Percentage property
    #[zbus(property)]
    fn percentage(&self) -> zbus::Result<f64>;

    /// PowerSupply property
    #[zbus(property)]
    fn power_supply(&self) -> zbus::Result<bool>;

    /// Serial property
    #[zbus(property)]
    fn serial(&self) -> zbus::Result<String>;

    /// State property
    #[zbus(property)]
    fn state(&self) -> zbus::Result<State>;

    /// Technology property
    #[zbus(property)]
    fn technology(&self) -> zbus::Result<Technology>;

    /// Temperature property
    #[zbus(property)]
    fn temperature(&self) -> zbus::Result<f64>;

    /// TimeToEmpty property
    #[zbus(property)]
    fn time_to_empty(&self) -> zbus::Result<i64>;

    /// TimeToFull property
    #[zbus(property)]
    fn time_to_full(&self) -> zbus::Result<i64>;

    /// Type property
    #[zbus(property)]
    fn type_(&self) -> zbus::Result<Type>;

    /// UpdateTime property
    #[zbus(property)]
    fn update_time(&self) -> zbus::Result<u64>;

    /// Vendor property
    #[zbus(property)]
    fn vendor(&self) -> zbus::Result<String>;

    /// Voltage property
    #[zbus(property)]
    fn voltage(&self) -> zbus::Result<f64>;

    /// WarningLevel property
    #[zbus(property)]
    fn warning_level(&self) -> zbus::Result<WarningLevel>;
}
