//! Tracking the focused application, for per-application profiles.
//!
//! Wayland deliberately hides this from ordinary clients, so each compositor
//! needs its own backend:
//!
//! * KDE Plasma: a tiny KWin script reports activations back over D-Bus,
//! * Hyprland: the event socket (`activewindow>>class,title`),
//! * Sway: i3 IPC window events.

use std::time::Duration;

use serde::Serialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::watch;
use tracing::{debug, info, warn};

use crate::dbus::{BUS_NAME, OBJECT_PATH, TRACKER_INTERFACE};
use crate::env::{self, Desktop};
use crate::hyprland;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ActiveApp {
    /// Wayland app id, X11 class or KWin resource class.
    pub app_id: String,
    /// Desktop file id, when known.
    pub desktop_file: String,
    pub title: String,
    pub pid: i32,
}

impl ActiveApp {
    /// Case insensitive match against the app id or desktop file id. A pattern
    /// also matches the last component of a reverse DNS id, so `firefox`
    /// matches `org.mozilla.firefox`.
    pub fn matches(&self, pattern: &str) -> bool {
        let pattern = pattern.trim().trim_end_matches(".desktop").to_ascii_lowercase();
        if pattern.is_empty() {
            return false;
        }
        [&self.app_id, &self.desktop_file].iter().any(|candidate| {
            let c = candidate.trim_end_matches(".desktop").to_ascii_lowercase();
            !c.is_empty() && (c == pattern || c.rsplit('.').next() == Some(pattern.as_str()) || pattern.rsplit('.').next() == Some(c.as_str()))
        })
    }
}

const KWIN_SCRIPT_NAME: &str = "mouse-master-window-tracker";

fn kwin_script() -> String {
    format!(
        r#"// Reports the active window to Mouse Master. Installed and removed automatically.
function report(w) {{
    callDBus("{BUS_NAME}", "{OBJECT_PATH}", "{TRACKER_INTERFACE}", "Activated",
        w ? String(w.resourceClass || "") : "",
        w ? String(w.desktopFileName || "") : "",
        w ? String(w.caption || "") : "",
        w ? Number(w.pid || 0) : 0);
}}
if (workspace.windowActivated) {{
    workspace.windowActivated.connect(report);
    report(workspace.activeWindow);
}} else {{
    workspace.clientActivated.connect(report);
    report(workspace.activeClient);
}}
"#
    )
}

async fn kwin_load_script(conn: &zbus::Connection) -> zbus::Result<()> {
    let dir = env::runtime_dir().join("mouse-master");
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join("kwin-window-tracker.js");
    std::fs::write(&path, kwin_script()).map_err(|e| zbus::Error::Failure(e.to_string()))?;
    let scripting = |method: &'static str| (Some("org.kde.KWin"), "/Scripting", Some("org.kde.kwin.Scripting"), method);
    let (dest, path_s, iface, _) = scripting("unloadScript");
    let _ = conn.call_method(dest, path_s, iface, "unloadScript", &(KWIN_SCRIPT_NAME,)).await;
    let reply = conn
        .call_method(dest, path_s, iface, "loadScript", &(path.to_string_lossy().as_ref(), KWIN_SCRIPT_NAME))
        .await?;
    let id: i32 = reply.body().deserialize()?;
    if id < 0 {
        return Err(zbus::Error::Failure("KWin refused to load the script".into()));
    }
    // Plasma 6 exposes scripts at /Scripting/Script<id>; Plasma 5 at /<id>.
    for script_path in [format!("/Scripting/Script{id}"), format!("/{id}")] {
        let ok = conn.call_method(dest, script_path.as_str(), Some("org.kde.kwin.Script"), "run", &()).await.is_ok();
        if ok {
            info!("KWin window tracker loaded");
            return Ok(());
        }
    }
    // Fall back to starting all loaded scripts.
    conn.call_method(dest, path_s, iface, "start", &()).await?;
    info!("KWin window tracker loaded");
    Ok(())
}

pub async fn kwin_unload_script(conn: &zbus::Connection) {
    let _ = conn
        .call_method(Some("org.kde.KWin"), "/Scripting", Some("org.kde.kwin.Scripting"), "unloadScript", &(KWIN_SCRIPT_NAME,))
        .await;
}

/// Runs the tracker for whatever desktop is active, re-detecting when the
/// backend goes away (compositor restart, user switching sessions).
pub async fn run(conn: zbus::Connection, desktop_tx: watch::Sender<Desktop>, app_tx: watch::Sender<ActiveApp>) {
    loop {
        let desktop = env::detect(&conn).await;
        desktop_tx.send_if_modified(|d| {
            if *d != desktop {
                info!("desktop: {}", desktop.name());
                *d = desktop;
                true
            } else {
                false
            }
        });
        match desktop {
            Desktop::Kde => {
                if let Err(e) = kwin_load_script(&conn).await {
                    warn!("could not load the KWin window tracker: {e}");
                    tokio::time::sleep(Duration::from_secs(10)).await;
                    continue;
                }
                // Activations arrive through D-Bus; wait until KWin goes away.
                wait_name_vanished(&conn, "org.kde.KWin").await;
            }
            Desktop::Hyprland => {
                if let Err(e) = hyprland_events(&app_tx).await {
                    debug!("hyprland event socket closed: {e}");
                }
            }
            Desktop::Sway => {
                if let Err(e) = sway_events(&app_tx).await {
                    debug!("sway IPC closed: {e}");
                }
            }
            Desktop::Gnome | Desktop::Other => {
                // No generic way to observe focus here; check again later.
                tokio::time::sleep(Duration::from_secs(30)).await;
            }
        }
        tokio::time::sleep(Duration::from_secs(2)).await;
    }
}

