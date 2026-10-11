//! Desktop notifications and on screen displays.

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::Mutex;
use tokio::sync::watch;
use tracing::debug;
use zbus::zvariant::Value;

use crate::env::Desktop;

#[derive(Clone)]
pub struct Notifier {
    conn: Option<zbus::Connection>,
    desktop: watch::Receiver<Desktop>,
    /// Notification ids by key, so updates replace the previous bubble.
    ids: Arc<Mutex<HashMap<String, u32>>>,
}

impl Notifier {
    pub fn new(conn: Option<zbus::Connection>, desktop: watch::Receiver<Desktop>) -> Notifier {
        Notifier { conn, desktop, ids: Arc::new(Mutex::new(HashMap::new())) }
    }

    /// Shows a notification. Notifications with the same `key` replace each other.
    pub async fn notify(&self, key: &str, summary: &str, body: &str, icon: &str, timeout_ms: i32, urgent: bool) {
        let Some(conn) = &self.conn else { return };
        let replaces = self.ids.lock().get(key).copied().unwrap_or(0);
        let mut hints: HashMap<&str, Value<'_>> = HashMap::new();
        hints.insert("desktop-entry", Value::from("io.github.burakgon.MouseMaster"));
        hints.insert("urgency", Value::from(if urgent { 2u8 } else { 1u8 }));
        if timeout_ms > 0 && timeout_ms < 3000 {
            hints.insert("transient", Value::from(true));
        }
        let actions: Vec<&str> = Vec::new();
        let reply = conn
            .call_method(
                Some("org.freedesktop.Notifications"),
                "/org/freedesktop/Notifications",
                Some("org.freedesktop.Notifications"),
                "Notify",
                &("Mouse Master", replaces, icon, summary, body, actions, hints, timeout_ms),
            )
            .await;
        match reply {
            Ok(msg) => {
                if let Ok(id) = msg.body().deserialize::<u32>() {
                    self.ids.lock().insert(key.to_string(), id);
                }
            }
            Err(e) => debug!("notification failed: {e}"),
        }
    }

    /// A short on screen display (DPI changes, wheel mode, ...).
    pub async fn osd(&self, icon: &str, text: &str) {
        let Some(conn) = &self.conn else { return };
        if *self.desktop.borrow() == Desktop::Kde {
            let ok = conn
                .call_method(
                    Some("org.kde.plasmashell"),
                    "/org/kde/osdService",
                    Some("org.kde.osdService"),
                    "showText",
                    &(icon, text),
                )
                .await
                .is_ok();
            if ok {
                return;
            }
        }
        self.notify("osd", text, "", icon, 1500, false).await;
    }
}
