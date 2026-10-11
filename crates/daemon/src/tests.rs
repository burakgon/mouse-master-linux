//! End to end tests: the real device actors driven by simulated hardware.
//!
//! Each test runs the full stack — channel, receiver handling, actor,
//! gestures, executor — against `hidpp::sim`, and records the input the
//! service would have sent to uinput.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use evdev::KeyCode as K;
use hidpp::receiver::ReceiverKind;
use hidpp::sim::{DeviceSpec, SimCommand, SimHandle, Topology};
use serde_json::json;
use tokio::sync::{broadcast, watch};

use crate::actions::exec::Executor;
use crate::config::ConfigStore;
use crate::devices::snapshot::DeviceSnapshot;
use crate::devices::{Command, DaemonEvent, DeviceMsg, Shared, node};
use crate::env::Desktop;
use crate::input::{Recorded, RecordingInput, SharedInput};
use crate::notify::Notifier;
use crate::window::ActiveApp;

const DIRECT: u8 = 0xFF;
const GESTURE: u16 = 0x00C3;
const BACK: u16 = 0x0053;

struct Harness {
    shared: Arc<Shared>,
    input: Arc<RecordingInput>,
    app: watch::Sender<ActiveApp>,
    _events: broadcast::Receiver<DaemonEvent>,
    _dir: tempfile::TempDir,
}

impl Harness {
    fn new() -> Harness {
        let dir = tempfile::tempdir().unwrap();
        let config = ConfigStore::open(dir.path().join("config.toml"));
        let input = Arc::new(RecordingInput::default());
        let (_desktop_tx, desktop_rx) = watch::channel(Desktop::Other);
        let executor = Executor::new(Some(input.clone() as SharedInput), None, desktop_rx.clone());
        let notifier = Notifier::new(None, desktop_rx);
        let (app, app_rx) = watch::channel(ActiveApp::default());
        let (events_tx, events) = broadcast::channel(512);
        let shared = Arc::new(Shared {
            config,
            executor,
            notifier,
            registry: Default::default(),
            events: events_tx,
            active_app: app_rx,
            monitoring: AtomicBool::new(false),
        });
        Harness { shared, input, app, _events: events, _dir: dir }
    }

    fn sim(&self, topology: Topology) -> SimHandle {
        node::spawn_sim(self.shared.clone(), topology).unwrap()
    }

    /// Waits until a device with the given name is online and initialized.
    async fn device(&self, name: &str) -> DeviceSnapshot {
        self.eventually(|| {
            self.shared.registry.snapshots().iter().find(|s| s.name == name && s.online && !s.busy).map(|s| (**s).clone())
        })
        .await
        .unwrap_or_else(|| panic!("{name} did not come online"))
    }

