//! Executes actions that are not tied to a device: key presses, scrolling,
//! desktop operations, commands. Device operations (DPI, SmartShift, host
//! switching) are handed back to the device that triggered the action.

use std::sync::Arc;
use std::time::{Duration, Instant};

use evdev::KeyCode as K;
use parking_lot::Mutex;
use tokio::sync::watch;
use tracing::{debug, info, warn};

use super::catalog::{self, DesktopOp, DeviceOp, Impl};
use super::keys::parse_combo;
use super::{Action, MacroStep};
use crate::env::{self, Desktop};
use crate::hyprland;
use crate::input::SharedInput;

/// A request for the device that triggered an action.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DeviceRequest {
    Op(DeviceOp),
    SetDpi(u16),
    /// Temporary DPI while a control is held; `None` restores the normal DPI.
    ShiftDpi(Option<u16>),
    SwitchHost(u8),
}

#[derive(Clone)]
pub struct Executor {
    input: Option<SharedInput>,
    conn: Option<zbus::Connection>,
    desktop: watch::Receiver<Desktop>,
    /// When KWin was last asked whether the touchpad is enabled.
    touchpad_checked: Arc<Mutex<Option<Instant>>>,
}

impl Executor {
    pub fn new(
        input: Option<SharedInput>,
        conn: Option<zbus::Connection>,
        desktop: watch::Receiver<Desktop>,
    ) -> Executor {
        Executor { input, conn, desktop, touchpad_checked: Default::default() }
    }

    pub fn desktop(&self) -> Desktop {
        *self.desktop.borrow()
    }

    pub fn input(&self) -> Option<&SharedInput> {
        self.input.as_ref()
    }

    fn with_input(&self, f: impl FnOnce(&SharedInput)) {
        match &self.input {
            Some(input) => f(input),
            None => warn!("virtual input is unavailable (no access to /dev/uinput)"),
        }
    }

    /// Zooms like a touchpad pinch (see [`crate::input::InputSink::pinch`]).
    pub async fn pinch(&self, factor: f64) {
        self.keep_touchpad_enabled().await;
        self.with_input(|i| i.pinch(factor));
    }

    pub fn end_pinch(&self) {
        if let Some(input) = &self.input {
            input.end_pinch();
        }
    }

    /// KWin's "Toggle Touchpad" switches every touchpad off, the virtual one
    /// included, which would quietly stop pinch zoom; switch it back on.
    async fn keep_touchpad_enabled(&self) {
        if self.desktop() != Desktop::Kde {
            return;
        }
        let (Some(conn), Some(input)) = (&self.conn, &self.input) else { return };
        let Some(name) = input.touchpad_sys_name() else { return };
        {
            let mut checked = self.touchpad_checked.lock();
            if checked.is_some_and(|t| t.elapsed() < Duration::from_secs(2)) {
                return;
            }
            *checked = Some(Instant::now());
        }
        let path = format!("/org/kde/KWin/InputDevice/{name}");
        let (dest, props, iface) =
            (Some("org.kde.KWin"), Some("org.freedesktop.DBus.Properties"), "org.kde.KWin.InputDevice");
        let enabled = conn
            .call_method(dest, path.as_str(), props, "Get", &(iface, "enabled"))
            .await
            .ok()
            .and_then(|m| m.body().deserialize::<zbus::zvariant::OwnedValue>().ok())
            .and_then(|v| bool::try_from(v).ok());
        if enabled == Some(false) {
            info!("turning the virtual touchpad back on (the touchpad toggle switched it off)");
            let value = zbus::zvariant::Value::from(true);
            if let Err(e) = conn.call_method(dest, path.as_str(), props, "Set", &(iface, "enabled", value)).await {
                warn!("cannot turn the virtual touchpad back on: {e}");
            }
        }
    }

    /// The control was pressed.
    pub fn press(&self, action: &Action) -> Option<DeviceRequest> {
        match action {
            Action::Keys { keys } => {
                match parse_combo(keys) {
                    Ok(keys) => self.with_input(|i| i.press(&keys)),
                    Err(e) => warn!("{e}"),
                }
                None
            }
            Action::Mouse { button } => {
                self.with_input(|i| i.set_key(button.code(), true));
                None
            }
            Action::ShiftDpi { dpi } => Some(DeviceRequest::ShiftDpi(Some(*dpi))),
            Action::Default | Action::None | Action::Gestures(_) => None,
            other => self.run(other),
        }
    }

