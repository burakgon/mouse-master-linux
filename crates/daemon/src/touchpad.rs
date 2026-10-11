//! A virtual touchpad that zooms by pinching with two fingers.
//!
//! Browsers, document and image viewers and maps zoom smoothly around the
//! pointer when a touchpad pinches, which Ctrl + wheel steps cannot do.
//! libinput turns two touches moving apart into a pinch gesture and the
//! compositor sends it to the window under the pointer.
//!
//! The fingers are driven from a thread of their own because libinput
//! expects touchpad frames at a steady pace: it throws away touches that
//! jump too far within one frame.

use std::io;
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread;
use std::time::{Duration, Instant};

use evdev::uinput::VirtualDevice;
use evdev::{
    AbsInfo, AbsoluteAxisCode as Abs, AttributeSet, BusType, EventType, InputEvent, InputId, KeyCode, PropType,
    UinputAbsSetup,
};
use tracing::{debug, warn};

/// Device units per millimetre.
const RES: i32 = 40;
const WIDTH_MM: f64 = 160.0;
const HEIGHT_MM: f64 = 100.0;
/// Half the distance between the fingers when they touch down.
const START_HALF_MM: f64 = 15.0;
/// How far each finger first moves apart. libinput tells a pinch from a
/// scroll once both touches moved 1.5 mm in opposite directions; this
/// movement is not part of the zoom.
const KICK_MM: f64 = 2.5;
/// Closest and furthest the fingers get before starting a new pinch.
const MIN_HALF_MM: f64 = 4.0;
const MAX_HALF_MM: f64 = WIDTH_MM / 2.0 - 5.0;
/// Furthest a finger moves in one frame, well below libinput's jump detection.
const MAX_STEP_MM: f64 = 1.5;
/// Time between frames (libinput's reference touchpad sends one every 12 ms).
const FRAME: Duration = Duration::from_millis(10);
/// The fingers are lifted after this long without zooming.
const IDLE: Duration = Duration::from_millis(500);
/// Time for the compositor to pick up the device after it is created.
const SETTLE: Duration = Duration::from_millis(400);

enum Command {
    Zoom(f64),
    End,
}

/// Sends pinches to the touchpad thread. Dropping it lifts the fingers and
/// removes the device.
pub struct Touchpad {
    tx: Sender<Command>,
    sys_name: Option<String>,
}

impl Touchpad {
    pub fn new(name: &str) -> io::Result<Touchpad> {
        let (pad, sys_name) = Pad::new(name)?;
        let (tx, rx) = mpsc::channel();
        thread::Builder::new().name("touchpad".into()).spawn(move || run(pad, rx))?;
        Ok(Touchpad { tx, sys_name })
    }

    /// The kernel name of the device (`event24`), as compositors know it.
    pub fn sys_name(&self) -> Option<&str> {
        self.sys_name.as_deref()
    }

    /// Multiplies the zoom by `factor` (above 1 zooms in).
    pub fn zoom(&self, factor: f64) {
        if factor.is_finite() && factor > 0.0 {
            let _ = self.tx.send(Command::Zoom(factor));
        }
    }

    /// Lifts the fingers once the zoom asked for so far is done.
    pub fn end(&self) {
        let _ = self.tx.send(Command::End);
    }
}

fn run(mut pad: Pad, rx: Receiver<Command>) {
    // Zoom still to apply, as a factor.
    let mut pending = 1.0f64;
    let mut end = false;
    let mut last_input = Instant::now();
    let zooming = |pending: f64| (pending - 1.0).abs() > 1e-4;
    loop {
        // Frames pace themselves, so commands that arrive meanwhile are merged.
        let wait = if zooming(pending) || (end && pad.is_down()) {
            Duration::ZERO
        } else if pad.is_down() {
            IDLE.saturating_sub(last_input.elapsed())
        } else {
            Duration::from_secs(3600)
        };
        let mut next = match rx.recv_timeout(wait) {
            Ok(cmd) => Some(cmd),
            Err(RecvTimeoutError::Timeout) => None,
            Err(RecvTimeoutError::Disconnected) => return,
        };
        while let Some(cmd) = next {
            match cmd {
                Command::Zoom(f) => {
                    pending *= f;
                    end = false;
                    last_input = Instant::now();
                }
                Command::End => end = true,
            }
            next = rx.try_recv().ok();
        }
        if zooming(pending) {
            pending = pad.zoom_step(pending);
        } else if pad.is_down() && (end || last_input.elapsed() >= IDLE) {
            pad.lift();
            pending = 1.0;
            end = false;
        }
    }
}

