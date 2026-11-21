// SPDX-License-Identifier: MPL-2.0

//! Re-powers the Bluetooth adapter that the system soft-blocks when the lid closes.

use zbus::Proxy;

const ADAPTER_PATH: &str = "/org/bluez/hci0";

/// Forces the adapter on. bluez's `AutoEnable` usually beats us to it, but setting
/// `Powered` again republishes the adapter state so the panel icon refreshes.
pub async fn ensure_powered() -> zbus::Result<()> {
    let connection = zbus::Connection::system().await?;
    let proxy = Proxy::new(&connection, "org.bluez", ADAPTER_PATH, "org.bluez.Adapter1").await?;
    proxy.set_property("Powered", true).await?;
    Ok(())
}
