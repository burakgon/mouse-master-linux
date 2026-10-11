//! Built-in actions. Each has a stable id used in configuration files, a label
//! and icon for the app, and an implementation that may depend on the desktop.

use evdev::KeyCode as K;
use serde::Serialize;

use super::MouseButton;
use crate::env::Desktop;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Category {
    Mouse,
    Navigation,
    Editing,
    Browser,
    Zoom,
    Media,
    System,
    Desktop,
    Window,
    Apps,
    Device,
}

impl Category {
    pub fn label(self) -> &'static str {
        match self {
            Category::Mouse => "Mouse",
            Category::Navigation => "Navigation",
            Category::Editing => "Editing",
            Category::Browser => "Browser & tabs",
            Category::Zoom => "Zoom",
            Category::Media => "Media",
            Category::System => "System",
            Category::Desktop => "Desktop",
            Category::Window => "Windows",
            Category::Apps => "Apps",
            Category::Device => "Device",
        }
    }

    pub const ALL: [Category; 11] = [
        Category::Mouse,
        Category::Navigation,
        Category::Editing,
        Category::Browser,
        Category::Zoom,
        Category::Media,
        Category::Desktop,
        Category::Window,
        Category::System,
        Category::Apps,
        Category::Device,
    ];
}

/// Desktop specific operations, resolved at run time for the current desktop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DesktopOp {
    Overview,
    ShowDesktop,
    PresentWindows,
    PresentAllWindows,
    DesktopGrid,
    WorkspaceLeft,
    WorkspaceRight,
    WorkspaceUp,
    WorkspaceDown,
    WindowToNextWorkspace,
    WindowToPreviousWorkspace,
    CloseWindow,
    MaximizeWindow,
    MinimizeWindow,
    FullscreenWindow,
    TileLeft,
    TileRight,
    ToggleFloating,
    AppLauncher,
    Search,
    LockScreen,
    Screenshot,
    ScreenshotRegion,
    EmojiPicker,
    ClipboardHistory,
    NightLight,
    DoNotDisturb,
    ScreenOff,
}