async fn wait_name_vanished(conn: &zbus::Connection, name: &str) {
    use futures::StreamExt;
    let Ok(proxy) = zbus::fdo::DBusProxy::new(conn).await else { return };
    let Ok(mut changes) = proxy.receive_name_owner_changed().await else { return };
    while let Some(signal) = changes.next().await {
        if let Ok(args) = signal.args() {
            if args.name().as_str() == name && args.new_owner().is_none() {
                return;
            }
        }
    }
}

async fn hyprland_events(app_tx: &watch::Sender<ActiveApp>) -> std::io::Result<()> {
    let instance = hyprland::Instance::find().ok_or_else(|| std::io::Error::other("no Hyprland instance"))?;
    // Initial state.
    if let Ok(json) = instance.request("j/activewindow").await {
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&json) {
            let app = ActiveApp {
                app_id: v["class"].as_str().unwrap_or_default().to_string(),
                desktop_file: v["initialClass"].as_str().unwrap_or_default().to_string(),
                title: v["title"].as_str().unwrap_or_default().to_string(),
                pid: v["pid"].as_i64().unwrap_or(0) as i32,
            };
            app_tx.send_replace(app);
        }
    }
    let mut lines = instance.events().await?;
    info!("Hyprland window tracker connected");
    while let Some(line) = lines.next_line().await? {
        if let Some(data) = line.strip_prefix("activewindow>>") {
            let (class, title) = data.split_once(',').unwrap_or((data, ""));
            let app = ActiveApp { app_id: class.to_string(), desktop_file: String::new(), title: title.to_string(), pid: 0 };
            app_tx.send_if_modified(|current| {
                if current.app_id != app.app_id || current.title != app.title {
                    *current = app;
                    true
                } else {
                    false
                }
            });
        }
    }
    Ok(())
}

async fn sway_events(app_tx: &watch::Sender<ActiveApp>) -> std::io::Result<()> {
    let path = std::env::var("SWAYSOCK").map_err(|_| std::io::Error::other("SWAYSOCK not set"))?;
    let mut stream = tokio::net::UnixStream::connect(path).await?;
    async fn send(stream: &mut tokio::net::UnixStream, kind: u32, payload: &str) -> std::io::Result<()> {
        let mut msg = b"i3-ipc".to_vec();
        msg.extend_from_slice(&(payload.len() as u32).to_ne_bytes());
        msg.extend_from_slice(&kind.to_ne_bytes());
        msg.extend_from_slice(payload.as_bytes());
        stream.write_all(&msg).await
    }
    send(&mut stream, 2, r#"["window"]"#).await?;
    info!("Sway window tracker connected");
    loop {
        let mut header = [0u8; 14];
        stream.read_exact(&mut header).await?;
        let len = u32::from_ne_bytes(header[6..10].try_into().unwrap()) as usize;
        let kind = u32::from_ne_bytes(header[10..14].try_into().unwrap());
        let mut payload = vec![0u8; len];
        stream.read_exact(&mut payload).await?;
        if kind != 0x8000_0003 {
            continue;
        }
        let Ok(v) = serde_json::from_slice::<serde_json::Value>(&payload) else { continue };
        if v["change"] != "focus" {
            continue;
        }
        let c = &v["container"];
        let app_id = c["app_id"].as_str().or_else(|| c["window_properties"]["class"].as_str()).unwrap_or_default();
        app_tx.send_replace(ActiveApp {
            app_id: app_id.to_string(),
            desktop_file: String::new(),
            title: c["name"].as_str().unwrap_or_default().to_string(),
            pid: c["pid"].as_i64().unwrap_or(0) as i32,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matching() {
        let app = ActiveApp { app_id: "org.mozilla.firefox".into(), desktop_file: "firefox".into(), ..Default::default() };
        assert!(app.matches("firefox"));
        assert!(app.matches("Firefox.desktop"));
        assert!(app.matches("org.mozilla.firefox"));
        assert!(!app.matches("fire"));
        let kde = ActiveApp { app_id: "dolphin".into(), desktop_file: "org.kde.dolphin".into(), ..Default::default() };
        assert!(kde.matches("org.kde.dolphin.desktop"));
        assert!(kde.matches("dolphin"));
        assert!(!ActiveApp::default().matches(""));
    }
}
