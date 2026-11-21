// SPDX-License-Identifier: MPL-2.0

//! Powers the internal panel off while the lid is closed.

use cosmic::iced::futures::StreamExt;

use crate::bluetooth;

const FALLBACK_OUTPUT: &str = "eDP-1";

/// Starts the lid watcher. Dropping the handle does not stop it; abort it to stop.
pub fn spawn() -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        if let Err(why) = watch_lid().await {
            eprintln!("lid watcher stopped: {why}");
        }
    })
}

/// Re-enables the internal panel, for when the applet is turned off while the lid is closed.
pub fn enable_internal() {
    tokio::spawn(async move {
        set_panel_power(&internal_output().await, true).await;
    });
}

async fn ensure_bluetooth_powered() {
    if let Err(why) = bluetooth::ensure_powered().await {
        eprintln!("failed to power on bluetooth: {why}");
    }
}

async fn watch_lid() -> zbus::Result<()> {
    // Covers the lid already being closed when the applet is toggled on.
    ensure_bluetooth_powered().await;

    let connection = zbus::Connection::system().await?;
    let proxy = zbus::Proxy::new(
        &connection,
        "org.freedesktop.UPower",
        "/org/freedesktop/UPower",
        "org.freedesktop.UPower",
    )
    .await?;

    let output = internal_output().await;
    let mut lid_closed = proxy.receive_property_changed::<bool>("LidIsClosed").await;
    while let Some(change) = lid_closed.next().await {
        match change.get().await {
            Ok(closed) => {
                set_panel_power(&output, !closed).await;
                if closed {
                    // The system's own soft-block lands on lid close, so run after it.
                    tokio::spawn(async {
                        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                        ensure_bluetooth_powered().await;
                    });
                }
            }
            Err(why) => eprintln!("failed to read UPower LidIsClosed: {why}"),
        }
    }

    Ok(())
}

async fn internal_output() -> String {
    match tokio::process::Command::new("cosmic-randr")
        .arg("list")
        .output()
        .await
    {
        Ok(list) if list.status.success() => {
            parse_internal_output(&String::from_utf8_lossy(&list.stdout))
                .unwrap_or_else(|| FALLBACK_OUTPUT.to_string())
        }
        Ok(list) => {
            eprintln!("cosmic-randr list failed: {}", list.status);
            FALLBACK_OUTPUT.to_string()
        }
        Err(why) => {
            eprintln!("failed to run cosmic-randr list: {why}");
            FALLBACK_OUTPUT.to_string()
        }
    }
}

async fn set_panel_power(output: &str, on: bool) {
    let action = if on { "enable" } else { "disable" };
    match tokio::process::Command::new("cosmic-randr")
        .args([action, output])
        .status()
        .await
    {
        Ok(status) if !status.success() => {
            eprintln!("cosmic-randr {action} {output} failed: {status}");
        }
        Err(why) => eprintln!("failed to run cosmic-randr {action} {output}: {why}"),
        Ok(_) => {}
    }
}

/// Parses the internal panel's output name out of `cosmic-randr list` output.
fn parse_internal_output(list: &str) -> Option<String> {
    list.lines()
        .map(strip_ansi)
        .filter_map(|line| line.split_whitespace().next().map(str::to_owned))
        .find(|name| name.starts_with("eDP-") || name.starts_with("LVDS-"))
}

/// Removes the CSI color sequences that `cosmic-randr` uses in its output.
fn strip_ansi(text: &str) -> String {
    let mut stripped = String::with_capacity(text.len());
    let mut chars = text.chars();
    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // Consume the `[` and everything up to the final byte of the CSI sequence.
            if chars.next() == Some('[') {
                for c in chars.by_ref() {
                    if ('@'..='~').contains(&c) {
                        break;
                    }
                }
            }
        } else {
            stripped.push(c);
        }
    }
    stripped
}

#[cfg(test)]
mod tests {
    use super::parse_internal_output;

    #[test]
    fn selects_internal_panel() {
        let list = "\x1b[1mHDMI-A-1\x1b[0m (enabled)\n\x1b[1meDP-1\x1b[0m (enabled)\n  Modes:\n";
        assert_eq!(parse_internal_output(list).as_deref(), Some("eDP-1"));
    }
}
