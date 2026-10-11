//! One task per device: owns the HID++ device, applies the configuration,
//! turns events into actions and answers commands from clients.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use hidpp::controls::{self, Role};
use hidpp::features::{
    self, battery, change_host, device_info, dpi, haptic, hires_wheel, hosts_info, id as fid, onboard_profiles,
    report_rate, reprog_controls, smartshift, thumbwheel, wireless_status,
};
use hidpp::{Channel, Device, Params, Report};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use super::snapshot::*;
use super::{Command, DaemonEvent, DeviceMsg, Mailbox, Reply, Shared};
use crate::actions::catalog::DeviceOp;
use crate::actions::exec::DeviceRequest;
use crate::actions::{Action, MouseButton};
use crate::config::{AppProfile, Bindings, DeviceConfig, DeviceSettings, ThumbwheelBinding, WheelModeSetting};
use crate::gestures::{GestureEvent, GestureTracker};
use crate::thumbwheel::{ThumbOutput, ThumbwheelTracker};
use crate::window::ActiveApp;

/// SmartShift thresholds offered to users (above this the wheel is effectively always ratcheted).
const SMARTSHIFT_MAX_THRESHOLD: u8 = 50;

pub fn spawn(shared: Arc<Shared>, channel: Channel, index: u8, connection: ConnectionInfo) -> Mailbox {
    let (tx, rx) = mpsc::unbounded_channel();
    let actor = Actor::new(shared, channel, index, connection, tx.clone());
    tokio::spawn(actor.run(rx));
    tx
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Diversion {
    divert: bool,
    raw_xy: bool,
}

struct Actor {
    shared: Arc<Shared>,
    channel: Channel,
    index: u8,
    mailbox: Mailbox,
    dev: Option<Device>,
    snap: Option<DeviceSnapshot>,
    connection: ConnectionInfo,

    controls: Vec<reprog_controls::ControlInfo>,
    applied: HashMap<u16, Diversion>,
    thumb_info: Option<thumbwheel::ThumbwheelInfo>,
    /// Thumb wheel reporting as last written: (diverted, inverted).
    thumb_applied: Option<(bool, bool)>,
    bindings: Bindings,
    active_profile: Option<String>,
    bindings_dirty: bool,
    profile_dpi: Option<u16>,

    pressed: Vec<u16>,
    held: HashMap<u16, Action>,
    gesture: Option<(u16, GestureTracker)>,
    thumb: Option<ThumbwheelTracker>,
    shift_saved_dpi: Option<u16>,
    skip_first_motion: bool,
    battery_warned: bool,
    app: ActiveApp,
}

impl Actor {
    fn new(shared: Arc<Shared>, channel: Channel, index: u8, connection: ConnectionInfo, mailbox: Mailbox) -> Actor {
        let app = shared.active_app.borrow().clone();
        Actor {
            shared,
            channel,
            index,
            mailbox,
            dev: None,
            snap: None,
            connection,
            controls: Vec::new(),
            applied: HashMap::new(),
            thumb_info: None,
            thumb_applied: None,
            bindings: Bindings::default(),
            active_profile: None,
            bindings_dirty: false,
            profile_dpi: None,
            pressed: Vec::new(),
            held: HashMap::new(),
            gesture: None,
            thumb: None,
            shift_saved_dpi: None,
            skip_first_motion: false,
            battery_warned: false,
            app,
        }
    }

    fn id(&self) -> String {
        self.snap.as_ref().map(|s| s.id.clone()).unwrap_or_default()
    }

    fn label(&self) -> String {
        self.snap.as_ref().map(|s| s.name.clone()).unwrap_or_else(|| format!("device {:#04x}", self.index))
    }

    async fn run(mut self, mut rx: mpsc::UnboundedReceiver<DeviceMsg>) {
        // Initialize, waiting for the device to come online if needed.
        let mut online = true;
        loop {
            if online {
                match self.init().await {
                    Ok(()) => break,
                    Err(e) if e.is_offline() => {
                        debug!("device {:#04x} on {} is offline: {e}", self.index, self.connection.node);
                        online = false;
                    }
                    Err(e) => {
                        debug!("ignoring device {:#04x} on {}: {e}", self.index, self.connection.node);
                        return;
                    }
                }
            }
            match rx.recv().await {
                Some(DeviceMsg::Online(true)) => online = true,
                Some(DeviceMsg::Online(false)) | Some(DeviceMsg::Report(_)) => {}
                Some(DeviceMsg::Command(_, reply)) => {
                    let _ = reply.send(Err("device is offline".into()));
                }
                Some(DeviceMsg::Stop(ack)) => {
                    let _ = ack.send(());
                    return;
                }
                Some(DeviceMsg::Shutdown) | None => return,
            }
        }

        let mut app_rx = self.shared.active_app.clone();
        app_rx.mark_unchanged();
        let mut battery_timer = tokio::time::interval(Duration::from_secs(15 * 60));
        battery_timer.tick().await;
        loop {
            tokio::select! {
                msg = rx.recv() => match msg {
                    None | Some(DeviceMsg::Shutdown) => break,
                    Some(DeviceMsg::Stop(ack)) => {
                        self.release_everything();
                        self.restore().await;
                        let _ = ack.send(());
                        break;
                    }
                    Some(DeviceMsg::Report(report)) => self.on_report(report).await,
                    Some(DeviceMsg::Online(online)) => self.on_online(online).await,
                    Some(DeviceMsg::Command(cmd, reply)) => {
                        let result = self.on_command(cmd).await;
                        let _ = reply.send(result);
                    }
                },
                changed = app_rx.changed() => {
                    if changed.is_ok() {
                        let app = app_rx.borrow_and_update().clone();
                        self.on_app(app).await;
                    }
                }
                _ = battery_timer.tick() => self.refresh_battery().await,
            }
        }
        self.release_everything();
        if let Some(snap) = &self.snap {
            self.shared.registry.detach(&snap.id, &self.mailbox);
            self.shared.emit(DaemonEvent::DeviceChanged(snap.id.clone()));
            info!("{} disconnected", snap.name);
        }
    }

    // ---------------------------------------------------------------- init

    async fn init(&mut self) -> hidpp::Result<()> {
        let dev = Device::open(self.channel.clone(), self.index).await?;
        let name = device_info::name(&dev).await.unwrap_or_else(|_| self.channel.info().name.clone());
        let kind = device_info::kind(&dev).await.unwrap_or(device_info::DeviceKind::Unknown);
        let identity = if dev.has(fid::DEVICE_FW_VERSION) { device_info::identity(&dev).await.ok() } else { None };
        let serial = match &identity {
            Some(i) if i.serial_supported => device_info::serial_number(&dev).await.ok().filter(|s| !s.is_empty()),
            _ => None,
        };
        let node = self.channel.info();
        let model_pid = identity.as_ref().and_then(|i| i.model_ids.primary()).unwrap_or(node.product_id);
        let unit_id = identity.as_ref().map(|i| i.unit_id.clone()).unwrap_or_default();
        let id = if !unit_id.is_empty() && unit_id != "00000000" {
            format!("{model_pid:04x}-{unit_id}")
        } else if let Some(s) = &serial {
            format!("{model_pid:04x}-{s}")
        } else {
            format!("{model_pid:04x}-{}", node.uniq.replace(':', ""))
        };
        let firmware = identity
            .as_ref()
            .map(|i| i.firmware.iter().filter(|f| f.kind <= 2 && !f.name.is_empty()).cloned().collect())
            .unwrap_or_default();
        let model_ids = identity
            .as_ref()
            .map(|i| i.model_ids.all().iter().map(|p| format!("{p:04x}")).collect())
            .unwrap_or_else(|| vec![format!("{model_pid:04x}")]);
        let features = dev
            .features()
            .iter()
            .map(|f| FeatureInfo { id: f.id, name: features::feature_name(f.id), version: f.version, hidden: f.is_hidden() })
            .collect();

        self.skip_first_motion = dev.feature_version(fid::REPROG_CONTROLS_V4).is_some_and(|v| v >= 5);
        self.controls = if dev.has(fid::REPROG_CONTROLS_V4) { reprog_controls::controls(&dev).await? } else { Vec::new() };
        self.applied = Self::read_diversions(&dev, &self.controls).await;
        let controls = self
            .controls
            .iter()
            .filter(|c| c.is_divertable())
            .map(|c| ControlInfo {
                cid: c.cid,
                key: controls::control_key(c.cid),
                name: controls::control_name(c.cid),
                role: Role::of(c.cid),
                divertable: c.is_divertable(),
                gestures: c.supports_raw_xy(),
                is_virtual: c.is_virtual(),
                mouse_button: c.flags.contains(reprog_controls::ControlFlags::MOUSE_BUTTON),
            })
            .collect::<Vec<_>>();

        let mut caps = Capabilities {
            gestures: controls.iter().any(|c| c.gestures && !c.is_virtual),
            battery: battery::battery_feature(&dev).is_some(),
            onboard_profiles: dev.has(fid::ONBOARD_PROFILES),
            ..Default::default()
        };
        let mut state = HardwareState::default();

        if dpi::dpi_feature(&dev).is_some() {
            match dpi::read(&dev).await {
                Ok(d) => {
                    caps.dpi = Some(DpiCaps { min: d.range.min(), max: d.range.max(), default: d.default, range: d.range });
                    state.dpi = Some(d.current);
                }
                Err(e) => warn!("{name}: cannot read DPI: {e}"),
            }
        }
        if smartshift::smartshift_feature(&dev).is_some() {
            match smartshift::read(&dev).await {
                Ok(s) => {
                    caps.smartshift = Some(SmartShiftCaps {
                        torque: s.torque.is_some(),
                        default_threshold: s.default_auto_disengage,
                        default_torque: s.default_torque,
                    });
                    state.wheel_mode = Some(s.mode);
                    state.smartshift_threshold = Some(s.auto_disengage);
                    state.smartshift_torque = s.torque;
                }
                Err(e) => warn!("{name}: cannot read SmartShift: {e}"),
            }
        }
        if dev.has(fid::HIRES_WHEEL) {
            if let (Ok(c), Ok(m)) = (hires_wheel::capability(&dev).await, hires_wheel::mode(&dev).await) {
                caps.wheel = Some(WheelCaps { invert: c.has_invert, ratchet_switch: c.has_ratchet_switch, multiplier: c.multiplier });
                state.hires_scroll = Some(m.hires);
                state.invert_scroll = Some(m.inverted);
                if c.has_ratchet_switch {
                    state.ratchet_engaged = hires_wheel::ratchet_engaged(&dev).await.ok();
                }
            }
        }
        if dev.has(fid::THUMB_WHEEL) {
            if let (Ok(info), Ok(status)) = (thumbwheel::info(&dev).await, thumbwheel::status(&dev).await) {
                caps.thumbwheel = Some(ThumbwheelCaps { touch: info.has_touch, tap: info.has_single_tap, proximity: info.has_proximity });
                state.thumbwheel_invert = Some(status.inverted);
                self.thumb_applied = Some((status.diverted, status.inverted));
                self.thumb_info = Some(info);
            }
        }
        if dev.has(fid::CHANGE_HOST) {
            if let Ok(h) = change_host::info(&dev).await {
                caps.hosts = Some(h.count);
                state.current_host = Some(h.current);
            }
        }
        if dev.has(fid::HOSTS_INFO) {
            if let Ok(hosts) = hosts_info::read(&dev).await {
                caps.host_names = hosts.can_write_names;
                state.hosts = hosts
                    .hosts
                    .iter()
                    .map(|h| HostEntry { index: h.index, paired: h.paired, bus: h.bus, name: h.name.clone(), current: h.index == hosts.current })
                    .collect();
            }
        }
        if dev.has(fid::HAPTIC) {
            if let Ok(h) = haptic::read(&dev).await {
                let waveforms = haptic::WAVEFORMS
                    .iter()
                    .filter(|(id, _)| h.waveforms.contains(id))
                    .map(|(id, n)| (*id, n.to_string()))
                    .collect();
                caps.haptics = Some(HapticCaps { waveforms, discrete_levels: h.discrete_levels });
                state.haptics = Some(h.enabled);
                state.haptics_level = Some(h.level);
            }
        }
        if report_rate::report_rate_feature(&dev).is_some() {
            if let Ok(r) = report_rate::read(&dev).await {
                caps.report_rates_us = r.supported_us;
                state.report_rate_us = Some(r.current_us);
            }
        }
        if caps.onboard_profiles {
            if let Ok(m) = onboard_profiles::mode(&dev).await {
                state.host_mode = Some(m == onboard_profiles::Mode::Host);
            }
        }
        let battery = battery::read(&dev).await.ok();
        let protocol = format!("{}.{}", dev.protocol().0, dev.protocol().1);

        self.dev = Some(dev);
        let first_time = !self.shared.config.has_device(&id);
        let gesture_control = self.controls.iter().find(|c| Role::of(c.cid) == Role::Gesture && c.supports_raw_xy()).map(|c| c.cid);
        let config = self.shared.config.update_device(&id, |c| {
            if c.name != name {
                c.name = name.clone();
            }
            if first_time {
                // Out of the box, the gesture button does gestures (it does nothing natively).
                if let Some(cid) = gesture_control {
                    c.bindings.buttons.insert(controls::control_key(cid), Action::Gestures(Default::default()));
                }
            }
            c.clone()
        });
        self.snap = Some(DeviceSnapshot {
            id: id.clone(),
            name: name.clone(),
            kind,
            model_id: format!("{model_pid:04x}"),
            model_ids,
            unit_id,
            serial,
            firmware,
            protocol,
            connection: self.connection.clone(),
            online: true,
            busy: true,
            battery,
            capabilities: caps,
            state,
            controls,
            features,
            config: config.clone(),
            active_profile: None,
            error: None,
        });
        let is_new = self.shared.registry.publish(self.snap.clone().unwrap(), Some(self.mailbox.clone()));
        self.shared.emit(if is_new { DaemonEvent::DevicesChanged } else { DaemonEvent::DeviceChanged(id.clone()) });
        info!("{name} ({id}) connected via {}", self.connection.receiver.as_deref().unwrap_or(match self.connection.bus {
            hidpp::hidraw::Bus::Bluetooth => "Bluetooth",
            _ => "USB",
        }));

        self.apply_configuration(&config).await;
        self.check_battery_warning().await;
        if let Some(s) = self.snap.as_mut() {
            s.busy = false;
        }
        self.publish();
        Ok(())
    }

    /// Applies all hardware settings and bindings (on connect and reconnect).
    async fn apply_configuration(&mut self, config: &DeviceConfig) {
        self.apply_settings(&config.settings, None).await;
        self.resolve_profile(true).await;
    }

    /// Reads how each divertable control currently reports, so only real
    /// differences are written (and leftovers from a crashed run are undone).
    async fn read_diversions(dev: &Device, controls: &[reprog_controls::ControlInfo]) -> HashMap<u16, Diversion> {
        let mut map = HashMap::new();
        for c in controls.iter().filter(|c| c.is_divertable()) {
            if let Ok(r) = reprog_controls::reporting(dev, c.cid).await {
                map.insert(
                    c.cid,
                    Diversion {
                        divert: r.flags.contains(reprog_controls::ReportingFlags::DIVERTED),
                        raw_xy: r.flags.contains(reprog_controls::ReportingFlags::RAW_XY_DIVERTED),
                    },
                );
            }
        }
        map
    }

    async fn on_online(&mut self, online: bool) {
        if let Some(s) = self.snap.as_mut() {
            if s.online == online && !online {
                return;
            }
        }
        if online {
            debug!("{} back online, re-applying configuration", self.label());
            // The device may have lost volatile settings; start from scratch.
            match self.init().await {
                Ok(()) => {}
                Err(e) => {
                    warn!("{}: re-initialization failed: {e}", self.label());
                    self.set_online(false);
                }
            }
        } else {
            self.release_everything();
            self.set_online(false);
        }
    }

    fn set_online(&mut self, online: bool) {
        if let Some(s) = self.snap.as_mut() {
            s.online = online;
            s.busy = false;
        }
        self.publish();
    }

    fn publish(&mut self) {
        let Some(snap) = self.snap.as_mut() else { return };
        snap.config = self.shared.config.device(&snap.id);
        snap.active_profile = self.active_profile.clone();
        self.shared.registry.publish(snap.clone(), Some(self.mailbox.clone()));
        self.shared.emit(DaemonEvent::DeviceChanged(snap.id.clone()));
    }

    fn release_everything(&mut self) {
        let held: Vec<(u16, Action)> = self.held.drain().collect();
        for (_, action) in held {
            let _ = self.shared.executor.release(&action);
        }
        self.pressed.clear();
        self.gesture = None;
        if let Some(input) = self.shared.executor.input() {
            input.reset_scroll();
        }
    }

    // ---------------------------------------------------------------- settings

    fn settings(&self) -> DeviceSettings {
        self.shared.config.device(&self.id()).settings
    }

    /// Writes settings to the device. With `only`, just the listed keys.
    async fn apply_settings(&mut self, s: &DeviceSettings, only: Option<&[String]>) -> Vec<String> {
        let wants = |key: &str| only.is_none_or(|keys| keys.iter().any(|k| k == key));
        let mut errors = Vec::new();
        let Some(dev) = self.dev.clone() else { return errors };
        let caps = self.snap.as_ref().map(|s| s.capabilities.clone()).unwrap_or_default();

        if wants("dpi") {
            if let (Some(value), Some(c)) = (s.dpi, &caps.dpi) {
                let value = c.range.snap(value);
                match dpi::set(&dev, value).await {
                    Ok(()) => self.state_mut(|st| st.dpi = Some(value)),
                    Err(e) => errors.push(format!("DPI: {e}")),
                }
            }
        }
        if (wants("wheel_mode") || wants("smartshift") || wants("smartshift_threshold") || wants("smartshift_torque"))
            && caps.smartshift.is_some()
            && (s.wheel_mode.is_some() || s.smartshift.is_some() || s.smartshift_threshold.is_some() || s.smartshift_torque.is_some())
        {
            let mode = s.wheel_mode.map(|m| match m {
                WheelModeSetting::Ratchet => smartshift::WheelMode::Ratchet,
                WheelModeSetting::FreeSpin => smartshift::WheelMode::FreeSpin,
            });
            let disengage = match (s.smartshift, s.smartshift_threshold) {
                (Some(false), _) => Some(smartshift::ALWAYS_RATCHET),
                (_, Some(t)) => Some(t.clamp(1, SMARTSHIFT_MAX_THRESHOLD)),
                (Some(true), None) => {
                    let current = self.snap.as_ref().and_then(|s| s.state.smartshift_threshold);
                    let default = caps.smartshift.as_ref().map(|c| c.default_threshold).unwrap_or(10);
                    Some(current.filter(|t| *t != smartshift::ALWAYS_RATCHET && *t != 0).unwrap_or(default.clamp(1, SMARTSHIFT_MAX_THRESHOLD)))
                }
                (None, None) => None,
            };
            let torque = s.smartshift_torque.filter(|_| caps.smartshift.as_ref().is_some_and(|c| c.torque)).map(|t| t.clamp(1, 100));
            match smartshift::write(&dev, mode, disengage, torque).await {
                Ok(()) => {
                    if let Ok(now) = smartshift::read(&dev).await {
                        self.state_mut(|st| {
                            st.wheel_mode = Some(now.mode);
                            st.smartshift_threshold = Some(now.auto_disengage);
                            st.smartshift_torque = now.torque;
                        });
                    }
                }
                Err(e) => errors.push(format!("SmartShift: {e}")),
            }
        }
        if (wants("hires_scroll") || wants("invert_scroll")) && caps.wheel.is_some() && (s.hires_scroll.is_some() || s.invert_scroll.is_some()) {
            let current = self.snap.as_ref().map(|s| s.state.clone()).unwrap_or_default();
            let mode = hires_wheel::WheelMode {
                diverted: false,
                hires: s.hires_scroll.or(current.hires_scroll).unwrap_or(true),
                inverted: s.invert_scroll.or(current.invert_scroll).unwrap_or(false),
            };
            match hires_wheel::set_mode(&dev, mode).await {
                Ok(m) => self.state_mut(|st| {
                    st.hires_scroll = Some(m.hires);
                    st.invert_scroll = Some(m.inverted);
                }),
                Err(e) => errors.push(format!("Scroll wheel: {e}")),
            }
        }
        if (wants("haptics") || wants("haptics_level")) && caps.haptics.is_some() && (s.haptics.is_some() || s.haptics_level.is_some()) {
            let current = self.snap.as_ref().map(|s| s.state.clone()).unwrap_or_default();
            let enabled = s.haptics.or(current.haptics).unwrap_or(true);
            let level = s.haptics_level.or(current.haptics_level).unwrap_or(50);
            match haptic::set(&dev, enabled, level).await {
                Ok(()) => self.state_mut(|st| {
                    st.haptics = Some(enabled);
                    st.haptics_level = Some(level);
                }),
                Err(e) => errors.push(format!("Haptics: {e}")),
            }
        }
        if wants("host_mode") && caps.onboard_profiles {
            if let Some(host) = s.host_mode {
                let mode = if host { onboard_profiles::Mode::Host } else { onboard_profiles::Mode::Onboard };
                match onboard_profiles::set_mode(&dev, mode).await {
                    Ok(()) => self.state_mut(|st| st.host_mode = Some(host)),
                    Err(e) => errors.push(format!("Onboard memory: {e}")),
                }
            }
        }
        if wants("report_rate_us") && !caps.report_rates_us.is_empty() {
            if let Some(us) = s.report_rate_us.filter(|us| caps.report_rates_us.contains(us)) {
                match report_rate::set(&dev, us).await {
                    Ok(()) => self.state_mut(|st| st.report_rate_us = Some(us)),
                    Err(e) => errors.push(format!("Report rate: {e}")),
                }
            }
        }
        if !errors.is_empty() {
            warn!("{}: {}", self.label(), errors.join("; "));
        }
        errors
    }

    fn state_mut(&mut self, f: impl FnOnce(&mut HardwareState)) {
        if let Some(s) = self.snap.as_mut() {
            f(&mut s.state);
        }
    }

    /// Validates and clamps user supplied settings against the device's capabilities.
    fn sanitize(&self, s: &mut DeviceSettings) {
        let Some(snap) = &self.snap else { return };
        let caps = &snap.capabilities;
        if let (Some(v), Some(c)) = (s.dpi, &caps.dpi) {
            s.dpi = Some(c.range.snap(v));
        }
        if let Some(c) = &caps.dpi {
            s.dpi_presets = s.dpi_presets.iter().map(|v| c.range.snap(*v)).collect();
            s.dpi_presets.sort_unstable();
            s.dpi_presets.dedup();
        }
        s.dpi_presets.truncate(8);
        s.smartshift_threshold = s.smartshift_threshold.map(|t| t.clamp(1, SMARTSHIFT_MAX_THRESHOLD));
        s.smartshift_torque = s.smartshift_torque.map(|t| t.clamp(1, 100));
        s.haptics_level = s.haptics_level.map(|l| l.min(100));
    }

    // ---------------------------------------------------------------- bindings

    /// Picks the application profile for the focused app and applies the resulting bindings.
    async fn resolve_profile(&mut self, force: bool) {
        let config = self.shared.config.device(&self.id());
        let general = self.shared.config.general();
        let profile: Option<&AppProfile> =
            if general.app_profiles { config.profiles.iter().find(|p| p.matches(&self.app)) } else { None };
        let profile_id = profile.map(|p| p.id.clone());
        let bindings = match profile {
            Some(p) => config.bindings.overlay(&p.bindings),
            None => config.bindings.clone(),
        };
        let profile_dpi = profile.and_then(|p| p.dpi);
        let changed = force || profile_id != self.active_profile || bindings != self.bindings;
        if !changed {
            return;
        }
        if profile_id != self.active_profile {
            debug!("{}: profile {:?} for {:?}", self.label(), profile_id, self.app.app_id);
        }
        self.active_profile = profile_id;
        self.bindings = bindings;
        if !self.pressed.is_empty() {
            // Changing diversion while a control is held would lose its release.
            self.bindings_dirty = true;
            return;
        }
        self.apply_bindings().await;
        // Per profile DPI.
        if profile_dpi != self.profile_dpi {
            let target = profile_dpi.or(config.settings.dpi);
            self.profile_dpi = profile_dpi;
            if let Some(v) = target {
                self.write_dpi(v).await;
            }
        }
    }

    fn binding_for(&self, cid: u16) -> Action {
        self.bindings.buttons.get(&controls::control_key(cid)).cloned().unwrap_or_default()
    }

    async fn apply_bindings(&mut self) {
        self.bindings_dirty = false;
        let Some(dev) = self.dev.clone() else { return };
        let controls = self.controls.clone();
        for c in controls.iter().filter(|c| c.is_divertable()) {
            let action = self.binding_for(c.cid);
            let want = Diversion { divert: action.needs_divert(), raw_xy: action.needs_raw_xy() && c.supports_raw_xy() };
            let current = self.applied.get(&c.cid).copied();
            if current == Some(want) {
                continue;
            }
            if current.is_none() && !want.divert {
                // Unknown state and nothing wanted: leave the control alone.
                continue;
            }
            let change = reprog_controls::ReportingChange {
                divert: Some(want.divert),
                raw_xy: c.supports_raw_xy().then_some(want.raw_xy),
                ..Default::default()
            };
            match reprog_controls::set_reporting(&dev, c.cid, change).await {
                Ok(()) => {
                    self.applied.insert(c.cid, want);
                }
                Err(e) => warn!("{}: cannot divert {}: {e}", self.label(), controls::control_name(c.cid)),
            }
        }
        self.apply_thumbwheel().await;
        self.publish();
    }

    fn effective_thumbwheel(&self) -> ThumbwheelBinding {
        let binding = self.bindings.thumbwheel.clone().unwrap_or_default();
        let tap = self.bindings.thumbwheel_tap.as_ref().is_some_and(|a| a.needs_divert());
        match binding {
            // Diverted only for taps: keep scrolling natively in software.
            ThumbwheelBinding::Default if tap => ThumbwheelBinding::Scroll { speed: 1.0 },
            other => other,
        }
    }

    async fn apply_thumbwheel(&mut self) {
        let Some(info) = self.thumb_info else { return };
        let Some(dev) = self.dev.clone() else { return };
        let binding = self.effective_thumbwheel();
        let divert = binding != ThumbwheelBinding::Default;
        let invert_setting = self.settings().thumbwheel_invert;
        let current_invert = self.snap.as_ref().and_then(|s| s.state.thumbwheel_invert).unwrap_or(false);
        let invert = invert_setting.unwrap_or(current_invert);
        // While diverted, inversion is done in software so it applies to actions too.
        let wanted = (divert, if divert { false } else { invert });
        if self.thumb_applied != Some(wanted) {
            match thumbwheel::set_reporting(&dev, wanted.0, wanted.1).await {
                Ok(()) => self.thumb_applied = Some(wanted),
                Err(e) => warn!("{}: cannot configure the thumb wheel: {e}", self.label()),
            }
        }
        self.state_mut(|st| st.thumbwheel_invert = Some(invert));
        self.thumb = divert.then(|| ThumbwheelTracker::new(binding, &info).inverted(invert));
    }

    async fn on_app(&mut self, app: ActiveApp) {
        self.app = app;
        if self.dev.is_some() && self.snap.as_ref().is_some_and(|s| s.online) {
            self.resolve_profile(false).await;
        }
    }

    // ---------------------------------------------------------------- events

    async fn on_report(&mut self, report: Report) {
        let Some(dev) = &self.dev else { return };
        if !report.is_feature_event() {
            return;
        }
        let Some(feature) = dev.feature_at(report.feature_index()) else { return };
        let params = Params::from(&report);
        let function = report.function();
        match feature {
            fid::REPROG_CONTROLS_V4 => match reprog_controls::decode_event(function, &params) {
                Some(reprog_controls::Event::DivertedButtons(cids)) => self.on_buttons(cids).await,
                Some(reprog_controls::Event::RawXy { dx, dy }) => self.on_motion(dx, dy).await,
                _ => {}
            },
            fid::THUMB_WHEEL => {
                if let Some(ev) = thumbwheel::decode_event(function, &params) {
                    self.on_thumbwheel(ev).await;
                }
            }
            fid::HIRES_WHEEL => {
                if let Some(hires_wheel::Event::Ratchet(engaged)) = hires_wheel::decode_event(function, &params) {
                    self.state_mut(|st| {
                        st.ratchet_engaged = Some(engaged);
                        st.wheel_mode = Some(if engaged { smartshift::WheelMode::Ratchet } else { smartshift::WheelMode::FreeSpin });
                    });
                    self.publish();
                }
            }
            fid::UNIFIED_BATTERY | fid::BATTERY_STATUS | fid::BATTERY_VOLTAGE if function == 0 => {
                if let Some(b) = battery::decode_event(feature, &params) {
                    if let Some(s) = self.snap.as_mut() {
                        s.battery = Some(b);
                    }
                    self.check_battery_warning().await;
                    self.publish();
                }
            }
            fid::WIRELESS_DEVICE_STATUS => {
                if let Some(ev) = wireless_status::decode_event(function, &params) {
                    if ev.reconfiguration_needed {
                        info!("{} asks for reconfiguration", self.label());
                        if let Some(dev) = self.dev.clone() {
                            self.applied = Self::read_diversions(&dev, &self.controls).await;
                            if let Ok(st) = thumbwheel::status(&dev).await {
                                self.thumb_applied = Some((st.diverted, st.inverted));
                            }
                        }
                        let config = self.shared.config.device(&self.id());
                        self.apply_configuration(&config).await;
                    }
                }
            }
            _ => {}
        }
    }

    async fn on_buttons(&mut self, now: Vec<u16>) {
        let released: Vec<u16> = self.pressed.iter().copied().filter(|c| !now.contains(c)).collect();
        let pressed: Vec<u16> = now.iter().copied().filter(|c| !self.pressed.contains(c)).collect();
        self.pressed = now;
        for cid in released {
            self.on_release(cid).await;
        }
        for cid in pressed {
            self.on_press(cid).await;
        }
        if self.pressed.is_empty() && self.bindings_dirty {
            self.apply_bindings().await;
        }
    }

    /// What a control does natively, for controls that are diverted without a binding.
    fn native_action(cid: u16) -> Action {
        match Role::of(cid) {
            Role::Middle => Action::Mouse { button: MouseButton::Middle },
            Role::Back => Action::Mouse { button: MouseButton::Back },
            Role::Forward => Action::Mouse { button: MouseButton::Forward },
            Role::Left => Action::Mouse { button: MouseButton::Left },
            Role::Right => Action::Mouse { button: MouseButton::Right },
            Role::ModeShift => Action::Builtin { id: "wheel-mode-toggle".into() },
            Role::ScrollLeft => Action::Builtin { id: "scroll-left".into() },
            Role::ScrollRight => Action::Builtin { id: "scroll-right".into() },
            _ => Action::None,
        }
    }

    async fn on_press(&mut self, cid: u16) {
        let key = controls::control_key(cid);
        self.shared.emit(DaemonEvent::Button { device: self.id(), control: key, pressed: true });
        let mut action = self.binding_for(cid);
        if action == Action::Default {
            action = Self::native_action(cid);
        }
        match &action {
            Action::Gestures(set) => {
                self.gesture = Some((cid, GestureTracker::new(set.clone(), self.skip_first_motion)));
            }
            other => {
                if let Some(req) = self.shared.executor.press(other) {
                    self.device_request(req).await;
                }
            }
        }
        self.held.insert(cid, action);
    }

    async fn on_release(&mut self, cid: u16) {
        let key = controls::control_key(cid);
        self.shared.emit(DaemonEvent::Button { device: self.id(), control: key.clone(), pressed: false });
        let Some(action) = self.held.remove(&cid) else { return };
        match action {
            Action::Gestures(_) => {
                if let Some((gcid, mut tracker)) = self.gesture.take() {
                    if gcid == cid {
                        let events = tracker.release();
                        let set = tracker.set().clone();
                        for ev in events {
                            self.gesture_event(&key, &set, ev).await;
                        }
                        if let Some(input) = self.shared.executor.input() {
                            input.reset_scroll();
                        }
                    } else {
                        self.gesture = Some((gcid, tracker));
                    }
                }
            }
            other => {
                if let Some(req) = self.shared.executor.release(&other) {
                    self.device_request(req).await;
                }
            }
        }
    }

    async fn on_motion(&mut self, dx: i16, dy: i16) {
        if self.shared.monitoring() {
            self.shared.emit(DaemonEvent::Motion { device: self.id(), dx: dx as i32, dy: dy as i32 });
        }
        let dpi = self.snap.as_ref().and_then(|s| s.state.dpi).unwrap_or(1000);
        let Some((cid, tracker)) = self.gesture.as_mut() else { return };
        let events = tracker.motion(dx, dy, dpi);
        if events.is_empty() {
            return;
        }
        let set = tracker.set().clone();
        let key = controls::control_key(*cid);
        for ev in events {
            self.gesture_event(&key, &set, ev).await;
        }
    }

    async fn gesture_event(&mut self, key: &str, set: &crate::actions::GestureSet, ev: GestureEvent) {
        let (name, action) = match ev {
            GestureEvent::Scroll { vertical, horizontal } => {
                if let Some(input) = self.shared.executor.input() {
                    input.scroll(vertical, horizontal);
                }
                return;
            }
            GestureEvent::Click => ("click".to_string(), (*set.click).clone()),
            GestureEvent::Fire(dir) => (format!("{dir:?}").to_lowercase(), set.action(dir).clone()),
        };
        debug!("{}: gesture {name} -> {action:?}", self.label());
        self.shared.emit(DaemonEvent::Gesture { device: self.id(), control: key.to_string(), gesture: name });
        if let Some(req) = self.shared.executor.run(&action) {
            self.device_request(req).await;
        }
    }

    async fn on_thumbwheel(&mut self, ev: thumbwheel::Event) {
        let Some(tracker) = self.thumb.as_mut() else { return };
        let outputs = tracker.event(&ev);
        let binding = tracker.binding().clone();
        for out in outputs {
            match out {
                ThumbOutput::Scroll(h) => {
                    if let Some(input) = self.shared.executor.input() {
                        input.scroll(0, h);
                    }
                }
                ThumbOutput::Step(dir) => {
                    if let ThumbwheelBinding::Actions { left, right, .. } = &binding {
                        let action = if dir == crate::actions::Direction::Left { left } else { right };
                        if let Some(req) = self.shared.executor.run(action) {
                            self.device_request(req).await;
                        }
                    }
                }
                ThumbOutput::Tap => {
                    if let Some(action) = self.bindings.thumbwheel_tap.clone() {
                        self.shared.emit(DaemonEvent::Button { device: self.id(), control: "thumbwheel-tap".into(), pressed: true });
                        self.shared.emit(DaemonEvent::Button { device: self.id(), control: "thumbwheel-tap".into(), pressed: false });
                        if let Some(req) = self.shared.executor.run(&action) {
                            self.device_request(req).await;
                        }
                    }
                }
            }
        }
    }

    // ---------------------------------------------------------------- device actions

    async fn write_dpi(&mut self, value: u16) -> Option<u16> {
        let dev = self.dev.clone()?;
        let caps = self.snap.as_ref()?.capabilities.dpi.clone()?;
        let value = caps.range.snap(value);
        match dpi::set(&dev, value).await {
            Ok(()) => {
                self.state_mut(|st| st.dpi = Some(value));
                self.publish();
                Some(value)
            }
            Err(e) => {
                warn!("{}: cannot set DPI: {e}", self.label());
                None
            }
        }
    }

    /// DPI values cycled through by the DPI actions.
    fn dpi_presets(&self) -> Vec<u16> {
        let Some(caps) = self.snap.as_ref().and_then(|s| s.capabilities.dpi.clone()) else { return Vec::new() };
        let configured = self.settings().dpi_presets;
        let mut presets: Vec<u16> = if configured.len() >= 2 {
            configured
        } else {
            [400u16, 800, 1200, 1600, 2400, 3200, 4000].into_iter().filter(|v| *v >= caps.min && *v <= caps.max).collect()
        };
        presets = presets.into_iter().map(|v| caps.range.snap(v)).collect();
        presets.sort_unstable();
        presets.dedup();
        presets
    }

    async fn set_user_dpi(&mut self, value: u16) {
        if let Some(v) = self.write_dpi(value).await {
            self.shared.config.update_device(&self.id(), |c| c.settings.dpi = Some(v));
            self.publish();
            self.osd("input-mouse", &format!("Pointer speed {v} DPI")).await;
        }
    }

    async fn osd(&self, icon: &str, text: &str) {
        if self.shared.config.general().osd {
            let notifier = self.shared.notifier.clone();
            let (icon, text) = (icon.to_string(), text.to_string());
            tokio::spawn(async move { notifier.osd(&icon, &text).await });
        }
    }

    async fn device_request(&mut self, req: DeviceRequest) {
        match req {
            DeviceRequest::SetDpi(v) => self.set_user_dpi(v).await,
            DeviceRequest::ShiftDpi(Some(v)) => {
                if self.shift_saved_dpi.is_none() {
                    self.shift_saved_dpi = self.snap.as_ref().and_then(|s| s.state.dpi);
                }
                self.write_dpi(v).await;
            }
            DeviceRequest::ShiftDpi(None) => {
                if let Some(v) = self.shift_saved_dpi.take() {
                    self.write_dpi(v).await;
                }
            }
            DeviceRequest::SwitchHost(host) => self.switch_host(host).await,
            DeviceRequest::Op(op) => self.device_op(op).await,
        }
    }

    async fn switch_host(&mut self, host: u8) {
        let Some(dev) = self.dev.clone() else { return };
        let count = self.snap.as_ref().and_then(|s| s.capabilities.hosts).unwrap_or(0);
        if host >= count {
            return;
        }
        info!("{}: switching to host {}", self.label(), host + 1);
        if let Err(e) = change_host::switch_to(&dev, host).await {
            warn!("{}: cannot switch host: {e}", self.label());
        }
    }

    async fn device_op(&mut self, op: DeviceOp) {
        let Some(dev) = self.dev.clone() else { return };
        match op {
            DeviceOp::DpiCycle | DeviceOp::DpiUp | DeviceOp::DpiDown => {
                let presets = self.dpi_presets();
                if presets.is_empty() {
                    return;
                }
                let current = self.snap.as_ref().and_then(|s| s.state.dpi).unwrap_or(presets[0]);
                let next = match op {
                    DeviceOp::DpiCycle => presets.iter().copied().find(|v| *v > current).unwrap_or(presets[0]),
                    DeviceOp::DpiUp => presets.iter().copied().find(|v| *v > current).unwrap_or(*presets.last().unwrap()),
                    _ => presets.iter().rev().copied().find(|v| *v < current).unwrap_or(presets[0]),
                };
                self.set_user_dpi(next).await;
            }
            DeviceOp::WheelModeToggle => {
                if smartshift::smartshift_feature(&dev).is_none() {
                    return;
                }
                let current = self.snap.as_ref().and_then(|s| s.state.wheel_mode).unwrap_or(smartshift::WheelMode::Ratchet);
                let next = match current {
                    smartshift::WheelMode::Ratchet => smartshift::WheelMode::FreeSpin,
                    smartshift::WheelMode::FreeSpin => smartshift::WheelMode::Ratchet,
                };
                if smartshift::write(&dev, Some(next), None, None).await.is_ok() {
                    self.state_mut(|st| st.wheel_mode = Some(next));
                    self.publish();
                    let label = if next == smartshift::WheelMode::FreeSpin { "Free spin" } else { "Ratchet" };
                    self.osd("input-mouse", label).await;
                }
            }
            DeviceOp::SmartShiftToggle => {
                let enabled = self.snap.as_ref().and_then(|s| s.state.smartshift_threshold).is_some_and(|t| t != smartshift::ALWAYS_RATCHET);
                let id = self.id();
                let mut settings = self.shared.config.update_device(&id, |c| {
                    c.settings.smartshift = Some(!enabled);
                    c.settings.clone()
                });
                self.sanitize(&mut settings);
                self.apply_settings(&settings, Some(&["smartshift".to_string()])).await;
                self.publish();
                self.osd("input-mouse", if enabled { "SmartShift off" } else { "SmartShift on" }).await;
            }
            DeviceOp::InvertScrollToggle => {
                let inverted = self.snap.as_ref().and_then(|s| s.state.invert_scroll).unwrap_or(false);
                let id = self.id();
                let settings = self.shared.config.update_device(&id, |c| {
                    c.settings.invert_scroll = Some(!inverted);
                    c.settings.clone()
                });
                self.apply_settings(&settings, Some(&["invert_scroll".to_string()])).await;
                self.publish();
                self.osd("input-mouse", if inverted { "Standard scrolling" } else { "Natural scrolling" }).await;
            }
            DeviceOp::ShowBattery => {
                let text = match self.snap.as_ref().and_then(|s| s.battery) {
                    Some(b) => match b.approximate_percent() {
                        Some(p) if b.is_charging() => format!("{p}% (charging)"),
                        Some(p) => format!("{p}%"),
                        None => "Unknown".into(),
                    },
                    None => "Unknown".into(),
                };
                let notifier = self.shared.notifier.clone();
                let summary = format!("{} battery", self.label());
                tokio::spawn(async move { notifier.notify("battery-info", &summary, &text, "battery", 4000, false).await });
            }
            DeviceOp::NextHost => {
                let (count, current) = self
                    .snap
                    .as_ref()
                    .map(|s| (s.capabilities.hosts.unwrap_or(0), s.state.current_host.unwrap_or(0)))
                    .unwrap_or((0, 0));
                if count > 1 {
                    self.switch_host((current + 1) % count).await;
                }
            }
        }
    }

    // ---------------------------------------------------------------- battery

    async fn refresh_battery(&mut self) {
        let Some(dev) = self.dev.clone() else { return };
        if !self.snap.as_ref().is_some_and(|s| s.online && s.capabilities.battery) {
            return;
        }
        if let Ok(b) = battery::read(&dev).await {
            if let Some(s) = self.snap.as_mut() {
                s.battery = Some(b);
            }
            self.check_battery_warning().await;
            self.publish();
        }
    }

    async fn check_battery_warning(&mut self) {
        let general = self.shared.config.general();
        let Some(b) = self.snap.as_ref().and_then(|s| s.battery) else { return };
        let Some(percent) = b.approximate_percent() else { return };
        if b.is_charging() || percent > general.low_battery_percent.saturating_add(5) {
            self.battery_warned = false;
            return;
        }
        if percent <= general.low_battery_percent && !self.battery_warned && general.notifications {
            self.battery_warned = true;
            let notifier = self.shared.notifier.clone();
            let summary = format!("{} battery low", self.label());
            let body = format!("{percent}% remaining. Charge it soon.");
            tokio::spawn(async move { notifier.notify("battery-low", &summary, &body, "battery-caution", 0, true).await });
        }
    }

    // ---------------------------------------------------------------- commands

    async fn on_command(&mut self, cmd: Command) -> Reply {
        let id = self.id();
        if !self.snap.as_ref().is_some_and(|s| s.online) {
            return Err("device is offline".into());
        }
        match cmd {
            Command::SetSettings { settings } => {
                let current = self.shared.config.device(&id).settings;
                let mut merged = serde_json::to_value(&current).map_err(|e| e.to_string())?;
                let (Some(obj), Some(patch)) = (merged.as_object_mut(), settings.as_object()) else {
                    return Err("settings must be an object".into());
                };
                let keys: Vec<String> = patch.keys().cloned().collect();
                for (k, v) in patch {
                    if !obj.contains_key(k) {
                        return Err(format!("unknown setting {k:?}"));
                    }
                    obj.insert(k.clone(), v.clone());
                }
                let mut new: DeviceSettings = serde_json::from_value(merged).map_err(|e| format!("invalid settings: {e}"))?;
                self.sanitize(&mut new);
                self.shared.config.update_device(&id, |c| c.settings = new.clone());
                let mut keys = keys;
                if keys.iter().any(|k| k.starts_with("smartshift") || k == "wheel_mode") {
                    keys.extend(["wheel_mode", "smartshift", "smartshift_threshold", "smartshift_torque"].map(String::from));
                }
                let errors = self.apply_settings(&new, Some(&keys)).await;
                if keys.iter().any(|k| k == "thumbwheel_invert") {
                    self.apply_thumbwheel().await;
                }
                self.publish();
                if errors.is_empty() { Ok(serde_json::Value::Null) } else { Err(errors.join("; ")) }
            }
            Command::SetBinding { profile, control, action } => {
                if let Some(Action::Gestures(_)) = &action {
                    let ok = self.controls.iter().any(|c| controls::control_key(c.cid) == control && c.supports_raw_xy());
                    if !ok {
                        return Err("this control cannot do gestures".into());
                    }
                }
                self.edit_bindings(&profile, |b| match action {
                    Some(a) => {
                        b.buttons.insert(control.clone(), a);
                    }
                    None => {
                        b.buttons.remove(&control);
                    }
                })?;
                self.resolve_profile(true).await;
                Ok(serde_json::Value::Null)
            }
            Command::SetThumbwheel { profile, binding } => {
                self.edit_bindings(&profile, |b| b.thumbwheel = binding)?;
                self.resolve_profile(true).await;
                Ok(serde_json::Value::Null)
            }
            Command::SetThumbwheelTap { profile, action } => {
                self.edit_bindings(&profile, |b| b.thumbwheel_tap = action)?;
                self.resolve_profile(true).await;
                Ok(serde_json::Value::Null)
            }
            Command::ResetBindings { profile } => {
                self.edit_bindings(&profile, |b| *b = Bindings::default())?;
                self.resolve_profile(true).await;
                Ok(serde_json::Value::Null)
            }
            Command::AddProfile { name, apps } => {
                let pid = format!(
                    "p{:x}",
                    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0)
                );
                let profile = AppProfile { id: pid.clone(), name, apps, ..Default::default() };
                self.shared.config.update_device(&id, |c| c.profiles.push(profile));
                self.resolve_profile(true).await;
                Ok(serde_json::Value::String(pid))
            }
            Command::UpdateProfile { id: pid, name, apps, enabled, dpi } => {
                let found = self.shared.config.update_device(&id, |c| {
                    let Some(p) = c.profiles.iter_mut().find(|p| p.id == pid) else { return false };
                    if let Some(n) = name {
                        p.name = n;
                    }
                    if let Some(a) = apps {
                        p.apps = a;
                    }
                    if let Some(e) = enabled {
                        p.enabled = e;
                    }
                    if let Some(d) = dpi {
                        p.dpi = d;
                    }
                    true
                });
                if !found {
                    return Err(format!("no profile {pid}"));
                }
                self.resolve_profile(true).await;
                Ok(serde_json::Value::Null)
            }
            Command::RemoveProfile { id: pid } => {
                self.shared.config.update_device(&id, |c| c.profiles.retain(|p| p.id != pid));
                self.resolve_profile(true).await;
                Ok(serde_json::Value::Null)
            }
            Command::SwitchHost { host } => {
                self.switch_host(host).await;
                Ok(serde_json::Value::Null)
            }
            Command::SetHostName { host, name } => {
                let dev = self.dev.clone().ok_or("device is offline")?;
                hosts_info::set_host_name(&dev, host, &name).await.map_err(|e| e.to_string())?;
                if let Ok(hosts) = hosts_info::read(&dev).await {
                    self.state_mut(|st| {
                        st.hosts = hosts
                            .hosts
                            .iter()
                            .map(|h| HostEntry { index: h.index, paired: h.paired, bus: h.bus, name: h.name.clone(), current: h.index == hosts.current })
                            .collect();
                    });
                }
                self.publish();
                Ok(serde_json::Value::Null)
            }
            Command::PlayHaptic { waveform } => {
                let dev = self.dev.clone().ok_or("device is offline")?;
                haptic::play(&dev, waveform).await.map_err(|e| e.to_string())?;
                Ok(serde_json::Value::Null)
            }
            Command::RunAction { action } => {
                if let Some(req) = self.shared.executor.run(&action) {
                    self.device_request(req).await;
                }
                Ok(serde_json::Value::Null)
            }
            Command::Reapply => {
                self.resolve_profile(true).await;
                Ok(serde_json::Value::Null)
            }
            Command::Refresh => {
                self.release_everything();
                self.init().await.map_err(|e| e.to_string())?;
                Ok(serde_json::Value::Null)
            }
        }
    }

    fn edit_bindings(&self, profile: &Option<String>, f: impl FnOnce(&mut Bindings)) -> Result<(), String> {
        let id = self.id();
        self.shared.config.update_device(&id, |c| match profile {
            None => {
                f(&mut c.bindings);
                Ok(())
            }
            Some(pid) => match c.profiles.iter_mut().find(|p| &p.id == pid) {
                Some(p) => {
                    f(&mut p.bindings);
                    Ok(())
                }
                None => Err(format!("no profile {pid}")),
            },
        })
    }

    /// Restores native behaviour before the service exits.
    async fn restore(&mut self) {
        let Some(dev) = self.dev.clone() else { return };
        for (cid, d) in self.applied.clone() {
            if d.divert || d.raw_xy {
                let _ = reprog_controls::set_reporting(&dev, cid, reprog_controls::ReportingChange::divert(false, false)).await;
            }
        }
        if let Some((true, _)) = self.thumb_applied {
            let invert = self.settings().thumbwheel_invert.unwrap_or(false);
            let _ = thumbwheel::set_reporting(&dev, false, invert).await;
        }
    }
}