/// Device side operations, handled by the device the action came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeviceOp {
    DpiCycle,
    DpiUp,
    DpiDown,
    WheelModeToggle,
    SmartShiftToggle,
    InvertScrollToggle,
    ShowBattery,
    NextHost,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Impl {
    /// Press and release a key combination.
    Keys(&'static [K]),
    /// Click a mouse button.
    Click(MouseButton),
    DoubleClick,
    /// Scroll by high resolution units (120 per notch).
    Scroll { vertical: i32, horizontal: i32 },
    /// Ctrl + scroll wheel, the most widely supported zoom gesture.
    ZoomWheel(i32),
    Desktop(DesktopOp),
    Device(DeviceOp),
    /// Launch the Mouse Master app.
    OpenApp,
    /// Run a fixed command.
    Command(&'static str),
}

#[derive(Debug, Clone, Copy)]
pub struct Builtin {
    pub id: &'static str,
    pub label: &'static str,
    pub category: Category,
    pub icon: &'static str,
    pub imp: Impl,
}

macro_rules! b {
    ($id:literal, $label:literal, $cat:ident, $icon:literal, $imp:expr) => {
        Builtin { id: $id, label: $label, category: Category::$cat, icon: $icon, imp: $imp }
    };
}

use Impl::*;

pub static BUILTINS: &[Builtin] = &[
    // Mouse
    b!("left-click", "Left click", Mouse, "mouse-pointer-click", Click(MouseButton::Left)),
    b!("right-click", "Right click", Mouse, "mouse-pointer-2", Click(MouseButton::Right)),
    b!("middle-click", "Middle click", Mouse, "mouse", Click(MouseButton::Middle)),
    b!("double-click", "Double click", Mouse, "mouse-pointer-click", DoubleClick),
    b!("back", "Back", Navigation, "arrow-left", Click(MouseButton::Back)),
    b!("forward", "Forward", Navigation, "arrow-right", Click(MouseButton::Forward)),
    b!("scroll-up", "Scroll up", Mouse, "chevron-up", Scroll { vertical: 120, horizontal: 0 }),
    b!("scroll-down", "Scroll down", Mouse, "chevron-down", Scroll { vertical: -120, horizontal: 0 }),
    b!("scroll-left", "Scroll left", Mouse, "chevron-left", Scroll { vertical: 0, horizontal: -120 }),
    b!("scroll-right", "Scroll right", Mouse, "chevron-right", Scroll { vertical: 0, horizontal: 120 }),
    // Navigation
    b!("page-up", "Page up", Navigation, "chevrons-up", Keys(&[K::KEY_PAGEUP])),
    b!("page-down", "Page down", Navigation, "chevrons-down", Keys(&[K::KEY_PAGEDOWN])),
    b!("home", "Home", Navigation, "arrow-up-to-line", Keys(&[K::KEY_HOME])),
    b!("end", "End", Navigation, "arrow-down-to-line", Keys(&[K::KEY_END])),
    b!("enter", "Enter", Navigation, "corner-down-left", Keys(&[K::KEY_ENTER])),
    b!("escape", "Escape", Navigation, "x", Keys(&[K::KEY_ESC])),
    b!("app-switcher", "Switch applications", Navigation, "app-window", Keys(&[K::KEY_LEFTALT, K::KEY_TAB])),
    // Editing
    b!("copy", "Copy", Editing, "copy", Keys(&[K::KEY_LEFTCTRL, K::KEY_C])),
    b!("cut", "Cut", Editing, "scissors", Keys(&[K::KEY_LEFTCTRL, K::KEY_X])),
    b!("paste", "Paste", Editing, "clipboard-paste", Keys(&[K::KEY_LEFTCTRL, K::KEY_V])),
    b!("undo", "Undo", Editing, "undo-2", Keys(&[K::KEY_LEFTCTRL, K::KEY_Z])),
    b!("redo", "Redo", Editing, "redo-2", Keys(&[K::KEY_LEFTCTRL, K::KEY_LEFTSHIFT, K::KEY_Z])),
    b!("select-all", "Select all", Editing, "text-select", Keys(&[K::KEY_LEFTCTRL, K::KEY_A])),
    b!("find", "Find", Editing, "search", Keys(&[K::KEY_LEFTCTRL, K::KEY_F])),
    b!("save", "Save", Editing, "save", Keys(&[K::KEY_LEFTCTRL, K::KEY_S])),
    b!("delete", "Delete", Editing, "delete", Keys(&[K::KEY_DELETE])),
    // Browser & tabs
    b!("new-tab", "New tab", Browser, "square-plus", Keys(&[K::KEY_LEFTCTRL, K::KEY_T])),
    b!("close-tab", "Close tab", Browser, "square-x", Keys(&[K::KEY_LEFTCTRL, K::KEY_W])),
    b!("reopen-tab", "Reopen closed tab", Browser, "rotate-ccw", Keys(&[K::KEY_LEFTCTRL, K::KEY_LEFTSHIFT, K::KEY_T])),
    b!("next-tab", "Next tab", Browser, "arrow-right-to-line", Keys(&[K::KEY_LEFTCTRL, K::KEY_TAB])),
    b!("previous-tab", "Previous tab", Browser, "arrow-left-to-line", Keys(&[K::KEY_LEFTCTRL, K::KEY_LEFTSHIFT, K::KEY_TAB])),
    b!("refresh", "Refresh", Browser, "refresh-cw", Keys(&[K::KEY_F5])),
    b!("fullscreen", "Full screen", Browser, "maximize", Keys(&[K::KEY_F11])),
    // Zoom
    b!("zoom-in", "Zoom in", Zoom, "zoom-in", ZoomWheel(120)),
    b!("zoom-out", "Zoom out", Zoom, "zoom-out", ZoomWheel(-120)),
    b!("zoom-reset", "Reset zoom", Zoom, "search", Keys(&[K::KEY_LEFTCTRL, K::KEY_0])),
    // Media
    b!("play-pause", "Play / pause", Media, "play", Keys(&[K::KEY_PLAYPAUSE])),
    b!("next-track", "Next track", Media, "skip-forward", Keys(&[K::KEY_NEXTSONG])),
    b!("previous-track", "Previous track", Media, "skip-back", Keys(&[K::KEY_PREVIOUSSONG])),
    b!("stop-media", "Stop", Media, "square", Keys(&[K::KEY_STOPCD])),
    b!("volume-up", "Volume up", Media, "volume-2", Keys(&[K::KEY_VOLUMEUP])),
    b!("volume-down", "Volume down", Media, "volume-1", Keys(&[K::KEY_VOLUMEDOWN])),
    b!("mute", "Mute", Media, "volume-x", Keys(&[K::KEY_MUTE])),
    b!("mic-mute", "Mute microphone", Media, "mic-off", Keys(&[K::KEY_MICMUTE])),
    // Desktop
    b!("overview", "Overview", Desktop, "layout-grid", Desktop(DesktopOp::Overview)),
    b!("show-desktop", "Show desktop", Desktop, "monitor", Desktop(DesktopOp::ShowDesktop)),
    b!("present-windows", "Present windows", Desktop, "layout-dashboard", Desktop(DesktopOp::PresentWindows)),
    b!("present-all-windows", "Present all windows", Desktop, "layout-dashboard", Desktop(DesktopOp::PresentAllWindows)),
    b!("desktop-grid", "Desktop grid", Desktop, "grid-2x2", Desktop(DesktopOp::DesktopGrid)),
    b!("workspace-left", "Previous desktop", Desktop, "arrow-big-left", Desktop(DesktopOp::WorkspaceLeft)),
    b!("workspace-right", "Next desktop", Desktop, "arrow-big-right", Desktop(DesktopOp::WorkspaceRight)),
    b!("workspace-up", "Desktop above", Desktop, "arrow-big-up", Desktop(DesktopOp::WorkspaceUp)),
    b!("workspace-down", "Desktop below", Desktop, "arrow-big-down", Desktop(DesktopOp::WorkspaceDown)),
    b!("app-launcher", "Application launcher", Desktop, "layout-grid", Desktop(DesktopOp::AppLauncher)),
    b!("search", "Search", Desktop, "search", Desktop(DesktopOp::Search)),
    b!("emoji-picker", "Emoji picker", Desktop, "smile", Desktop(DesktopOp::EmojiPicker)),
    b!("clipboard-history", "Clipboard history", Desktop, "clipboard-list", Desktop(DesktopOp::ClipboardHistory)),
    b!("screenshot", "Screenshot", Desktop, "camera", Desktop(DesktopOp::Screenshot)),
    b!("screenshot-region", "Screenshot of a region", Desktop, "scan", Desktop(DesktopOp::ScreenshotRegion)),
    // Windows
    b!("close-window", "Close window", Window, "x", Desktop(DesktopOp::CloseWindow)),
    b!("maximize-window", "Maximize window", Window, "maximize-2", Desktop(DesktopOp::MaximizeWindow)),
    b!("minimize-window", "Minimize window", Window, "minimize-2", Desktop(DesktopOp::MinimizeWindow)),
    b!("fullscreen-window", "Full screen window", Window, "expand", Desktop(DesktopOp::FullscreenWindow)),
    b!("tile-left", "Tile window left", Window, "panel-left", Desktop(DesktopOp::TileLeft)),
    b!("tile-right", "Tile window right", Window, "panel-right", Desktop(DesktopOp::TileRight)),
    b!("toggle-floating", "Toggle floating", Window, "picture-in-picture-2", Desktop(DesktopOp::ToggleFloating)),
    b!("window-next-desktop", "Move window to next desktop", Window, "arrow-right-from-line", Desktop(DesktopOp::WindowToNextWorkspace)),
    b!("window-previous-desktop", "Move window to previous desktop", Window, "arrow-left-from-line", Desktop(DesktopOp::WindowToPreviousWorkspace)),
    // System
    b!("brightness-up", "Brightness up", System, "sun", Keys(&[K::KEY_BRIGHTNESSUP])),
    b!("brightness-down", "Brightness down", System, "sun-dim", Keys(&[K::KEY_BRIGHTNESSDOWN])),
    b!("lock-screen", "Lock screen", System, "lock", Desktop(DesktopOp::LockScreen)),
    b!("screen-off", "Turn off screen", System, "monitor-off", Desktop(DesktopOp::ScreenOff)),
    b!("night-light", "Toggle night light", System, "moon", Desktop(DesktopOp::NightLight)),
    b!("do-not-disturb", "Toggle do not disturb", System, "bell-off", Desktop(DesktopOp::DoNotDisturb)),
    // Apps
    b!("calculator", "Calculator", Apps, "calculator", Keys(&[K::KEY_CALC])),
    b!("file-manager", "File manager", Apps, "folder", Command("xdg-open \"$HOME\"")),
    b!("mouse-master", "Mouse Master", Apps, "mouse", OpenApp),
    // Device
    b!("dpi-cycle", "Cycle pointer speed", Device, "gauge", Device(DeviceOp::DpiCycle)),
    b!("dpi-up", "Pointer speed up", Device, "gauge", Device(DeviceOp::DpiUp)),
    b!("dpi-down", "Pointer speed down", Device, "gauge", Device(DeviceOp::DpiDown)),
    b!("wheel-mode-toggle", "Toggle ratchet / free spin", Device, "disc-3", Device(DeviceOp::WheelModeToggle)),
    b!("smartshift-toggle", "Toggle SmartShift", Device, "disc-3", Device(DeviceOp::SmartShiftToggle)),
    b!("invert-scroll-toggle", "Toggle scroll direction", Device, "arrow-down-up", Device(DeviceOp::InvertScrollToggle)),
    b!("show-battery", "Show battery level", Device, "battery-medium", Device(DeviceOp::ShowBattery)),
    b!("next-host", "Switch to next computer", Device, "monitor-smartphone", Device(DeviceOp::NextHost)),
];

pub fn find(id: &str) -> Option<&'static Builtin> {
    BUILTINS.iter().find(|b| b.id == id)
}

/// Whether a desktop operation can be performed on the given desktop.
pub fn desktop_op_available(op: DesktopOp, desktop: Desktop) -> bool {
    use DesktopOp::*;
    match desktop {
        Desktop::Kde => !matches!(op, ToggleFloating),
        // Hyprland's `dpms off` does not wake up on input by default, so screen off is not offered.
        Desktop::Hyprland => !matches!(
            op,
            ScreenOff
                | ShowDesktop
                | PresentWindows
                | PresentAllWindows
                | DesktopGrid
                | WorkspaceUp
                | WorkspaceDown
                | MinimizeWindow
                | EmojiPicker
                | ClipboardHistory
                | NightLight
                | DoNotDisturb
        ),
        Desktop::Gnome => !matches!(
            op,
            PresentWindows
                | DesktopGrid
                | TileLeft
                | TileRight
                | ToggleFloating
                | EmojiPicker
                | ClipboardHistory
                | NightLight
                | DoNotDisturb
                | ScreenOff
        ),
        Desktop::Sway => matches!(
            op,
            WorkspaceLeft
                | WorkspaceRight
                | WindowToNextWorkspace
                | WindowToPreviousWorkspace
                | CloseWindow
                | FullscreenWindow
                | ToggleFloating
                | LockScreen
                | AppLauncher
        ),
        Desktop::Other => matches!(op, LockScreen | CloseWindow),
    }
}

pub fn available(b: &Builtin, desktop: Desktop) -> bool {
    match b.imp {
        Impl::Desktop(op) => desktop_op_available(op, desktop),
        _ => true,
    }
}

#[derive(Serialize)]
pub struct CatalogEntry {
    pub id: &'static str,
    pub label: &'static str,
    pub category: Category,
    pub icon: &'static str,
    pub available: bool,
}

#[derive(Serialize)]
pub struct CatalogCategory {
    pub id: Category,
    pub label: &'static str,
}

#[derive(Serialize)]
pub struct Catalog {
    pub desktop: Desktop,
    pub categories: Vec<CatalogCategory>,
    pub actions: Vec<CatalogEntry>,
}

pub fn catalog(desktop: Desktop) -> Catalog {
    Catalog {
        desktop,
        categories: Category::ALL.iter().map(|c| CatalogCategory { id: *c, label: c.label() }).collect(),
        actions: BUILTINS
            .iter()
            .map(|b| CatalogEntry { id: b.id, label: b.label, category: b.category, icon: b.icon, available: available(b, desktop) })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique() {
        let mut ids: Vec<_> = BUILTINS.iter().map(|b| b.id).collect();
        ids.sort();
        let n = ids.len();
        ids.dedup();
        assert_eq!(n, ids.len());
    }

    #[test]
    fn defaults_exist() {
        for id in ["overview", "show-desktop", "workspace-left", "workspace-right", "copy", "dpi-cycle"] {
            assert!(find(id).is_some(), "{id}");
        }
    }
}
