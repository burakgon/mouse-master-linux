//! Desktop environment detection and launching programs inside the graphical session.
//!
//! The service usually runs as a systemd user unit, which may start before the
//! compositor and does not necessarily inherit its environment. Everything here
//! is therefore resolved lazily, at the time it is needed.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;

use serde::Serialize;
use tracing::{debug, warn};

use crate::hyprland;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Desktop {
    Kde,
    Hyprland,
    Gnome,
    Sway,
    Other,
}

impl Desktop {
    pub fn name(self) -> &'static str {
        match self {
            Desktop::Kde => "KDE Plasma",
            Desktop::Hyprland => "Hyprland",
            Desktop::Gnome => "GNOME",
            Desktop::Sway => "Sway",
            Desktop::Other => "Desktop",
        }
    }
}

pub fn runtime_dir() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(format!("/run/user/{}", unsafe { libc::getuid() })))
}

async fn name_has_owner(conn: &zbus::Connection, name: &str) -> bool {
    let Ok(proxy) = zbus::fdo::DBusProxy::new(conn).await else { return false };
    let Ok(name) = zbus::names::BusName::try_from(name) else { return false };
    proxy.name_has_owner(name).await.unwrap_or(false)
}

/// Reads the environment of the systemd user manager, which desktops import
/// their session variables (WAYLAND_DISPLAY, XDG_CURRENT_DESKTOP, ...) into.
pub async fn systemd_environment(conn: Option<&zbus::Connection>) -> HashMap<String, String> {
    let mut env = HashMap::new();
    let Some(conn) = conn else { return env };
    let reply = conn
        .call_method(
            Some("org.freedesktop.systemd1"),
            "/org/freedesktop/systemd1",
            Some("org.freedesktop.DBus.Properties"),
            "Get",
            &("org.freedesktop.systemd1.Manager", "Environment"),
        )
        .await;
    if let Ok(msg) = reply {
        if let Ok(value) = msg.body().deserialize::<zbus::zvariant::OwnedValue>() {
            if let Ok(list) = Vec::<String>::try_from(value) {
                for entry in list {
                    if let Some((k, v)) = entry.split_once('=') {
                        env.insert(k.to_string(), v.to_string());
                    }
                }
            }
        }
    }
    env
}

pub async fn detect(conn: &zbus::Connection) -> Desktop {
    if hyprland::Instance::find().is_some() {
        return Desktop::Hyprland;
    }
    if name_has_owner(conn, "org.kde.KWin").await {
        return Desktop::Kde;
    }
    if name_has_owner(conn, "org.gnome.Shell").await {
        return Desktop::Gnome;
    }
    let mut current = std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_default();
    if current.is_empty() {
        current = systemd_environment(Some(conn)).await.remove("XDG_CURRENT_DESKTOP").unwrap_or_default();
    }
    let current = current.to_ascii_lowercase();
    if current.contains("kde") {
        Desktop::Kde
    } else if current.contains("hyprland") {
        Desktop::Hyprland
    } else if current.contains("gnome") {
        Desktop::Gnome
    } else if current.contains("sway") || std::env::var_os("SWAYSOCK").is_some() {
        Desktop::Sway
    } else {
        Desktop::Other
    }
}

/// Environment for programs started by actions: our own environment, overlaid
/// with the session variables known to the systemd user manager.
pub async fn session_environment(conn: Option<&zbus::Connection>) -> HashMap<String, String> {
    let mut env: HashMap<String, String> = std::env::vars().collect();
    for (k, v) in systemd_environment(conn).await {
        env.insert(k, v);
    }
    env
}

/// Runs a shell command detached from the service.
pub async fn spawn_shell(conn: Option<&zbus::Connection>, desktop: Desktop, command: &str) {
    if desktop == Desktop::Hyprland {
        // Let the compositor start it: correct environment, window rules apply.
        if let Some(instance) = hyprland::Instance::find() {
            match instance.request(&format!("dispatch exec {command}")).await {
                Ok(reply) if reply.trim() == "ok" => return,
                Ok(reply) => warn!("hyprland exec failed: {reply}"),
                Err(e) => warn!("hyprland exec failed: {e}"),
            }
        }
    }
    let env = session_environment(conn).await;
    let mut cmd = tokio::process::Command::new("/bin/sh");
    cmd.arg("-c")
        .arg(command)
        .env_clear()
        .envs(env)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .process_group(0);
    match cmd.spawn() {
        Ok(mut child) => {
            debug!("started: {command}");
            tokio::spawn(async move {
                let _ = child.wait().await;
            });
        }
        Err(e) => warn!("failed to run {command:?}: {e}"),
    }
}

