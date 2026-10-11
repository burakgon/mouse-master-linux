//! The session bus API used by the app and the command line tool.
//!
//! Complex values travel as JSON strings: the data model evolves faster than a
//! D-Bus type signature should, and every client (QML, Rust, shell) handles JSON.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use tokio::sync::{broadcast, watch};
use tracing::{debug, warn};
use zbus::fdo;
use zbus::object_server::SignalEmitter;

use crate::actions::Action;
use crate::actions::catalog;
use crate::config::{Config, General};
use crate::devices::{Command, DaemonEvent, Shared};
use crate::env::Desktop;
use crate::window::ActiveApp;

pub const BUS_NAME: &str = "io.github.burakgon.MouseMaster.Daemon";
pub const OBJECT_PATH: &str = "/io/github/burakgon/MouseMaster/Daemon";
pub const INTERFACE: &str = "io.github.burakgon.MouseMaster.Daemon1";
pub const TRACKER_INTERFACE: &str = "io.github.burakgon.MouseMaster.WindowTracker1";

pub struct DaemonIface {
    pub shared: Arc<Shared>,
    pub desktop: watch::Receiver<Desktop>,
    pub input_available: bool,
}

fn json<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "null".into())
}

fn bad(msg: impl std::fmt::Display) -> fdo::Error {
    fdo::Error::InvalidArgs(msg.to_string())
}

#[zbus::interface(name = "io.github.burakgon.MouseMaster.Daemon1")]
impl DaemonIface {
    #[zbus(property)]
    fn version(&self) -> &str {
        env!("CARGO_PKG_VERSION")
    }

    #[zbus(property)]
    fn desktop(&self) -> String {
        json(&*self.desktop.borrow()).trim_matches('"').to_string()
    }

    /// False when /dev/uinput is not accessible: actions other than device
    /// settings will not work.
    #[zbus(property)]
    fn input_available(&self) -> bool {
        self.input_available
    }

    /// JSON array of device snapshots.
    fn list_devices(&self) -> String {
        let snapshots = self.shared.registry.snapshots();
        json(&snapshots.iter().map(|s| s.as_ref()).collect::<Vec<_>>())
    }

    fn get_device(&self, id: &str) -> fdo::Result<String> {
        self.shared.registry.snapshot(id).map(|s| json(&*s)).ok_or_else(|| bad(format!("no device {id}")))
    }

    /// Runs a device command (see `devices::Command`), returns a JSON result.
    async fn command(&self, id: &str, command: &str) -> fdo::Result<String> {
        let command: Command = serde_json::from_str(command).map_err(|e| bad(format!("invalid command: {e}")))?;
        debug!("command for {id}: {command:?}");
        match self.shared.command(id, command).await {
            Ok(v) => Ok(json(&v)),
            Err(e) => Err(fdo::Error::Failed(e)),
        }
    }

    /// Built-in actions available on the current desktop.
    fn get_catalog(&self) -> String {
        json(&catalog::catalog(*self.desktop.borrow()))
    }

    fn get_config(&self) -> String {
        json(&self.shared.config.get())
    }

    /// Replaces the whole configuration (import).
    fn set_config(&self, config: &str) -> fdo::Result<()> {
        let parsed: Config = serde_json::from_str(config)
            .or_else(|_| toml::from_str(config))
            .map_err(|e| bad(format!("invalid configuration: {e}")))?;
        self.shared.config.replace(parsed);
        for mailbox in self.shared.registry.mailboxes() {
            let (tx, _rx) = tokio::sync::oneshot::channel();
            let _ = mailbox.send(crate::devices::DeviceMsg::Command(Command::Refresh, tx));
        }
        Ok(())
    }

    fn get_general(&self) -> String {
        json(&self.shared.config.general())
    }

    /// Partial update of the general settings.
    fn set_general(&self, patch: &str) -> fdo::Result<()> {
        let patch: serde_json::Value = serde_json::from_str(patch).map_err(bad)?;
        let mut current = serde_json::to_value(self.shared.config.general()).map_err(bad)?;
        let (Some(obj), Some(p)) = (current.as_object_mut(), patch.as_object()) else {
            return Err(bad("expected an object"));
        };
        for (k, v) in p {
            if !obj.contains_key(k) {
                return Err(bad(format!("unknown setting {k:?}")));
            }
            obj.insert(k.clone(), v.clone());
        }
        let general: General = serde_json::from_value(current).map_err(bad)?;
        self.shared.config.update(|c| c.general = general);
        // Profile matching depends on `app_profiles`.
        for mailbox in self.shared.registry.mailboxes() {
            let (tx, _rx) = tokio::sync::oneshot::channel();
            let _ = mailbox.send(crate::devices::DeviceMsg::Command(Command::Reapply, tx));
        }
        Ok(())
    }

    /// Removes a disconnected device from the list and forgets its settings.
    fn forget_device(&self, id: &str) -> fdo::Result<()> {
        if self.shared.registry.mailbox(id).is_some() {
            return Err(fdo::Error::Failed("the device is connected".into()));
        }
        self.shared.registry.remove(id);
        self.shared.config.update(|c| c.devices.remove(id));
        self.shared.emit(DaemonEvent::DevicesChanged);
        Ok(())
    }

