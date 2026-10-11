//! Thumb wheel processing when the wheel is diverted.

use hidpp::features::thumbwheel::{Event, RotationStatus, ThumbwheelInfo};

use crate::actions::Direction;
use crate::config::ThumbwheelBinding;
use crate::input::WHEEL_UNIT;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThumbOutput {
    /// Horizontal high resolution scroll (positive scrolls right).
    Scroll(i32),
    /// One step of an action binding.
    Step(Direction),
    Tap,
}

#[derive(Debug)]
pub struct ThumbwheelTracker {
    binding: ThumbwheelBinding,
    native_resolution: f32,
    diverted_resolution: f32,
    direction: f32,
    invert: bool,
    acc: f32,
    remainder: f32,
}

impl ThumbwheelTracker {
    pub fn new(binding: ThumbwheelBinding, info: &ThumbwheelInfo) -> ThumbwheelTracker {
        ThumbwheelTracker {
            binding,
            native_resolution: info.native_resolution.max(1) as f32,
            diverted_resolution: info.diverted_resolution.max(1) as f32,
            direction: info.default_direction as f32,
            invert: false,
            acc: 0.0,
            remainder: 0.0,
        }
    }

    /// Inverts the rotation direction (for scrolling and action steps alike).
    pub fn inverted(mut self, invert: bool) -> ThumbwheelTracker {
        self.invert = invert;
        self
    }

    pub fn binding(&self) -> &ThumbwheelBinding {
        &self.binding
    }

    pub fn event(&mut self, ev: &Event) -> Vec<ThumbOutput> {
        let mut out = Vec::new();
        if ev.single_tap {
            out.push(ThumbOutput::Tap);
        }
        if ev.status == RotationStatus::Start {
            self.acc = 0.0;
            self.remainder = 0.0;
        }
        if ev.rotation == 0 {
            return out;
        }
        // Positive means "to the right" after applying the device's default direction.
        let sign = if self.invert { -1.0 } else { 1.0 };
        let units = ev.rotation as f32 * self.direction * sign;
        match &self.binding {
            ThumbwheelBinding::Default => {}
            ThumbwheelBinding::Scroll { speed } => {
                // Match native scrolling: one native notch is one wheel unit.
                let per_unit = self.native_resolution * WHEEL_UNIT as f32 / self.diverted_resolution;
                self.remainder += units * per_unit * speed.clamp(0.1, 10.0);
                let whole = self.remainder.trunc() as i32;
                self.remainder -= whole as f32;
                if whole != 0 {
                    out.push(ThumbOutput::Scroll(whole));
                }
            }
            ThumbwheelBinding::Actions { step_degrees, .. } => {
                let step = (step_degrees.clamp(3.0, 180.0) / 360.0 * self.diverted_resolution).max(1.0);
                self.acc += units;
                while self.acc.abs() >= step {
                    let dir = if self.acc > 0.0 { Direction::Right } else { Direction::Left };
                    out.push(ThumbOutput::Step(dir));
                    self.acc -= step * self.acc.signum();
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actions::Action;

    fn info() -> ThumbwheelInfo {
        ThumbwheelInfo {
            native_resolution: 18,
            diverted_resolution: 120,
            default_direction: -1,
            has_timestamp: true,
            has_touch: true,
            has_proximity: true,
            has_single_tap: true,
        }
    }

    fn ev(rotation: i16, status: RotationStatus) -> Event {
        Event { rotation, timestamp: 0, status, touch: true, proximity: true, single_tap: false }
    }

    #[test]
    fn scroll_matches_native_speed() {
        let mut t = ThumbwheelTracker::new(ThumbwheelBinding::Scroll { speed: 1.0 }, &info());
        // A full turn in diverted units is 120; natively that is 18 notches.
        let mut total = 0;
        for _ in 0..12 {
            for o in t.event(&ev(-10, RotationStatus::Active)) {
                if let ThumbOutput::Scroll(v) = o {
                    total += v;
                }
            }
        }
        assert_eq!(total, 18 * 120);
    }

    #[test]
    fn action_steps() {
        let binding = ThumbwheelBinding::Actions {
            left: Action::Builtin { id: "volume-down".into() },
            right: Action::Builtin { id: "volume-up".into() },
            step_degrees: 30.0,
        };
        let mut t = ThumbwheelTracker::new(binding, &info());
        // 30 degrees = 10 diverted units. Rotation -25 with direction -1 is +25 (right).
        let out = t.event(&ev(-25, RotationStatus::Start));
        assert_eq!(out, vec![ThumbOutput::Step(Direction::Right), ThumbOutput::Step(Direction::Right)]);
        let out = t.event(&ev(-5, RotationStatus::Active));
        assert_eq!(out, vec![ThumbOutput::Step(Direction::Right)]);
    }

    #[test]
    fn inverted_swaps_direction() {
        let mut t = ThumbwheelTracker::new(ThumbwheelBinding::Scroll { speed: 1.0 }, &info()).inverted(true);
        let out = t.event(&ev(-10, RotationStatus::Active));
        assert!(matches!(out[0], ThumbOutput::Scroll(v) if v < 0));
    }

    #[test]
    fn tap() {
        let mut t = ThumbwheelTracker::new(ThumbwheelBinding::Default, &info());
        let mut e = ev(0, RotationStatus::Inactive);
        e.single_tap = true;
        assert_eq!(t.event(&e), vec![ThumbOutput::Tap]);
    }
}