    async fn eventually<T>(&self, mut f: impl FnMut() -> Option<T>) -> Option<T> {
        for _ in 0..200 {
            if let Some(v) = f() {
                return Some(v);
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        None
    }

    async fn command(&self, id: &str, command: serde_json::Value) {
        let command: Command = serde_json::from_value(command).unwrap();
        self.shared.command(id, command).await.unwrap();
    }

    /// Waits for recorded input to settle and returns it.
    async fn input(&self) -> Vec<Recorded> {
        let mut all = Vec::new();
        let mut quiet = 0;
        while quiet < 5 {
            tokio::time::sleep(Duration::from_millis(10)).await;
            let batch = self.input.take();
            if batch.is_empty() {
                quiet += 1;
            } else {
                quiet = 0;
                all.extend(batch);
            }
        }
        all
    }

    /// Lets the simulation and the actor process everything sent so far.
    async fn settle(&self) {
        tokio::time::sleep(Duration::from_millis(60)).await;
    }
}

fn tap(keys: &[K]) -> Vec<Recorded> {
    let mut v: Vec<Recorded> = keys.iter().map(|k| Recorded::Key(*k, true)).collect();
    v.extend(keys.iter().rev().map(|k| Recorded::Key(*k, false)));
    v
}

/// Raw counts for a distance in millimetres at 1000 DPI.
fn mm(v: f32) -> i16 {
    (v / 25.4 * 1000.0).round() as i16
}

#[tokio::test]
async fn first_connection_sets_up_gestures() {
    let h = Harness::new();
    let sim = h.sim(Topology::Direct(DeviceSpec::mx_master_3s()));
    let dev = h.device("MX Master 3S").await;
    assert_eq!(dev.id, "b034-5C8F7AF8");
    assert!(matches!(dev.config.bindings.buttons.get("gesture"), Some(crate::actions::Action::Gestures(_))));
    // The gesture button is diverted with raw XY; nothing else is touched.
    let state = sim.snapshot(DIRECT).await.unwrap();
    assert_eq!(state.reporting.get(&GESTURE), Some(&0x11));
    assert_eq!(state.reporting.get(&BACK).copied().unwrap_or(0), 0);
    assert!(!state.thumb_diverted);
}

#[tokio::test]
async fn gesture_directions_and_click() {
    let h = Harness::new();
    let sim = h.sim(Topology::Direct(DeviceSpec::mx_master_3s()));
    let dev = h.device("MX Master 3S").await;
    let keys = |k: &[&str]| json!({ "type": "keys", "keys": k });
    h.command(
        &dev.id,
        json!({ "command": "set-binding", "profile": null, "control": "gesture", "action": {
            "type": "gestures",
            "up": keys(&["KEY_LEFTMETA", "KEY_UP"]),
            "down": keys(&["KEY_LEFTMETA", "KEY_DOWN"]),
            "left": keys(&["KEY_LEFTMETA", "KEY_LEFT"]),
            "right": keys(&["KEY_LEFTMETA", "KEY_RIGHT"]),
            "click": keys(&["KEY_LEFTMETA"]),
            "threshold_mm": 5.0
        }}),
    )
    .await;

    // Hold, move 8 mm up in steps, release: one "up".
    sim.press(DIRECT, &[GESTURE]);
    sim.motion(DIRECT, 0, -1); // stale first report, ignored by design
    for _ in 0..8 {
        sim.motion(DIRECT, 0, mm(-1.0));
    }
    sim.release_all(DIRECT);
    assert_eq!(h.input().await, tap(&[K::KEY_LEFTMETA, K::KEY_UP]));

    // Press and release without moving: click.
    sim.press(DIRECT, &[GESTURE]);
    sim.release_all(DIRECT);
    assert_eq!(h.input().await, tap(&[K::KEY_LEFTMETA]));

    // Diagonal, mostly right.
    sim.press(DIRECT, &[GESTURE]);
    sim.motion(DIRECT, 0, 0);
    sim.motion(DIRECT, mm(7.0), mm(2.0));
    sim.release_all(DIRECT);
    assert_eq!(h.input().await, tap(&[K::KEY_LEFTMETA, K::KEY_RIGHT]));
}

#[tokio::test]
async fn pan_gesture_scrolls() {
    let h = Harness::new();
    let sim = h.sim(Topology::Direct(DeviceSpec::mx_master_3s()));
    let dev = h.device("MX Master 3S").await;
    h.command(
        &dev.id,
        json!({ "command": "set-binding", "control": "gesture", "action": { "type": "gestures", "mode": "pan" } }),
    )
    .await;
    sim.press(DIRECT, &[GESTURE]);
    sim.motion(DIRECT, 0, 0);
    for _ in 0..6 {
        sim.motion(DIRECT, 0, mm(1.0)); // move down: scroll down
    }
    sim.release_all(DIRECT);
    let input = h.input().await;
    let vertical: i32 = input.iter().map(|r| if let Recorded::Scroll(v, _) = r { *v } else { 0 }).sum();
    assert!((-250..=-230).contains(&vertical), "{input:?}");
    assert!(!input.iter().any(|r| matches!(r, Recorded::Key(..))), "pan must not click: {input:?}");
}

#[tokio::test]
async fn remapped_button_holds_keys() {
    let h = Harness::new();
    let sim = h.sim(Topology::Direct(DeviceSpec::mx_master_3s()));
    let dev = h.device("MX Master 3S").await;
    h.command(
        &dev.id,
        json!({ "command": "set-binding", "control": "back", "action": { "type": "keys", "keys": ["KEY_LEFTCTRL", "KEY_Z"] } }),
    )
    .await;
    assert_eq!(sim.snapshot(DIRECT).await.unwrap().reporting.get(&BACK), Some(&0x01));
    sim.press(DIRECT, &[BACK]);
    assert_eq!(h.input().await, vec![Recorded::Key(K::KEY_LEFTCTRL, true), Recorded::Key(K::KEY_Z, true)]);
    sim.release_all(DIRECT);
    assert_eq!(h.input().await, vec![Recorded::Key(K::KEY_Z, false), Recorded::Key(K::KEY_LEFTCTRL, false)]);

    // Back to default: no longer diverted.
    h.command(&dev.id, json!({ "command": "set-binding", "control": "back", "action": null })).await;
    assert_eq!(sim.snapshot(DIRECT).await.unwrap().reporting.get(&BACK), Some(&0x00));
}

#[tokio::test]
async fn thumbwheel_actions_and_scroll_speed() {
    let h = Harness::new();
    let sim = h.sim(Topology::Direct(DeviceSpec::mx_master_3s()));
    let dev = h.device("MX Master 3S").await;
    h.command(
        &dev.id,
        json!({ "command": "set-thumbwheel", "binding": {
            "type": "actions",
            "left": { "type": "builtin", "id": "volume-down" },
            "right": { "type": "builtin", "id": "volume-up" },
            "step_degrees": 30.0
        }}),
    )
    .await;
    assert!(sim.snapshot(DIRECT).await.unwrap().thumb_diverted);
    // 30 degrees is 10 diverted units; the MX Master's default direction is -1.
    sim.send(SimCommand::Thumbwheel { slot: DIRECT, rotation: -25, status: 1, flags: 0 });
    assert_eq!(h.input().await, [tap(&[K::KEY_VOLUMEUP]), tap(&[K::KEY_VOLUMEUP])].concat());
    sim.send(SimCommand::Thumbwheel { slot: DIRECT, rotation: 20, status: 1, flags: 0 });
    assert_eq!(h.input().await, [tap(&[K::KEY_VOLUMEDOWN]), tap(&[K::KEY_VOLUMEDOWN])].concat());

    // Inverted scrolling at double speed.
    h.command(&dev.id, json!({ "command": "set-settings", "settings": { "thumbwheel_invert": true } })).await;
    h.command(&dev.id, json!({ "command": "set-thumbwheel", "binding": { "type": "scroll", "speed": 2.0 } })).await;
    let state = sim.snapshot(DIRECT).await.unwrap();
    assert!(state.thumb_diverted && !state.thumb_inverted, "inverted in software while diverted");
    sim.send(SimCommand::Thumbwheel { slot: DIRECT, rotation: -10, status: 1, flags: 0 });
    let input = h.input().await;
    let horizontal: i32 = input.iter().map(|r| if let Recorded::Scroll(_, x) = r { *x } else { 0 }).sum();
    // 10 units = 1.5 native notches = 180 units, doubled and inverted.
    assert_eq!(horizontal, -360, "{input:?}");

    // Default again: the wheel goes back to native, keeping the inversion in the device.
    h.command(&dev.id, json!({ "command": "set-thumbwheel", "binding": null })).await;
    let state = sim.snapshot(DIRECT).await.unwrap();
    assert!(!state.thumb_diverted && state.thumb_inverted);
}

#[tokio::test]
async fn application_profiles_switch_bindings() {
    let h = Harness::new();
    let sim = h.sim(Topology::Direct(DeviceSpec::mx_master_3s()));
    let dev = h.device("MX Master 3S").await;
    let id = dev.id.clone();
    let profile = h.shared.command(&id, serde_json::from_value(json!({ "command": "add-profile", "name": "Firefox", "apps": ["firefox"] })).unwrap()).await.unwrap();
    let profile = profile.as_str().unwrap().to_string();
    h.command(&id, json!({ "command": "set-binding", "profile": profile, "control": "back",
        "action": { "type": "keys", "keys": ["KEY_LEFTCTRL", "KEY_W"] } }))
        .await;
    // Not focused yet: back is native, not diverted.
    assert_eq!(sim.snapshot(DIRECT).await.unwrap().reporting.get(&BACK).copied().unwrap_or(0), 0);

    h.app.send_replace(ActiveApp { app_id: "org.mozilla.firefox".into(), ..Default::default() });
    let active = h.eventually(|| h.shared.registry.snapshot(&id).and_then(|s| s.active_profile.clone())).await;
    assert_eq!(active.as_deref(), Some(profile.as_str()));
    h.settle().await;
    assert_eq!(sim.snapshot(DIRECT).await.unwrap().reporting.get(&BACK), Some(&0x01));
    sim.press(DIRECT, &[BACK]);
    sim.release_all(DIRECT);
    assert_eq!(h.input().await, tap(&[K::KEY_LEFTCTRL, K::KEY_W]));

    // Another application: the default bindings return.
    h.app.send_replace(ActiveApp { app_id: "org.kde.dolphin".into(), ..Default::default() });
    h.eventually(|| h.shared.registry.snapshot(&id).filter(|s| s.active_profile.is_none()).map(|_| ())).await.unwrap();
    h.settle().await;
    assert_eq!(sim.snapshot(DIRECT).await.unwrap().reporting.get(&BACK), Some(&0x00));
}

#[tokio::test]
async fn profile_change_waits_for_release() {
    let h = Harness::new();
    let sim = h.sim(Topology::Direct(DeviceSpec::mx_master_3s()));
    let dev = h.device("MX Master 3S").await;
    let id = dev.id.clone();
    let profile = h.shared.command(&id, serde_json::from_value(json!({ "command": "add-profile", "name": "Editor", "apps": ["editor"] })).unwrap()).await.unwrap();
    // In the editor the gesture button keeps its native behaviour.
    h.command(&id, json!({ "command": "set-binding", "profile": profile, "control": "gesture", "action": { "type": "default" } })).await;
    h.command(&id, json!({ "command": "set-binding", "control": "gesture", "action": {
        "type": "gestures", "up": { "type": "keys", "keys": ["KEY_F1"] }, "threshold_mm": 5.0 } }))
        .await;
    // A gesture switches focus to the "editor" application while the button is held...
    sim.press(DIRECT, &[GESTURE]);
    sim.motion(DIRECT, 0, 0);
    sim.motion(DIRECT, 0, mm(-8.0));
    h.settle().await;
    h.app.send_replace(ActiveApp { app_id: "editor".into(), ..Default::default() });
    h.settle().await;
    // ...the gesture button stays diverted until it is released, so the release is not lost.
    assert_eq!(sim.snapshot(DIRECT).await.unwrap().reporting.get(&GESTURE), Some(&0x11));
    sim.release_all(DIRECT);
    assert_eq!(h.input().await, tap(&[K::KEY_F1]));
    h.settle().await;
    assert_eq!(sim.snapshot(DIRECT).await.unwrap().reporting.get(&GESTURE), Some(&0x00));
}

#[tokio::test]
async fn settings_are_applied_and_restored_after_power_cycle() {
    let h = Harness::new();
    let sim = h.sim(Topology::Direct(DeviceSpec::mx_master_3s()));
    let dev = h.device("MX Master 3S").await;
    h.command(&dev.id, json!({ "command": "set-settings", "settings": { "dpi": 1234, "smartshift": false, "invert_scroll": true } })).await;
    let state = sim.snapshot(DIRECT).await.unwrap();
    assert_eq!(state.dpi, 1250, "snapped to the 50 DPI step");
    assert_eq!(state.auto_disengage, 255);
    assert_eq!(state.hires_mode & 0x04, 0x04);
    let snap = h.shared.registry.snapshot(&dev.id).unwrap();
    assert_eq!(snap.config.settings.dpi, Some(1250));

    // The device forgets its diversions; the service puts them back.
    sim.send(SimCommand::PowerCycle { slot: DIRECT });
    for _ in 0..100 {
        if sim.snapshot(DIRECT).await.unwrap().reporting.get(&GESTURE) == Some(&0x11) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("gesture diversion was not restored");
}

#[tokio::test]
async fn dpi_cycle_from_a_button() {
    let h = Harness::new();
    let sim = h.sim(Topology::Direct(DeviceSpec::mx_master_3s()));
    let dev = h.device("MX Master 3S").await;
    h.command(&dev.id, json!({ "command": "set-settings", "settings": { "dpi_presets": [800, 1600, 3200], "dpi": 800 } })).await;
    h.command(&dev.id, json!({ "command": "set-binding", "control": "mode-shift", "action": { "type": "builtin", "id": "dpi-cycle" } })).await;
    for expected in [1600, 3200, 800] {
        sim.press(DIRECT, &[0x00C4]);
        sim.release_all(DIRECT);
        h.settle().await;
        assert_eq!(sim.snapshot(DIRECT).await.unwrap().dpi, expected);
    }
}

#[tokio::test]
async fn precision_mode_while_held() {
    let h = Harness::new();
    let sim = h.sim(Topology::Direct(DeviceSpec::mx_master_3s()));
    let dev = h.device("MX Master 3S").await;
    h.command(&dev.id, json!({ "command": "set-binding", "control": "forward", "action": { "type": "shift-dpi", "dpi": 400 } })).await;
    sim.press(DIRECT, &[0x0056]);
    h.settle().await;
    assert_eq!(sim.snapshot(DIRECT).await.unwrap().dpi, 400);
    sim.release_all(DIRECT);
    h.settle().await;
    assert_eq!(sim.snapshot(DIRECT).await.unwrap().dpi, 1000);
}

#[tokio::test]
async fn receiver_devices_reconnect() {
    let h = Harness::new();
    let sim = h.sim(Topology::Receiver { kind: ReceiverKind::Bolt, devices: vec![DeviceSpec::mx_master_4()] });
    let dev = h.device("MX Master 4").await;
    assert_eq!(dev.connection.receiver.as_deref(), Some("Bolt Receiver"));
    assert!(dev.capabilities.haptics.is_some());
    assert!(dev.capabilities.smartshift.as_ref().is_some_and(|s| s.torque));
    assert_eq!(sim.snapshot(1).await.unwrap().reporting.get(&GESTURE), Some(&0x11));

    // Asleep: offline, diversions lost. Awake: everything is applied again.
    sim.send(SimCommand::Online { slot: 1, online: false });
    h.eventually(|| h.shared.registry.snapshot(&dev.id).filter(|s| !s.online).map(|_| ())).await.expect("went offline");
    assert!(sim.snapshot(1).await.unwrap().reporting.is_empty());
    sim.send(SimCommand::Online { slot: 1, online: true });
    h.device("MX Master 4").await;
    h.settle().await;
    assert_eq!(sim.snapshot(1).await.unwrap().reporting.get(&GESTURE), Some(&0x11));

    // Haptics.
    h.command(&dev.id, json!({ "command": "play-haptic", "waveform": 4 })).await;
    assert_eq!(sim.snapshot(1).await.unwrap().haptics_played, vec![4]);
}

#[tokio::test]
async fn stopping_restores_native_behaviour() {
    let h = Harness::new();
    let sim = h.sim(Topology::Direct(DeviceSpec::mx_master_3s()));
    let dev = h.device("MX Master 3S").await;
    h.command(&dev.id, json!({ "command": "set-binding", "control": "back", "action": { "type": "builtin", "id": "copy" } })).await;
    h.command(&dev.id, json!({ "command": "set-thumbwheel", "binding": { "type": "scroll", "speed": 1.5 } })).await;
    let mailbox = h.shared.registry.mailbox(&dev.id).unwrap();
    let (tx, rx) = tokio::sync::oneshot::channel();
    mailbox.send(DeviceMsg::Stop(tx)).unwrap();
    tokio::time::timeout(Duration::from_secs(2), rx).await.unwrap().unwrap();
    let state = sim.snapshot(DIRECT).await.unwrap();
    assert!(state.reporting.values().all(|f| *f == 0), "{:?}", state.reporting);
    assert!(!state.thumb_diverted);
}