    /// The control was released.
    pub fn release(&self, action: &Action) -> Option<DeviceRequest> {
        match action {
            Action::Keys { keys } => {
                if let Ok(keys) = parse_combo(keys) {
                    self.with_input(|i| i.release(&keys));
                }
                None
            }
            Action::Mouse { button } => {
                self.with_input(|i| i.set_key(button.code(), false));
                None
            }
            Action::ShiftDpi { .. } => Some(DeviceRequest::ShiftDpi(None)),
            _ => None,
        }
    }

    /// Runs an action once (gesture directions, wheel steps, tests).
    pub fn run(&self, action: &Action) -> Option<DeviceRequest> {
        match action {
            Action::Default | Action::None | Action::Gestures(_) => None,
            Action::Builtin { id } => self.run_builtin(id),
            Action::Keys { keys } => {
                match parse_combo(keys) {
                    Ok(keys) => self.with_input(|i| i.tap(&keys)),
                    Err(e) => warn!("{e}"),
                }
                None
            }
            Action::Mouse { button } => {
                self.with_input(|i| i.click(button.code()));
                None
            }
            Action::Command { command } => {
                let (conn, desktop, command) = (self.conn.clone(), self.desktop(), command.clone());
                tokio::spawn(async move { env::spawn_shell(conn.as_ref(), desktop, &command).await });
                None
            }
            Action::Open { target } => {
                let (conn, desktop, target) = (self.conn.clone(), self.desktop(), target.clone());
                tokio::spawn(async move { env::open(conn.as_ref(), desktop, &target).await });
                None
            }
            Action::SetDpi { dpi } | Action::ShiftDpi { dpi } => Some(DeviceRequest::SetDpi(*dpi)),
            Action::SwitchHost { host } => Some(DeviceRequest::SwitchHost(*host)),
            Action::Macro { steps } => {
                let this = self.clone();
                let steps = steps.clone();
                tokio::spawn(async move { this.run_macro(&steps).await });
                None
            }
        }
    }

    async fn run_macro(&self, steps: &[MacroStep]) {
        for step in steps {
            match step {
                MacroStep::Keys { keys } => match parse_combo(keys) {
                    Ok(keys) => self.with_input(|i| i.tap(&keys)),
                    Err(e) => warn!("macro: {e}"),
                },
                MacroStep::Builtin { id } => {
                    if self.run_builtin(id).is_some() {
                        debug!("macro: device action {id} ignored");
                    }
                }
                MacroStep::Click { button } => self.with_input(|i| i.click(button.code())),
                MacroStep::Delay { ms } => tokio::time::sleep(Duration::from_millis((*ms).min(60_000) as u64)).await,
                MacroStep::Command { command } => env::spawn_shell(self.conn.as_ref(), self.desktop(), command).await,
            }
            // A small gap so applications see distinct events.
            tokio::time::sleep(Duration::from_millis(8)).await;
        }
    }

    pub fn run_builtin(&self, id: &str) -> Option<DeviceRequest> {
        let Some(builtin) = catalog::find(id) else {
            warn!("unknown built-in action {id:?}");
            return None;
        };
        match builtin.imp {
            Impl::Keys(keys) => self.with_input(|i| i.tap(keys)),
            Impl::Click(b) => self.with_input(|i| i.click(b.code())),
            Impl::DoubleClick => self.with_input(|i| {
                i.click(K::BTN_LEFT);
                i.click(K::BTN_LEFT);
            }),
            Impl::Scroll { vertical, horizontal } => self.with_input(|i| i.scroll(vertical, horizontal)),
            Impl::ZoomWheel(amount) => self.with_input(|i| i.zoom(amount)),
            Impl::Device(op) => return Some(DeviceRequest::Op(op)),
            Impl::Desktop(op) => {
                let this = self.clone();
                tokio::spawn(async move { this.desktop_op(op).await });
            }
            Impl::OpenApp => {
                let (conn, desktop) = (self.conn.clone(), self.desktop());
                tokio::spawn(async move { env::spawn_shell(conn.as_ref(), desktop, "mouse-master").await });
            }
            Impl::Command(command) => {
                let (conn, desktop) = (self.conn.clone(), self.desktop());
                tokio::spawn(async move { env::spawn_shell(conn.as_ref(), desktop, command).await });
            }
        }
        None
    }

