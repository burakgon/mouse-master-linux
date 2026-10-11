//! Virtual input devices (uinput) used to perform actions.
//!
//! Two devices are created so libinput classifies them cleanly: a keyboard
//! for key combinations and a pointer for buttons, motion and (high
//! resolution) scrolling. Both work on any Wayland compositor and on X11.

use std::io;
use std::sync::Arc;
use std::time::{Duration, Instant};

use evdev::uinput::VirtualDevice;
use evdev::{AttributeSet, BusType, EventType, InputEvent, InputId, KeyCode, RelativeAxisCode};
use parking_lot::Mutex;
use tracing::{debug, warn};

const KEYBOARD_NAME: &str = "Mouse Master virtual keyboard";
const POINTER_NAME: &str = "Mouse Master virtual pointer";

/// High resolution wheel units per notch.
pub const WHEEL_UNIT: i32 = 120;

/// Where actions send their input: uinput in the service, a recorder in tests.
pub trait InputSink: Send + Sync {
    /// Presses or releases a single key or button.
    fn set_key(&self, k: KeyCode, pressed: bool);
    /// Scrolls by high resolution units (120 per notch). Positive vertical
    /// values scroll up, positive horizontal values scroll right.
    fn scroll(&self, vertical: i32, horizontal: i32);
    /// Resets partial scroll accumulation (at the end of a gesture).
    fn reset_scroll(&self) {}
    /// Releases everything still held (on shutdown or device disconnect).
    fn release_all(&self);

    /// Whether input can currently be delivered.
    fn available(&self) -> bool {
        true
    }

    /// Presses keys in order (e.g. modifiers first).
    fn press(&self, keys: &[KeyCode]) {
        for k in keys {
            self.set_key(*k, true);
        }
    }

    /// Releases keys in reverse order.
    fn release(&self, keys: &[KeyCode]) {
        for k in keys.iter().rev() {
            self.set_key(*k, false);
        }
    }

    /// Presses and releases a combination.
    fn tap(&self, keys: &[KeyCode]) {
        self.press(keys);
        self.release(keys);
    }

    fn click(&self, button: KeyCode) {
        self.set_key(button, true);
        self.set_key(button, false);
    }

    /// Ctrl + wheel, the most widely supported zoom gesture.
    fn zoom(&self, amount: i32) {
        self.set_key(KeyCode::KEY_LEFTCTRL, true);
        self.scroll(amount, 0);
        self.set_key(KeyCode::KEY_LEFTCTRL, false);
    }
}

struct Pointer {
    dev: VirtualDevice,
    /// Accumulated high resolution scroll, to emit legacy notches.
    v_acc: i32,
    h_acc: i32,
}

pub struct VirtualInput {
    keyboard: Mutex<VirtualDevice>,
    pointer: Mutex<Pointer>,
    /// Keys currently held down by us, so they can be released on shutdown.
    held: Mutex<Vec<KeyCode>>,
}

pub type SharedInput = Arc<dyn InputSink>;

/// Creates the uinput devices as soon as `/dev/uinput` is accessible.
///
/// At boot the service can start a moment before logind hands the session
/// access to `/dev/uinput`; creation is retried on use and by [`LazyInput::keep_trying`].
pub struct LazyInput {
    inner: Mutex<Option<Arc<VirtualInput>>>,
    last_attempt: Mutex<Option<Instant>>,
    warned: std::sync::atomic::AtomicBool,
}

impl LazyInput {
    pub fn new() -> Arc<LazyInput> {
        let lazy = Arc::new(LazyInput {
            inner: Mutex::new(None),
            last_attempt: Mutex::new(None),
            warned: std::sync::atomic::AtomicBool::new(false),
        });
        lazy.get();
        lazy
    }