type Output = Box<dyn FnMut(&[InputEvent]) -> io::Result<()> + Send>;

struct Pad {
    out: Output,
    /// Whether to wait between frames (always, except in tests).
    pace: bool,
    created: Instant,
    last_frame: Instant,
    /// Half the distance between the fingers while they are down.
    half: Option<f64>,
    /// Last position written, in device units.
    written: i32,
    /// Whether the fingers moved since the kick. libinput only starts the
    /// pinch on that movement, and lifting before it confuses the next one.
    begun: bool,
    tracking_id: i32,
}

fn abs_setup(code: Abs, max: i32, resolution: i32) -> UinputAbsSetup {
    UinputAbsSetup::new(code, AbsInfo::new(0, 0, max, 0, 0, resolution))
}

fn abs(code: Abs, value: i32) -> InputEvent {
    InputEvent::new(EventType::ABSOLUTE.0, code.0, value)
}

fn key(code: KeyCode, value: i32) -> InputEvent {
    InputEvent::new(EventType::KEY.0, code.0, value)
}

impl Pad {
    fn new(name: &str) -> io::Result<(Pad, Option<String>)> {
        let mut keys = AttributeSet::<KeyCode>::new();
        for k in [KeyCode::BTN_LEFT, KeyCode::BTN_TOUCH, KeyCode::BTN_TOOL_FINGER, KeyCode::BTN_TOOL_DOUBLETAP] {
            keys.insert(k);
        }
        let mut props = AttributeSet::<PropType>::new();
        props.insert(PropType::POINTER);
        props.insert(PropType::BUTTONPAD);
        let (w, h) = ((WIDTH_MM * RES as f64) as i32, (HEIGHT_MM * RES as f64) as i32);
        // Logitech's vendor id makes libinput treat it as an external touchpad:
        // no disable-while-typing and no palm detection at the edges.
        let mut dev = VirtualDevice::builder()?
            .name(name)
            .input_id(InputId::new(BusType::BUS_VIRTUAL, 0x046D, 0x4D54, 1))
            .with_keys(&keys)?
            .with_properties(&props)?
            .with_absolute_axis(&abs_setup(Abs::ABS_X, w, RES))?
            .with_absolute_axis(&abs_setup(Abs::ABS_Y, h, RES))?
            .with_absolute_axis(&abs_setup(Abs::ABS_MT_SLOT, 1, 0))?
            .with_absolute_axis(&abs_setup(Abs::ABS_MT_POSITION_X, w, RES))?
            .with_absolute_axis(&abs_setup(Abs::ABS_MT_POSITION_Y, h, RES))?
            .with_absolute_axis(&abs_setup(Abs::ABS_MT_TRACKING_ID, 0xFFFF, 0))?
            .build()?;
        let sys_name = std::fs::read_dir(dev.get_syspath()?)?
            .filter_map(|e| e.ok()?.file_name().into_string().ok())
            .find(|n| n.starts_with("event"));
        debug!("created the virtual touchpad {sys_name:?}");
        Ok((Pad::with_output(Box::new(move |events| dev.emit(events)), true), sys_name))
    }

    fn with_output(out: Output, pace: bool) -> Pad {
        let now = Instant::now();
        Pad { out, pace, created: now, last_frame: now, half: None, written: 0, begun: false, tracking_id: 0 }
    }

    fn is_down(&self) -> bool {
        self.half.is_some()
    }