    /// Enables live input signals: presses of every control and raw motion
    /// while a gesture button is held (used by the app while it is visible).
    /// Monitoring stops by itself when the caller leaves the bus.
    async fn set_monitoring(
        &self,
        enabled: bool,
        #[zbus(header)] header: zbus::message::Header<'_>,
        #[zbus(connection)] conn: &zbus::Connection,
    ) {
        set_monitoring(&self.shared, enabled);
        if enabled {
            if let Some(sender) = header.sender().map(|s| s.to_owned()) {
                let (shared, conn) = (self.shared.clone(), conn.clone());
                tokio::spawn(async move {
                    use futures::StreamExt;
                    let Ok(proxy) = zbus::fdo::DBusProxy::new(&conn).await else { return };
                    let Ok(mut changes) = proxy.receive_name_owner_changed().await else { return };
                    // The client may already be gone.
                    if !proxy.name_has_owner(sender.clone().into()).await.unwrap_or(false) {
                        set_monitoring(&shared, false);
                        return;
                    }
                    while let Some(signal) = changes.next().await {
                        if let Ok(args) = signal.args() {
                            if args.name().as_str() == sender.as_str() && args.new_owner().is_none() {
                                set_monitoring(&shared, false);
                                return;
                            }
                        }
                        if !shared.monitoring() {
                            return;
                        }
                    }
                });
            }
        }
    }

    /// Runs an action that does not need a device (to try it out).
    fn run_action(&self, action: &str) -> fdo::Result<()> {
        let action: Action = serde_json::from_str(action).map_err(bad)?;
        let _ = self.shared.executor.run(&action);
        Ok(())
    }

    #[zbus(property)]
    fn active_app(&self) -> String {
        json(&*self.shared.active_app.borrow())
    }

    #[zbus(signal)]
    async fn devices_changed(emitter: &SignalEmitter<'_>) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn device_changed(emitter: &SignalEmitter<'_>, id: &str, snapshot: &str) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn button_event(emitter: &SignalEmitter<'_>, id: &str, control: &str, pressed: bool) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn motion_event(emitter: &SignalEmitter<'_>, id: &str, dx: i32, dy: i32) -> zbus::Result<()>;

    #[zbus(signal)]
    async fn gesture_event(emitter: &SignalEmitter<'_>, id: &str, control: &str, gesture: &str) -> zbus::Result<()>;

    /// The focused application changed (JSON `ActiveApp`).
    #[zbus(signal)]
    async fn focus_changed(emitter: &SignalEmitter<'_>, app: &str) -> zbus::Result<()>;
}

fn set_monitoring(shared: &Shared, enabled: bool) {
    let was = shared.monitoring.swap(enabled, Ordering::Relaxed);
    if was != enabled {
        for mailbox in shared.registry.mailboxes() {
            let _ = mailbox.send(crate::devices::DeviceMsg::Monitor(enabled));
        }
    }
}

/// Receives focus changes from the KWin script.
pub struct TrackerIface {
    pub app_tx: watch::Sender<ActiveApp>,
}

#[zbus::interface(name = "io.github.burakgon.MouseMaster.WindowTracker1")]
impl TrackerIface {
    fn activated(&self, resource_class: &str, desktop_file: &str, caption: &str, pid: i32) {
        let app = ActiveApp {
            app_id: resource_class.to_string(),
            desktop_file: desktop_file.to_string(),
            title: caption.to_string(),
            pid,
        };
        self.app_tx.send_if_modified(|current| {
            if current.app_id != app.app_id || current.desktop_file != app.desktop_file {
                *current = app;
                true
            } else {
                false
            }
        });
    }
}

/// Forwards internal events to D-Bus signals.
pub async fn pump_events(conn: zbus::Connection, shared: Arc<Shared>, mut events: broadcast::Receiver<DaemonEvent>) {
    let iface = match conn.object_server().interface::<_, DaemonIface>(OBJECT_PATH).await {
        Ok(i) => i,
        Err(e) => {
            warn!("cannot emit signals: {e}");
            return;
        }
    };
    let emitter = iface.signal_emitter().clone();
    loop {
        let event = match events.recv().await {
            Ok(e) => e,
            Err(broadcast::error::RecvError::Lagged(n)) => {
                debug!("signal pump lagged by {n} events");
                let _ = DaemonIface::devices_changed(&emitter).await;
                continue;
            }
            Err(broadcast::error::RecvError::Closed) => return,
        };
        let result = match event {
            DaemonEvent::DevicesChanged => DaemonIface::devices_changed(&emitter).await,
            DaemonEvent::DeviceChanged(id) => match shared.registry.snapshot(&id) {
                Some(snap) => DaemonIface::device_changed(&emitter, &id, &json(&*snap)).await,
                None => Ok(()),
            },
            DaemonEvent::Button { device, control, pressed } => {
                DaemonIface::button_event(&emitter, &device, &control, pressed).await
            }
            DaemonEvent::Motion { device, dx, dy } => DaemonIface::motion_event(&emitter, &device, dx, dy).await,
            DaemonEvent::Gesture { device, control, gesture } => {
                DaemonIface::gesture_event(&emitter, &device, &control, &gesture).await
            }
            DaemonEvent::ActiveApp(app) => {
                let r = DaemonIface::focus_changed(&emitter, &json(&app)).await;
                let _ = iface.get().await.active_app_changed(&emitter).await;
                r
            }
        };
        if let Err(e) = result {
            debug!("signal emission failed: {e}");
        }
    }
}