    /// The devices, creating them if needed (at most one attempt per second).
    fn get(&self) -> Option<Arc<VirtualInput>> {
        if let Some(v) = self.inner.lock().as_ref() {
            return Some(v.clone());
        }
        {
            let mut last = self.last_attempt.lock();
            if last.is_some_and(|t| t.elapsed() < Duration::from_secs(1)) {
                return None;
            }
            *last = Some(Instant::now());
        }
        match VirtualInput::new() {
            Ok(v) => {
                let v = Arc::new(v);
                *self.inner.lock() = Some(v.clone());
                if self.warned.load(std::sync::atomic::Ordering::Relaxed) {
                    tracing::info!("virtual input devices are ready");
                }
                Some(v)
            }
            Err(e) => {
                if !self.warned.swap(true, std::sync::atomic::Ordering::Relaxed) {
                    warn!(
                        "cannot create virtual input devices yet ({e}); retrying. If this persists, install the udev rules."
                    );
                }
                None
            }
        }
    }

    /// Retries in the background until the devices exist.
    pub async fn keep_trying(self: Arc<Self>) {
        while self.get().is_none() {
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
    }
}

impl InputSink for LazyInput {
    fn set_key(&self, k: KeyCode, pressed: bool) {
        if let Some(v) = self.get() {
            v.set_key(k, pressed);
        }
    }
    fn scroll(&self, vertical: i32, horizontal: i32) {
        if let Some(v) = self.get() {
            v.scroll(vertical, horizontal);
        }
    }
    fn reset_scroll(&self) {
        if let Some(v) = self.inner.lock().as_ref() {
            v.reset_scroll();
        }
    }
    fn release_all(&self) {
        if let Some(v) = self.inner.lock().as_ref() {
            v.release_all();
        }
    }
    fn available(&self) -> bool {
        self.get().is_some()
    }
}

fn key_event(k: KeyCode, value: i32) -> InputEvent {
    InputEvent::new(EventType::KEY.0, k.0, value)
}

fn rel_event(axis: RelativeAxisCode, value: i32) -> InputEvent {
    InputEvent::new(EventType::RELATIVE.0, axis.0, value)
}

impl VirtualInput {
    pub fn new() -> io::Result<VirtualInput> {
        let mut keys = AttributeSet::<KeyCode>::new();
        // Keyboard keys and multimedia keys, but no buttons (0x100..0x160 are BTN_*).
        for code in 1..0x100u16 {
            keys.insert(KeyCode::new(code));
        }
        for code in 0x160..0x2FFu16 {
            // Skip the D-pad and "trigger happy" buttons so the device is never mistaken for a gamepad.
            if !(0x220..0x224).contains(&code) && !(0x2C0..0x2E8).contains(&code) {
                keys.insert(KeyCode::new(code));
            }
        }
        let keyboard = VirtualDevice::builder()?
            .name(KEYBOARD_NAME)
            .input_id(InputId::new(BusType::BUS_VIRTUAL, 0x046D, 0x4D4D, 1))
            .with_keys(&keys)?
            .build()?;

        let mut buttons = AttributeSet::<KeyCode>::new();
        for b in [
            KeyCode::BTN_LEFT,
            KeyCode::BTN_RIGHT,
            KeyCode::BTN_MIDDLE,
            KeyCode::BTN_SIDE,
            KeyCode::BTN_EXTRA,
            KeyCode::BTN_FORWARD,
            KeyCode::BTN_BACK,
            KeyCode::BTN_TASK,
        ] {
            buttons.insert(b);
        }
        let mut axes = AttributeSet::<RelativeAxisCode>::new();
        for a in [
            RelativeAxisCode::REL_X,
            RelativeAxisCode::REL_Y,
            RelativeAxisCode::REL_WHEEL,
            RelativeAxisCode::REL_HWHEEL,
            RelativeAxisCode::REL_WHEEL_HI_RES,
            RelativeAxisCode::REL_HWHEEL_HI_RES,
        ] {
            axes.insert(a);
        }
        let pointer = VirtualDevice::builder()?
            .name(POINTER_NAME)
            .input_id(InputId::new(BusType::BUS_VIRTUAL, 0x046D, 0x4D50, 1))
            .with_keys(&buttons)?
            .with_relative_axes(&axes)?
            .build()?;
        debug!("created uinput devices");
        Ok(VirtualInput {
            keyboard: Mutex::new(keyboard),
            pointer: Mutex::new(Pointer { dev: pointer, v_acc: 0, h_acc: 0 }),
            held: Mutex::new(Vec::new()),
        })
    }