    /// Writes a frame with both fingers `half` device units from the centre.
    fn frame(&mut self, half: i32, touch_down: bool) {
        let (cx, cy) = ((WIDTH_MM / 2.0) as i32 * RES, (HEIGHT_MM / 2.0) as i32 * RES);
        let mut events = Vec::with_capacity(12);
        for (slot, x) in [(0, cx - half), (1, cx + half)] {
            events.push(abs(Abs::ABS_MT_SLOT, slot));
            if touch_down {
                self.tracking_id = (self.tracking_id + 1) & 0xFFFF;
                events.push(abs(Abs::ABS_MT_TRACKING_ID, self.tracking_id));
            }
            events.push(abs(Abs::ABS_MT_POSITION_X, x));
            events.push(abs(Abs::ABS_MT_POSITION_Y, cy));
        }
        if touch_down {
            events.push(key(KeyCode::BTN_TOUCH, 1));
            events.push(key(KeyCode::BTN_TOOL_DOUBLETAP, 1));
        }
        events.push(abs(Abs::ABS_X, cx - half));
        events.push(abs(Abs::ABS_Y, cy));
        self.emit(&events);
        self.written = half;
    }

    fn emit(&mut self, events: &[InputEvent]) {
        // Keep frames apart so libinput sees plausible finger speeds.
        if let Some(rest) = FRAME.checked_sub(self.last_frame.elapsed()).filter(|_| self.pace) {
            thread::sleep(rest);
        }
        if let Err(e) = (self.out)(events) {
            warn!("virtual touchpad write failed: {e}");
        }
        self.last_frame = Instant::now();
    }

    fn touch_down(&mut self) {
        if let Some(rest) = SETTLE.checked_sub(self.created.elapsed()).filter(|_| self.pace) {
            thread::sleep(rest);
        }
        let start = START_HALF_MM * RES as f64;
        let kicked = (START_HALF_MM + KICK_MM) * RES as f64;
        self.frame(start as i32, true);
        self.frame(kicked as i32, false);
        self.half = Some(kicked);
        self.begun = false;
    }

    fn lift(&mut self) {
        if self.half.take().is_none() {
            return;
        }
        if !self.begun {
            let nudge = self.written + 1;
            self.frame(nudge, false);
        }
        let mut events = Vec::with_capacity(6);
        for slot in [0, 1] {
            events.push(abs(Abs::ABS_MT_SLOT, slot));
            events.push(abs(Abs::ABS_MT_TRACKING_ID, -1));
        }
        events.push(key(KeyCode::BTN_TOUCH, 0));
        events.push(key(KeyCode::BTN_TOOL_DOUBLETAP, 0));
        self.emit(&events);
    }

    /// Moves the fingers one frame towards `factor` times their distance and
    /// returns the factor still to go.
    fn zoom_step(&mut self, factor: f64) -> f64 {
        let half = match self.half {
            Some(h) => h,
            None => {
                self.touch_down();
                return factor;
            }
        };
        let (lo, hi) = (MIN_HALF_MM * RES as f64, MAX_HALF_MM * RES as f64);
        if (factor < 1.0 && half <= lo) || (factor > 1.0 && half >= hi) {
            // Out of room: lift the fingers and carry on with a new pinch.
            self.lift();
            self.touch_down();
            return factor;
        }
        let target = (half * factor).clamp(lo, hi);
        let max_step = MAX_STEP_MM * RES as f64;
        let next = half + (target - half).clamp(-max_step, max_step);
        self.half = Some(next);
        let x = next.round() as i32;
        if x != self.written {
            self.frame(x, false);
            self.begun = true;
        }
        factor * half / next
    }
}

