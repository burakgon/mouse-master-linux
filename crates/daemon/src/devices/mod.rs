//! Device management: hidraw nodes, receivers, and one actor task per device.

pub mod actor;
pub mod node;
pub mod snapshot;

use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use parking_lot::RwLock;
use serde::Deserialize;
use tokio::sync::{broadcast, mpsc, oneshot, watch};

use crate::actions::Action;
use crate::actions::exec::Executor;
use crate::config::{ConfigStore, ThumbwheelBinding};
use crate::notify::Notifier;
use crate::window::ActiveApp;
use snapshot::DeviceSnapshot;

/// Events published to clients.
#[derive(Debug, Clone)]
pub enum DaemonEvent {
    /// Devices were added or removed.
    DevicesChanged,
    /// A device's snapshot changed.
    DeviceChanged(String),
    /// A diverted control was pressed or released.
    Button {
        device: String,
        control: String,
        pressed: bool,
    },
    /// Raw motion while a gesture button is held (only while monitoring).
    Motion {
        device: String,
        dx: i32,
        dy: i32,
    },
    /// A gesture was recognised.
    Gesture {
        device: String,
        control: String,
        gesture: String,
    },
    ActiveApp(ActiveApp),
}

/// Requests handled by a device actor.
#[derive(Debug, Deserialize)]
#[serde(tag = "command", rename_all = "kebab-case")]
pub enum Command {
    /// Partial update of the hardware settings (keys of `DeviceSettings`).
    SetSettings {
        settings: serde_json::Value,
    },
    /// Sets (or with `action: null` removes) a button binding. `profile: null` is the default profile.
    SetBinding {
        profile: Option<String>,
        control: String,
        action: Option<Action>,
    },
    SetThumbwheel {
        profile: Option<String>,
        binding: Option<ThumbwheelBinding>,
    },
    SetThumbwheelTap {
        profile: Option<String>,
        action: Option<Action>,
    },
    /// Restores the default bindings of a profile.
    ResetBindings {
        profile: Option<String>,
    },
    AddProfile {
        name: String,
        apps: Vec<String>,
    },
    UpdateProfile {
        id: String,
        name: Option<String>,
        apps: Option<Vec<String>>,
        enabled: Option<bool>,
        dpi: Option<Option<u16>>,
    },
    RemoveProfile {
        id: String,
    },
    SwitchHost {
        host: u8,
    },
    SetHostName {
        host: u8,
        name: String,
    },
    PlayHaptic {
        waveform: u8,
    },
    /// Runs an action as if triggered by this device (to try it out).
    RunAction {
        action: Action,
    },
    /// Re-reads everything from the device and applies the configuration again.
    Refresh,
    /// Re-evaluates which application profile applies (after general settings changed).
    Reapply,
}

pub type Reply = Result<serde_json::Value, String>;

#[derive(Debug)]
pub enum DeviceMsg {
    Report(hidpp::Report),
    /// The receiver reported the device going on- or offline.
    Online(bool),
    Command(Command, oneshot::Sender<Reply>),
    /// The node went away.
    Shutdown,
    /// The service is exiting: restore native behaviour, then acknowledge.
    Stop(oneshot::Sender<()>),
    /// A client started or stopped watching live input (press highlighting).
    Monitor(bool),
}

pub type Mailbox = mpsc::UnboundedSender<DeviceMsg>;

struct Entry {
    snapshot: Arc<DeviceSnapshot>,
    mailbox: Option<Mailbox>,
}

/// All devices seen during this session, live or disconnected.
#[derive(Default)]
pub struct Registry {
    entries: RwLock<BTreeMap<String, Entry>>,
}

impl Registry {
    pub fn publish(&self, snapshot: DeviceSnapshot, mailbox: Option<Mailbox>) -> bool {
        let id = snapshot.id.clone();
        let mut entries = self.entries.write();
        let is_new = !entries.contains_key(&id);
        let mailbox = mailbox.or_else(|| entries.get(&id).and_then(|e| e.mailbox.clone()));
        entries.insert(id, Entry { snapshot: Arc::new(snapshot), mailbox });
        is_new
    }

    /// Marks a device as disconnected, keeping it listed.
    pub fn detach(&self, id: &str, mailbox: &Mailbox) {
        let mut entries = self.entries.write();
        if let Some(entry) = entries.get_mut(id) {
            if entry.mailbox.as_ref().is_some_and(|m| m.same_channel(mailbox)) {
                entry.mailbox = None;
                let mut snap = (*entry.snapshot).clone();
                snap.online = false;
                snap.busy = false;
                entry.snapshot = Arc::new(snap);
            }
        }
    }

    pub fn snapshots(&self) -> Vec<Arc<DeviceSnapshot>> {
        self.entries.read().values().map(|e| e.snapshot.clone()).collect()
    }

    pub fn snapshot(&self, id: &str) -> Option<Arc<DeviceSnapshot>> {
        self.entries.read().get(id).map(|e| e.snapshot.clone())
    }

    pub fn mailbox(&self, id: &str) -> Option<Mailbox> {
        self.entries.read().get(id).and_then(|e| e.mailbox.clone())
    }

    pub fn remove(&self, id: &str) -> bool {
        self.entries.write().remove(id).is_some()
    }

    pub fn mailboxes(&self) -> Vec<Mailbox> {
        self.entries.read().values().filter_map(|e| e.mailbox.clone()).collect()
    }
}

/// State shared by all device actors and the D-Bus service.
pub struct Shared {
    pub config: ConfigStore,
    pub executor: Executor,
    pub notifier: Notifier,
    pub registry: Registry,
    pub events: broadcast::Sender<DaemonEvent>,
    pub active_app: watch::Receiver<ActiveApp>,
    /// False while another user's session has the seat.
    pub present: watch::Receiver<bool>,
    /// A client asked for live motion events (gesture preview).
    pub monitoring: AtomicBool,
}

impl Shared {
    pub fn emit(&self, event: DaemonEvent) {
        let _ = self.events.send(event);
    }

    pub fn monitoring(&self) -> bool {
        self.monitoring.load(Ordering::Relaxed)
    }

    /// Sends a command to a device and waits for the reply.
    pub async fn command(&self, id: &str, command: Command) -> Reply {
        let mailbox = self.registry.mailbox(id).ok_or_else(|| format!("device {id} is not connected"))?;
        let (tx, rx) = oneshot::channel();
        mailbox.send(DeviceMsg::Command(command, tx)).map_err(|_| "device went away".to_string())?;
        rx.await.map_err(|_| "device went away".to_string())?
    }
}