    async fn kde_shortcut(&self, component: &str, name: &str) {
        let Some(conn) = &self.conn else { return };
        let path = format!("/component/{component}");
        let result = conn
            .call_method(
                Some("org.kde.kglobalaccel"),
                path.as_str(),
                Some("org.kde.kglobalaccel.Component"),
                "invokeShortcut",
                &(name,),
            )
            .await;
        if let Err(e) = result {
            warn!("KDE shortcut {component}/{name} failed: {e}");
        }
    }

    async fn hypr(&self, dispatcher: &str) {
        match hyprland::Instance::find() {
            Some(instance) => {
                if let Err(e) = instance.dispatch(dispatcher).await {
                    warn!("hyprland dispatch {dispatcher:?} failed: {e}");
                }
            }
            None => warn!("Hyprland is not running"),
        }
    }

    async fn shell(&self, command: &str) {
        env::spawn_shell(self.conn.as_ref(), self.desktop(), command).await;
    }

    fn keys(&self, keys: &[K]) {
        self.with_input(|i| i.tap(keys));
    }

    /// Runs the first launcher found in PATH.
    async fn launcher(&self) {
        const LAUNCHERS: &[(&str, &str)] = &[
            ("fuzzel", "fuzzel"),
            ("walker", "walker"),
            ("anyrun", "anyrun"),
            ("wofi", "wofi --show drun"),
            ("rofi", "rofi -show drun"),
            ("tofi-drun", "tofi-drun --drun-launch=true"),
            ("bemenu-run", "bemenu-run"),
        ];
        let path = env::session_environment(self.conn.as_ref()).await.remove("PATH").unwrap_or_default();
        for (bin, cmd) in LAUNCHERS {
            if path.split(':').any(|d| std::path::Path::new(d).join(bin).is_file()) {
                self.shell(cmd).await;
                return;
            }
        }
        warn!("no application launcher found (install fuzzel, wofi or rofi)");
    }

    async fn screenshot(&self, region: bool) {
        let path = env::session_environment(self.conn.as_ref()).await.remove("PATH").unwrap_or_default();
        let has = |bin: &str| path.split(':').any(|d| std::path::Path::new(d).join(bin).is_file());
        let cmd = if has("hyprshot") {
            if region { "hyprshot -m region".to_string() } else { "hyprshot -m output".to_string() }
        } else if has("grim") {
            let file = "\"$(xdg-user-dir PICTURES 2>/dev/null || echo $HOME)/Screenshot_$(date +%Y%m%d_%H%M%S).png\"";
            if region && has("slurp") { format!("grim -g \"$(slurp)\" {file}") } else { format!("grim {file}") }
        } else {
            warn!("no screenshot tool found (install grim and slurp)");
            return;
        };
        self.shell(&cmd).await;
    }