/// Quotes a string for /bin/sh.
pub fn shell_quote(s: &str) -> String {
    if !s.is_empty() && s.chars().all(|c| c.is_ascii_alphanumeric() || "-_./:=@%+,".contains(c)) {
        s.to_string()
    } else {
        format!("'{}'", s.replace('\'', r"'\''"))
    }
}

/// Finds a desktop entry by id (`org.kde.dolphin` or `org.kde.dolphin.desktop`).
pub fn find_desktop_file(id: &str) -> Option<PathBuf> {
    let file = if id.ends_with(".desktop") { id.to_string() } else { format!("{id}.desktop") };
    let mut dirs: Vec<PathBuf> = Vec::new();
    if let Some(d) = dirs::data_dir() {
        dirs.push(d);
    }
    let system = std::env::var("XDG_DATA_DIRS").unwrap_or_else(|_| "/usr/local/share:/usr/share".into());
    dirs.extend(system.split(':').filter(|s| !s.is_empty()).map(PathBuf::from));
    dirs.push(PathBuf::from("/var/lib/flatpak/exports/share"));
    if let Some(h) = dirs::home_dir() {
        dirs.push(h.join(".local/share/flatpak/exports/share"));
    }
    dirs.into_iter().map(|d| d.join("applications").join(&file)).find(|p| p.is_file())
}

/// Extracts a runnable command line from a desktop entry's `Exec` key.
pub fn desktop_exec(path: &std::path::Path) -> Option<String> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut in_entry = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with('[') {
            in_entry = line == "[Desktop Entry]";
            continue;
        }
        if in_entry {
            if let Some(exec) = line.strip_prefix("Exec=") {
                // Drop field codes (%f, %U, ...), keep literal percent signs.
                let mut out = String::with_capacity(exec.len());
                let mut chars = exec.chars().peekable();
                while let Some(c) = chars.next() {
                    if c == '%' {
                        match chars.next() {
                            Some('%') => out.push('%'),
                            _ => {}
                        }
                    } else {
                        out.push(c);
                    }
                }
                return Some(out.trim().to_string());
            }
        }
    }
    None
}

/// Opens a URL, path or application id.
pub async fn open(conn: Option<&zbus::Connection>, desktop: Desktop, target: &str) {
    let target = target.trim();
    if target.is_empty() {
        return;
    }
    let looks_like_url = target.contains("://") || target.starts_with("mailto:");
    let expanded = if let Some(rest) = target.strip_prefix("~/") {
        dirs::home_dir().map(|h| h.join(rest).to_string_lossy().into_owned()).unwrap_or_else(|| target.to_string())
    } else {
        target.to_string()
    };
    if !looks_like_url && !expanded.starts_with('/') {
        if let Some(exec) = find_desktop_file(target).and_then(|p| desktop_exec(&p)) {
            spawn_shell(conn, desktop, &exec).await;
            return;
        }
    }
    spawn_shell(conn, desktop, &format!("xdg-open {}", shell_quote(&expanded))).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quoting() {
        assert_eq!(shell_quote("simple"), "simple");
        assert_eq!(shell_quote("it's here"), r"'it'\''s here'");
        assert_eq!(shell_quote("https://example.org/a?b=c"), "'https://example.org/a?b=c'");
    }

    #[test]
    fn exec_field_codes() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("x.desktop");
        std::fs::write(&p, "[Desktop Entry]\nName=X\nExec=foo --bar %U 100%%\n[Desktop Action new]\nExec=other\n").unwrap();
        assert_eq!(desktop_exec(&p).unwrap(), "foo --bar  100%");
    }
}
