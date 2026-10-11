//! Mouse gesture recognition: turns raw pointer motion reported while a
//! gesture button is held into directions, clicks or scrolling.

use crate::actions::{Direction, GestureMode, GestureSet, GestureTrigger};

const MM_PER_INCH: f32 = 25.4;
/// Pan mode: high resolution scroll units per millimetre at speed 1.0
/// (one notch every 3 mm).
const PAN_UNITS_PER_MM: f32 = 40.0;
/// Movement below this is still a click in pan and zoom mode.
const PAN_CLICK_SLOP_MM: f32 = 2.0;
/// Zoom mode: millimetres to move the mouse to double the zoom at speed 1.0.
const ZOOM_MM_PER_DOUBLING: f32 = 30.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GestureEvent {
    Fire(Direction),
    Click,
    Scroll {
        vertical: i32,
        horizontal: i32,
    },
    /// Pinch zoom by a factor (above 1 zooms in).
    Zoom(f32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Axis {
    Vertical,
    Horizontal,
}

#[derive(Debug)]
pub struct GestureTracker {
    set: GestureSet,
    acc_x: f32,
    acc_y: f32,
    travel: f32,
    fired: bool,
    locked_axis: Option<Axis>,
    skip_next: bool,
    rem_v: f32,
    rem_h: f32,
}

fn direction_of(x: f32, y: f32) -> (Direction, Axis) {
    if x.abs() > y.abs() {
        (if x > 0.0 { Direction::Right } else { Direction::Left }, Axis::Horizontal)
    } else {
        // Screen coordinates: moving the mouse away from you is negative y.
        (if y > 0.0 { Direction::Down } else { Direction::Up }, Axis::Vertical)
    }
}

impl GestureTracker {
    /// `skip_first`: ignore the first motion report, which some devices
    /// (MX Master 3S) fill with stale data.
    pub fn new(set: GestureSet, skip_first: bool) -> GestureTracker {
        GestureTracker {
            set,
            acc_x: 0.0,
            acc_y: 0.0,
            travel: 0.0,
            fired: false,
            locked_axis: None,
            skip_next: skip_first,
            rem_v: 0.0,
            rem_h: 0.0,
        }
    }

    pub fn set(&self) -> &GestureSet {
        &self.set
    }

    fn threshold(&self) -> f32 {
        self.set.threshold_mm.clamp(1.0, 50.0)
    }

    /// Feeds raw motion in device counts at the given resolution.
    pub fn motion(&mut self, dx: i16, dy: i16, dpi: u16) -> Vec<GestureEvent> {
        if self.skip_next {
            self.skip_next = false;
            return Vec::new();
        }
        let scale = MM_PER_INCH / dpi.max(100) as f32;
        let (mx, my) = (dx as f32 * scale, dy as f32 * scale);
        self.travel += (mx * mx + my * my).sqrt();
        match self.set.mode {
            GestureMode::Pan => self.pan(mx, my),
            GestureMode::Zoom => {
                if my == 0.0 {
                    return Vec::new();
                }
                // Moving the mouse away from you (negative y) zooms in.
                let speed = self.set.zoom_speed.clamp(0.1, 10.0);
                vec![GestureEvent::Zoom(2f32.powf(-my / ZOOM_MM_PER_DOUBLING * speed))]
            }
            GestureMode::Directional => {
                self.acc_x += mx;
                self.acc_y += my;
                if self.set.trigger == GestureTrigger::Move { self.directional() } else { Vec::new() }
            }
        }
    }

    fn pan(&mut self, mx: f32, my: f32) -> Vec<GestureEvent> {
        let k = PAN_UNITS_PER_MM * self.set.pan_speed.clamp(0.1, 10.0);
        let sign = if self.set.pan_invert { -1.0 } else { 1.0 };
        // Moving the mouse down scrolls down (negative wheel values).
        self.rem_v += -my * k * sign;
        self.rem_h += mx * k * sign;
        let (v, h) = (self.rem_v.trunc() as i32, self.rem_h.trunc() as i32);
        self.rem_v -= v as f32;
        self.rem_h -= h as f32;
        if v == 0 && h == 0 { Vec::new() } else { vec![GestureEvent::Scroll { vertical: v, horizontal: h }] }
    }

    fn directional(&mut self) -> Vec<GestureEvent> {
        let mut out = Vec::new();
        let threshold = self.threshold();
        loop {
            let (dir, axis) = direction_of(self.acc_x, self.acc_y);
            let along = match axis {
                Axis::Horizontal => self.acc_x.abs(),
                Axis::Vertical => self.acc_y.abs(),
            };
            if along < threshold {
                break;
            }
            if self.fired {
                // Only a repeating axis keeps firing, and only along the axis it started on.
                if !self.set.repeats(dir) || self.locked_axis != Some(axis) {
                    // Swallow movement so a later reversal does not fire twice.
                    self.acc_x = 0.0;
                    self.acc_y = 0.0;
                    break;
                }
            }
            out.push(GestureEvent::Fire(dir));
            self.fired = true;
            self.locked_axis = Some(axis);
            match axis {
                Axis::Horizontal => {
                    self.acc_x -= threshold * self.acc_x.signum();
                    self.acc_y = 0.0;
                }
                Axis::Vertical => {
                    self.acc_y -= threshold * self.acc_y.signum();
                    self.acc_x = 0.0;
                }
            }
            if !self.set.repeats(dir) {
                self.acc_x = 0.0;
                self.acc_y = 0.0;
                break;
            }
        }
        out
    }

    /// The gesture button was released.
    pub fn release(&mut self) -> Vec<GestureEvent> {
        let threshold = self.threshold();
        match self.set.mode {
            GestureMode::Pan | GestureMode::Zoom => {
                if self.travel < PAN_CLICK_SLOP_MM {
                    vec![GestureEvent::Click]
                } else {
                    Vec::new()
                }
            }
            GestureMode::Directional => match self.set.trigger {
                GestureTrigger::Release => {
                    let magnitude = (self.acc_x * self.acc_x + self.acc_y * self.acc_y).sqrt();
                    if magnitude >= threshold {
                        vec![GestureEvent::Fire(direction_of(self.acc_x, self.acc_y).0)]
                    } else if self.travel < threshold * 1.5 {
                        vec![GestureEvent::Click]
                    } else {
                        Vec::new()
                    }
                }
                GestureTrigger::Move => {
                    if !self.fired && self.travel < threshold * 1.5 {
                        vec![GestureEvent::Click]
                    } else {
                        Vec::new()
                    }
                }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tracker(f: impl FnOnce(&mut GestureSet)) -> GestureTracker {
        let mut set = GestureSet { threshold_mm: 5.0, ..Default::default() };
        f(&mut set);
        GestureTracker::new(set, false)
    }

    /// Counts at 1000 DPI for a distance in millimetres.
    fn mm(v: f32) -> i16 {
        (v / 25.4 * 1000.0).round() as i16
    }

    #[test]
    fn click_without_movement() {
        let mut t = tracker(|_| {});
        assert!(t.motion(mm(0.5), mm(-0.5), 1000).is_empty());
        assert_eq!(t.release(), vec![GestureEvent::Click]);
    }

    #[test]
    fn fires_once_on_threshold() {
        let mut t = tracker(|_| {});
        assert!(t.motion(mm(-3.0), 0, 1000).is_empty());
        assert_eq!(t.motion(mm(-3.0), 0, 1000), vec![GestureEvent::Fire(Direction::Left)]);
        // Further movement does nothing without repeat.
        assert!(t.motion(mm(-20.0), 0, 1000).is_empty());
        assert!(t.motion(0, mm(20.0), 1000).is_empty());
        assert!(t.release().is_empty());
    }

    #[test]
    fn up_is_negative_y() {
        let mut t = tracker(|_| {});
        assert_eq!(t.motion(mm(1.0), mm(-8.0), 1000), vec![GestureEvent::Fire(Direction::Up)]);
    }

    #[test]
    fn repeat_vertical() {
        let mut t = tracker(|s| s.repeat_vertical = true);
        let ev = t.motion(0, mm(16.0), 1000);
        assert_eq!(ev, vec![GestureEvent::Fire(Direction::Down); 3]);
        // Reversing keeps repeating along the same axis.
        let ev = t.motion(0, mm(-11.5), 1000);
        assert_eq!(ev, vec![GestureEvent::Fire(Direction::Up); 2]);
        // But never switches to the horizontal axis.
        assert!(t.motion(mm(30.0), 0, 1000).is_empty());
    }

    #[test]
    fn release_trigger() {
        let mut t = tracker(|s| s.trigger = GestureTrigger::Release);
        assert!(t.motion(mm(10.0), mm(2.0), 1000).is_empty());
        assert_eq!(t.release(), vec![GestureEvent::Fire(Direction::Right)]);
    }

    #[test]
    fn skip_first_report() {
        let set = GestureSet { threshold_mm: 5.0, ..Default::default() };
        let mut t = GestureTracker::new(set, true);
        assert!(t.motion(mm(50.0), 0, 1000).is_empty());
        assert_eq!(t.release(), vec![GestureEvent::Click]);
    }

    #[test]
    fn zoom_follows_vertical_movement() {
        let mut t = tracker(|s| s.mode = GestureMode::Zoom);
        let mut zoom = 1.0;
        for _ in 0..30 {
            for e in t.motion(mm(0.3), mm(-1.0), 1000) {
                if let GestureEvent::Zoom(f) = e {
                    zoom *= f;
                }
            }
        }
        // 30 mm up doubles the zoom; sideways movement does nothing.
        assert!((zoom - 2.0).abs() < 0.02, "{zoom}");
        assert!(t.motion(mm(5.0), 0, 1000).is_empty());
        assert!(matches!(t.motion(0, mm(3.0), 1000)[..], [GestureEvent::Zoom(f)] if f < 1.0));
        assert!(t.release().is_empty());
        let mut t = tracker(|s| s.mode = GestureMode::Zoom);
        assert_eq!(t.release(), vec![GestureEvent::Click]);
    }

    #[test]
    fn pan_scrolls_and_keeps_remainders() {
        let mut t = tracker(|s| s.mode = GestureMode::Pan);
        let mut v = 0;
        for _ in 0..30 {
            for e in t.motion(0, mm(0.1), 1000) {
                if let GestureEvent::Scroll { vertical, .. } = e {
                    v += vertical;
                }
            }
        }
        // 3 mm down at 40 units/mm is about one notch down.
        assert!((-125..=-115).contains(&v), "{v}");
        assert!(t.release().is_empty());
    }
}
