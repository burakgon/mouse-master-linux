//! The action model: what a button, gesture direction or wheel step does.

pub mod catalog;
pub mod exec;
pub mod keys;

use serde::{Deserialize, Serialize};

/// Something a control can do. Serialized with a `type` tag, e.g.
/// `{ type = "builtin", id = "copy" }` or `{ type = "keys", keys = ["KEY_LEFTCTRL", "KEY_C"] }`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum Action {
    /// The control's native function; the control is not diverted.
    #[default]
    Default,
    /// Do nothing (disables the control).
    None,
    /// A built-in action from the [`catalog`].
    Builtin { id: String },
    /// A key combination. Keys are pressed in order and held while the control is held.
    Keys { keys: Vec<String> },
    /// A mouse button, held while the control is held.
    Mouse { button: MouseButton },
    /// A shell command.
    Command { command: String },
    /// A URL, a file or folder, or an application (desktop file id).
    Open { target: String },
    /// Sets the pointer resolution.
    SetDpi { dpi: u16 },
    /// Uses another resolution while the control is held ("precision mode").
    ShiftDpi { dpi: u16 },
    /// Switches the device to another Easy-Switch channel (0 based).
    SwitchHost { host: u8 },
    /// Hold the control and move the mouse.
    Gestures(GestureSet),
    /// A sequence of steps.
    Macro { steps: Vec<MacroStep> },
}

impl Action {
    /// Whether the control must be diverted to run this action.
    pub fn needs_divert(&self) -> bool {
        !matches!(self, Action::Default)
    }

    /// Whether the control must report raw pointer motion while held.
    pub fn needs_raw_xy(&self) -> bool {
        matches!(self, Action::Gestures(_))
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "kebab-case")]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Back,
    Forward,
}

impl MouseButton {
    pub fn code(self) -> evdev::KeyCode {
        use evdev::KeyCode;
        match self {
            MouseButton::Left => KeyCode::BTN_LEFT,
            MouseButton::Right => KeyCode::BTN_RIGHT,
            MouseButton::Middle => KeyCode::BTN_MIDDLE,
            MouseButton::Back => KeyCode::BTN_SIDE,
            MouseButton::Forward => KeyCode::BTN_EXTRA,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "step", rename_all = "kebab-case")]
pub enum MacroStep {
    /// Press and release a key combination.
    Keys { keys: Vec<String> },
    /// Run a built-in action.
    Builtin { id: String },
    /// Click a mouse button.
    Click { button: MouseButton },
    /// Wait.
    Delay { ms: u32 },
    /// Run a command.
    Command { command: String },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum GestureMode {
    /// Up/down/left/right/click, each with its own action.
    #[default]
    Directional,
    /// Moving the mouse scrolls in any direction.
    Pan,
    /// Moving the mouse up zooms in and down zooms out, like a touchpad pinch.
    Zoom,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum GestureTrigger {
    /// Fire as soon as the movement passes the threshold, while still holding.
    #[default]
    Move,
    /// Fire when the button is released.
    Release,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct GestureSet {
    pub mode: GestureMode,
    pub up: Box<Action>,
    pub down: Box<Action>,
    pub left: Box<Action>,
    pub right: Box<Action>,
    /// Pressing and releasing without moving.
    pub click: Box<Action>,
    /// Distance in millimetres the mouse must travel to recognise a direction.
    pub threshold_mm: f32,
    pub trigger: GestureTrigger,
    /// Keep firing up/down actions every threshold while held (volume, zoom, ...).
    pub repeat_vertical: bool,
    /// Keep firing left/right actions every threshold while held.
    pub repeat_horizontal: bool,
    /// Pan mode: scroll speed multiplier.
    pub pan_speed: f32,
    /// Pan mode: invert scrolling (drag the content instead of the view).
    pub pan_invert: bool,
    /// Zoom mode: speed multiplier.
    pub zoom_speed: f32,
}

impl Default for GestureSet {
    fn default() -> Self {
        let b = |id: &str| Box::new(Action::Builtin { id: id.to_string() });
        GestureSet {
            mode: GestureMode::Directional,
            up: b("overview"),
            down: b("show-desktop"),
            left: b("workspace-left"),
            right: b("workspace-right"),
            click: b("overview"),
            threshold_mm: 7.0,
            trigger: GestureTrigger::Move,
            repeat_vertical: false,
            repeat_horizontal: false,
            pan_speed: 1.0,
            pan_invert: false,
            zoom_speed: 1.0,
        }
    }
}

impl GestureSet {
    pub fn action(&self, dir: Direction) -> &Action {
        match dir {
            Direction::Up => &self.up,
            Direction::Down => &self.down,
            Direction::Left => &self.left,
            Direction::Right => &self.right,
        }
    }

    pub fn repeats(&self, dir: Direction) -> bool {
        match dir {
            Direction::Up | Direction::Down => self.repeat_vertical,
            Direction::Left | Direction::Right => self.repeat_horizontal,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_shapes_are_stable() {
        let a = Action::Builtin { id: "copy".into() };
        assert_eq!(serde_json::to_string(&a).unwrap(), r#"{"type":"builtin","id":"copy"}"#);
        let g: Action = serde_json::from_str(r#"{"type":"gestures","mode":"pan","pan_speed":2.0}"#).unwrap();
        match g {
            Action::Gestures(set) => {
                assert_eq!(set.mode, GestureMode::Pan);
                assert_eq!(set.pan_speed, 2.0);
                assert_eq!(*set.click, Action::Builtin { id: "overview".into() });
            }
            other => panic!("unexpected {other:?}"),
        }
        let k: Action = serde_json::from_str(r#"{"type":"keys","keys":["KEY_LEFTCTRL","KEY_C"]}"#).unwrap();
        assert!(k.needs_divert() && !k.needs_raw_xy());
    }
}
