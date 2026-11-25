// SPDX-License-Identifier: MPL-2.0

//! Holds a systemd-logind inhibitor lock for as long as it is alive.

/// A logind inhibitor lock. Dropping it closes the lock file descriptor,
/// which is how logind releases the inhibit.
#[derive(Debug)]
pub struct InhibitLock {
    _fd: zbus::OwnedFd,
}

/// Blocks logind's suspend, lid-switch, and idle handling until the lock is dropped.
pub async fn acquire() -> zbus::Result<InhibitLock> {
    let connection = zbus::Connection::system().await?;
    let proxy = zbus::Proxy::new(
        &connection,
        "org.freedesktop.login1",
        "/org/freedesktop/login1",
        "org.freedesktop.login1.Manager",
    )
    .await?;

    let _fd: zbus::OwnedFd = proxy
        .call(
            "Inhibit",
            &(
                "sleep:handle-lid-switch:idle",
                "cosmic-applet-stay-awake",
                "Stay-awake applet active",
                "block",
            ),
        )
        .await?;

    Ok(InhibitLock { _fd })
}