    pub async fn desktop_op(&self, op: DesktopOp) {
        use DesktopOp::*;
        let desktop = self.desktop();
        if !catalog::desktop_op_available(op, desktop) {
            warn!("{op:?} is not available on {}", desktop.name());
            return;
        }
        match desktop {
            Desktop::Kde => {
                let (component, name) = match op {
                    Overview => ("kwin", "Overview"),
                    ShowDesktop => ("kwin", "Show Desktop"),
                    PresentWindows => ("kwin", "Expose"),
                    PresentAllWindows => ("kwin", "ExposeAll"),
                    DesktopGrid => ("kwin", "Grid View"),
                    WorkspaceLeft => ("kwin", "Switch One Desktop to the Left"),
                    WorkspaceRight => ("kwin", "Switch One Desktop to the Right"),
                    WorkspaceUp => ("kwin", "Switch One Desktop Up"),
                    WorkspaceDown => ("kwin", "Switch One Desktop Down"),
                    WindowToNextWorkspace => ("kwin", "Window to Next Desktop"),
                    WindowToPreviousWorkspace => ("kwin", "Window to Previous Desktop"),
                    CloseWindow => ("kwin", "Window Close"),
                    MaximizeWindow => ("kwin", "Window Maximize"),
                    MinimizeWindow => ("kwin", "Window Minimize"),
                    FullscreenWindow => ("kwin", "Window Fullscreen"),
                    TileLeft => ("kwin", "Window Quick Tile Left"),
                    TileRight => ("kwin", "Window Quick Tile Right"),
                    NightLight => ("kwin", "Toggle Night Color"),
                    AppLauncher => ("plasmashell", "activate application launcher"),
                    ClipboardHistory => ("plasmashell", "show-on-mouse-pos"),
                    DoNotDisturb => ("plasmashell", "toggle do not disturb"),
                    Search => ("org_kde_krunner_desktop", "_launch"),
                    LockScreen => ("ksmserver", "Lock Session"),
                    Screenshot => ("org_kde_spectacle_desktop", "FullScreenScreenShot"),
                    ScreenshotRegion => ("org_kde_spectacle_desktop", "RectangularRegionScreenShot"),
                    EmojiPicker => ("org_kde_plasma_emojier_desktop", "_launch"),
                    ScreenOff => ("org_kde_powerdevil", "Turn Off Screen"),
                    ToggleFloating => return,
                };
                self.kde_shortcut(component, name).await;
            }
            Desktop::Hyprland => match op {
                Overview => self.hypr("hyprexpo:expo toggle").await,
                WorkspaceLeft => self.hypr("workspace e-1").await,
                WorkspaceRight => self.hypr("workspace e+1").await,
                WindowToNextWorkspace => self.hypr("movetoworkspace e+1").await,
                WindowToPreviousWorkspace => self.hypr("movetoworkspace e-1").await,
                CloseWindow => self.hypr("killactive").await,
                MaximizeWindow => self.hypr("fullscreen 1").await,
                FullscreenWindow => self.hypr("fullscreen 0").await,
                TileLeft => self.hypr("movewindow l").await,
                TileRight => self.hypr("movewindow r").await,
                ToggleFloating => self.hypr("togglefloating").await,
                AppLauncher | Search => self.launcher().await,
                LockScreen => self.shell("loginctl lock-session").await,
                Screenshot => self.screenshot(false).await,
                ScreenshotRegion => self.screenshot(true).await,
                _ => {}
            },
            Desktop::Gnome => match op {
                Overview | Search => self.keys(&[K::KEY_LEFTMETA]),
                ShowDesktop => self.keys(&[K::KEY_LEFTMETA, K::KEY_D]),
                WorkspaceLeft => self.keys(&[K::KEY_LEFTMETA, K::KEY_PAGEUP]),
                WorkspaceRight => self.keys(&[K::KEY_LEFTMETA, K::KEY_PAGEDOWN]),
                WorkspaceUp => self.keys(&[K::KEY_LEFTMETA, K::KEY_PAGEUP]),
                WorkspaceDown => self.keys(&[K::KEY_LEFTMETA, K::KEY_PAGEDOWN]),
                WindowToNextWorkspace => self.keys(&[K::KEY_LEFTMETA, K::KEY_LEFTSHIFT, K::KEY_PAGEDOWN]),
                WindowToPreviousWorkspace => self.keys(&[K::KEY_LEFTMETA, K::KEY_LEFTSHIFT, K::KEY_PAGEUP]),
                PresentAllWindows => self.keys(&[K::KEY_LEFTMETA]),
                CloseWindow => self.keys(&[K::KEY_LEFTALT, K::KEY_F4]),
                MaximizeWindow => self.keys(&[K::KEY_LEFTMETA, K::KEY_UP]),
                MinimizeWindow => self.keys(&[K::KEY_LEFTMETA, K::KEY_H]),
                FullscreenWindow => self.keys(&[K::KEY_F11]),
                AppLauncher => self.keys(&[K::KEY_LEFTMETA, K::KEY_A]),
                LockScreen => self.shell("loginctl lock-session").await,
                Screenshot => self.keys(&[K::KEY_SYSRQ]),
                ScreenshotRegion => self.keys(&[K::KEY_LEFTSHIFT, K::KEY_SYSRQ]),
                _ => {}
            },
            Desktop::Sway => match op {
                WorkspaceLeft => self.shell("swaymsg workspace prev_on_output").await,
                WorkspaceRight => self.shell("swaymsg workspace next_on_output").await,
                WindowToNextWorkspace => self.shell("swaymsg move container to workspace next_on_output").await,
                WindowToPreviousWorkspace => self.shell("swaymsg move container to workspace prev_on_output").await,
                CloseWindow => self.shell("swaymsg kill").await,
                FullscreenWindow => self.shell("swaymsg fullscreen toggle").await,
                ToggleFloating => self.shell("swaymsg floating toggle").await,
                AppLauncher => self.launcher().await,
                LockScreen => self.shell("loginctl lock-session").await,
                _ => {}
            },
            Desktop::Other => match op {
                LockScreen => self.shell("loginctl lock-session").await,
                CloseWindow => self.keys(&[K::KEY_LEFTALT, K::KEY_F4]),
                _ => {}
            },
        }
    }
}