impl Drop for Pad {
    fn drop(&mut self) {
        self.lift();
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq)]
    enum Frame {
        Down(i32),
        Move(i32),
        Up,
    }

    /// A pad that records frames as the distance between the fingers.
    fn pad() -> (Pad, Arc<Mutex<Vec<Frame>>>) {
        let frames = Arc::new(Mutex::new(Vec::new()));
        let sink = frames.clone();
        let out: Output = Box::new(move |events: &[InputEvent]| {
            let mut xs = [None, None];
            let (mut slot, mut down, mut up) = (0, false, false);
            for e in events {
                match (e.event_type(), e.code()) {
                    (EventType::ABSOLUTE, c) if c == Abs::ABS_MT_SLOT.0 => slot = e.value() as usize,
                    (EventType::ABSOLUTE, c) if c == Abs::ABS_MT_POSITION_X.0 => xs[slot] = Some(e.value()),
                    (EventType::ABSOLUTE, c) if c == Abs::ABS_MT_TRACKING_ID.0 => {
                        down |= e.value() >= 0;
                        up |= e.value() < 0;
                    }
                    _ => {}
                }
            }
            let frame = match xs {
                _ if up => Frame::Up,
                [Some(a), Some(b)] if down => Frame::Down(b - a),
                [Some(a), Some(b)] => {
                    // Both fingers stay symmetric around the centre.
                    assert_eq!(a + b, (WIDTH_MM as i32) * RES, "{events:?}");
                    Frame::Move(b - a)
                }
                _ => panic!("unexpected frame {events:?}"),
            };
            sink.lock().unwrap().push(frame);
            Ok(())
        });
        (Pad::with_output(out, false), frames)
    }

    fn zoom(pad: &mut Pad, factor: f64) {
        let mut left = factor;
        for _ in 0..1000 {
            if (left - 1.0).abs() < 1e-6 {
                return;
            }
            left = pad.zoom_step(left);
        }
        panic!("zoom did not finish");
    }

    /// The zoom each pinch made, from the frames.
    fn pinches(frames: &[Frame]) -> Vec<f64> {
        let mut out = Vec::new();
        let mut start = None;
        let mut last = 0;
        for (i, f) in frames.iter().enumerate() {
            match *f {
                Frame::Down(_) => start = None,
                // The kick right after touching down is not part of the zoom.
                Frame::Move(d) if matches!(frames[i - 1], Frame::Down(_)) => {
                    start = Some(d);
                    last = d;
                }
                Frame::Move(d) => last = d,
                Frame::Up => out.push(last as f64 / start.unwrap() as f64),
            }
        }
        out
    }

    #[test]
    fn zooms_by_the_factor_in_small_steps() {
        let (mut pad, frames) = pad();
        zoom(&mut pad, 2.0);
        pad.lift();
        let frames = frames.lock().unwrap().clone();
        assert!(matches!(frames[0], Frame::Down(_)));
        let zooms = pinches(&frames);
        assert_eq!(zooms.len(), 1);
        assert!((zooms[0] - 2.0).abs() < 0.01, "{zooms:?}");
        let max_step = (MAX_STEP_MM * RES as f64 * 2.0).round() as i32 + 1;
        for pair in frames.windows(2) {
            if let [Frame::Move(a), Frame::Move(b)] = pair {
                assert!((b - a).abs() <= max_step, "{pair:?}");
            }
        }
    }

    #[test]
    fn starts_a_new_pinch_when_out_of_room() {
        let (mut pad, frames) = pad();
        zoom(&mut pad, 0.05);
        pad.lift();
        let zooms = pinches(&frames.lock().unwrap());
        assert!(zooms.len() >= 2, "{zooms:?}");
        let total: f64 = zooms.iter().product();
        assert!((total - 0.05).abs() < 0.002, "{zooms:?}");
    }

    #[test]
    fn fingers_move_before_lifting() {
        // libinput only begins a pinch on movement after the kick; lifting
        // before that leaves it half started and breaks the next pinch.
        let (mut pad, frames) = pad();
        zoom(&mut pad, 1.0001);
        pad.lift();
        let frames = frames.lock().unwrap().clone();
        assert_eq!(frames.len(), 4, "{frames:?}");
        assert!(matches!(frames[..], [Frame::Down(_), Frame::Move(_), Frame::Move(_), Frame::Up]));
    }
}