    fn emit_keyboard(&self, events: &[InputEvent]) {
        if let Err(e) = self.keyboard.lock().emit(events) {
            warn!("uinput keyboard write failed: {e}");
        }
    }

    fn emit_pointer(&self, events: &[InputEvent]) {
        if let Err(e) = self.pointer.lock().dev.emit(events) {
            warn!("uinput pointer write failed: {e}");
        }
    }

    fn is_button(k: KeyCode) -> bool {
        (0x100..0x160).contains(&k.0)
    }
}

impl InputSink for VirtualInput {
    /// Presses or releases a single key or button.
    fn set_key(&self, k: KeyCode, pressed: bool) {
        {
            let mut held = self.held.lock();
            if pressed {
                held.push(k);
            } else if let Some(pos) = held.iter().rposition(|h| *h == k) {
                held.remove(pos);
            }
        }
        let ev = [key_event(k, pressed as i32)];
        if Self::is_button(k) { self.emit_pointer(&ev) } else { self.emit_keyboard(&ev) }
    }

    /// Scrolls by high resolution units (120 per notch). Positive vertical
    /// values scroll up, positive horizontal values scroll right.
    fn scroll(&self, vertical: i32, horizontal: i32) {
        if vertical == 0 && horizontal == 0 {
            return;
        }
        let mut p = self.pointer.lock();
        let mut events = Vec::with_capacity(4);
        if vertical != 0 {
            events.push(rel_event(RelativeAxisCode::REL_WHEEL_HI_RES, vertical));
            p.v_acc += vertical;
            let notches = p.v_acc / WHEEL_UNIT;
            if notches != 0 {
                p.v_acc -= notches * WHEEL_UNIT;
                events.push(rel_event(RelativeAxisCode::REL_WHEEL, notches));
            }
        }
        if horizontal != 0 {
            events.push(rel_event(RelativeAxisCode::REL_HWHEEL_HI_RES, horizontal));
            p.h_acc += horizontal;
            let notches = p.h_acc / WHEEL_UNIT;
            if notches != 0 {
                p.h_acc -= notches * WHEEL_UNIT;
                events.push(rel_event(RelativeAxisCode::REL_HWHEEL, notches));
            }
        }
        if let Err(e) = p.dev.emit(&events) {
            warn!("uinput scroll failed: {e}");
        }
    }

    /// Resets partial scroll accumulation (at the end of a gesture).
    fn reset_scroll(&self) {
        let mut p = self.pointer.lock();
        p.v_acc = 0;
        p.h_acc = 0;
    }

    /// Releases everything still held (on shutdown or device disconnect).
    fn release_all(&self) {
        let held: Vec<KeyCode> = std::mem::take(&mut *self.held.lock());
        for k in held.iter().rev() {
            let ev = [key_event(*k, 0)];
            if Self::is_button(*k) { self.emit_pointer(&ev) } else { self.emit_keyboard(&ev) }
        }
    }
}

impl Drop for VirtualInput {
    fn drop(&mut self) {
        self.release_all();
    }
}

/// Records input instead of sending it (tests).
#[cfg(test)]
#[derive(Default)]
pub struct RecordingInput {
    events: Mutex<Vec<Recorded>>,
}

#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Recorded {
    Key(KeyCode, bool),
    Scroll(i32, i32),
}

#[cfg(test)]
impl RecordingInput {
    pub fn take(&self) -> Vec<Recorded> {
        std::mem::take(&mut *self.events.lock())
    }
}

#[cfg(test)]
impl InputSink for RecordingInput {
    fn set_key(&self, k: KeyCode, pressed: bool) {
        self.events.lock().push(Recorded::Key(k, pressed));
    }
    fn scroll(&self, vertical: i32, horizontal: i32) {
        self.events.lock().push(Recorded::Scroll(vertical, horizontal));
    }
    fn release_all(&self) {}
}
