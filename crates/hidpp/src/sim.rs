//! Simulated HID++ devices and receivers.
//!
//! A simulation talks over one end of a `SOCK_SEQPACKET` socket pair, exactly
//! like a hidraw node: one HID report per message. The other end is handed to
//! [`Channel::with_hidraw`](crate::Channel::with_hidraw), so the whole stack —
//! channel, device, features, receiver handling — runs unmodified. Used by the
//! test suites and by the service's demo mode.
//!
//! The device models answer the requests Mouse Master makes with the values a
//! real MX Master 3S returned, and keep state (DPI, diversion, wheel modes...)
//! so writes can be read back.

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read, Write};
use std::os::fd::{FromRawFd, OwnedFd};
use std::path::PathBuf;

use tokio::io::unix::AsyncFd;
use tokio::sync::mpsc;

use crate::features::id;
use crate::hidraw::{Bus, NodeInfo, ReportSupport};
use crate::receiver::ReceiverKind;

/// A reprogrammable control of a simulated device.
#[derive(Debug, Clone, Copy)]
pub struct ControlSpec {
    pub cid: u16,
    pub task: u16,
    /// Capability flags (getCidInfo bytes 4 and 8).
    pub flags: u16,
    pub group: u8,
    pub group_mask: u8,
}

/// The static description of a simulated device.
#[derive(Debug, Clone)]
pub struct DeviceSpec {
    pub name: String,
    /// HID++ 2.0 device type (3 = mouse).
    pub kind: u8,
    pub unit_id: u32,
    /// Bluetooth (LE) product id.
    pub model_pid: u16,
    /// Product id used when paired with a receiver.
    pub wireless_pid: u16,
    pub serial: String,
    /// Features in table order (index 1 onwards; the root feature is implicit).
    pub features: Vec<(u16, u8)>,
    pub controls: Vec<ControlSpec>,
    pub dpi_min: u16,
    pub dpi_max: u16,
    pub dpi_step: u16,
    pub dpi_default: u16,
    pub hosts: Vec<String>,
    pub current_host: u8,
    pub battery_percent: u8,
    pub torque: bool,
    /// Start without answering requests, like a device that just reconnected.
    pub start_offline: bool,
}

impl DeviceSpec {
    /// An MX Master 3S, mirroring a real device (firmware RBM 22.01).
    pub fn mx_master_3s() -> DeviceSpec {
        DeviceSpec {
            name: "MX Master 3S".into(),
            kind: 3,
            unit_id: 0x5C8F_7AF8,
            model_pid: 0xB034,
            wireless_pid: 0xB034,
            serial: "2418LZXRDD58".into(),
            features: vec![
                (id::FEATURE_SET, 2),
                (id::DEVICE_FW_VERSION, 4),
                (id::DEVICE_NAME, 0),
                (id::WIRELESS_DEVICE_STATUS, 0),
                (id::CONFIG_CHANGE, 0),
                (0x0021, 1),
                (id::DEVICE_FRIENDLY_NAME, 0),
                (id::UNIFIED_BATTERY, 3),
                (id::REPROG_CONTROLS_V4, 5),
                (id::CHANGE_HOST, 1),
                (id::HOSTS_INFO, 2),
                (0x2250, 1),
                (id::ADJUSTABLE_DPI, 2),
                (id::SMART_SHIFT, 0),
                (id::HIRES_WHEEL, 1),
                (id::THUMB_WHEEL, 0),
                (0x2251, 0),
            ],
            controls: master_controls(),
            dpi_min: 200,
            dpi_max: 8000,
            dpi_step: 50,
            dpi_default: 1000,
            hosts: vec!["work-laptop".into(), "mouse-master-demo".into(), "MacBook Pro".into()],
            current_host: 1,
            battery_percent: 65,
            torque: false,
            start_offline: false,
        }
    }

    /// An MX Master 4 like device: haptic feedback and SmartShift with torque.
    pub fn mx_master_4() -> DeviceSpec {
        let mut spec = DeviceSpec::mx_master_3s();
        spec.name = "MX Master 4".into();
        spec.unit_id = 0x1A2B_3C4D;
        spec.model_pid = 0xB042;
        spec.wireless_pid = 0xB042;
        spec.serial = "2533LZ4DEMO1".into();
        spec.battery_percent = 92;
        spec.torque = true;
        for f in spec.features.iter_mut() {
            if f.0 == id::SMART_SHIFT {
                *f = (id::SMART_SHIFT_ENHANCED, 0);
            }
        }
        spec.features.push((id::HAPTIC, 0));
        spec.controls.push(ControlSpec { cid: 0x01A0, task: 0x0128, flags: 0x0130, group: 0, group_mask: 0 });
        spec
    }

    fn feature_index(&self, feature: u16) -> Option<u8> {
        if feature == id::ROOT {
            return Some(0);
        }
        self.features.iter().position(|f| f.0 == feature).map(|i| i as u8 + 1)
    }

    fn feature_at(&self, index: u8) -> Option<(u16, u8)> {
        if index == 0 { Some((id::ROOT, 0)) } else { self.features.get(index as usize - 1).copied() }
    }
}

fn master_controls() -> Vec<ControlSpec> {
    let c = |cid, task, flags, group, group_mask| ControlSpec { cid, task, flags, group, group_mask };
    vec![
        c(0x0050, 0x0038, 0x0401, 1, 0x01),
        c(0x0051, 0x0039, 0x0401, 1, 0x01),
        c(0x0052, 0x003A, 0x0531, 2, 0x03),
        c(0x0053, 0x003C, 0x0531, 2, 0x03),
        c(0x0056, 0x003E, 0x0531, 2, 0x03),
        c(0x00C3, 0x00A9, 0x0531, 2, 0x03),
        c(0x00C4, 0x009D, 0x0531, 2, 0x03),
        c(0x00D7, 0x00B4, 0x03A0, 3, 0x00),
    ]
}

/// Mutable state of a simulated device.
#[derive(Debug)]
struct DeviceState {
    spec: DeviceSpec,
    online: bool,
    dpi: u16,
    wheel_mode: u8,
    auto_disengage: u8,
    torque: u8,
    hires_mode: u8,
    ratchet: bool,
    thumb_diverted: bool,
    thumb_inverted: bool,
    /// Reporting flags per control: byte 2 of setCidReporting, plus the v4
    /// extension byte shifted left by 8 (bit 0x100 = analytics key events).
    reporting: HashMap<u16, u16>,
    haptic_enabled: bool,
    haptic_level: u8,
    played: Vec<u8>,
    pressed: Vec<u16>,
}

impl DeviceState {
    fn new(spec: DeviceSpec) -> DeviceState {
        DeviceState {
            dpi: spec.dpi_default,
            wheel_mode: 2,
            auto_disengage: 10,
            torque: 50,
            hires_mode: 0x02,
            ratchet: true,
            thumb_diverted: false,
            thumb_inverted: false,
            reporting: HashMap::new(),
            haptic_enabled: true,
            haptic_level: 60,
            played: Vec::new(),
            pressed: Vec::new(),
            online: !spec.start_offline,
            spec,
        }
    }

    /// Forgets everything volatile, like a power cycle.
    fn reset_volatile(&mut self) {
        self.reporting.clear();
        self.thumb_diverted = false;
        self.pressed.clear();
    }

    fn diverted(&self, cid: u16) -> bool {
        self.reporting.get(&cid).is_some_and(|f| f & 0x01 != 0)
    }

    fn raw_xy(&self, cid: u16) -> bool {
        self.reporting.get(&cid).is_some_and(|f| f & 0x10 != 0)
    }

    fn analytics(&self, cid: u16) -> bool {
        self.reporting.get(&cid).is_some_and(|f| f & 0x100 != 0)
    }

    /// Answers a HID++ 2.0 request: Ok(reply parameters) or Err(error code).
    fn handle(&mut self, feature_index: u8, function: u8, p: &[u8]) -> Result<Vec<u8>, u8> {
        let Some((feature, _)) = self.spec.feature_at(feature_index) else { return Err(0x06) };
        let p = |i: usize| p.get(i).copied().unwrap_or(0);
        let spec = &self.spec;
        let reply = match (feature, function) {
            (id::ROOT, 0) => {
                let wanted = u16::from_be_bytes([p(0), p(1)]);
                match spec.feature_index(wanted) {
                    Some(i) => {
                        let version = spec.feature_at(i).map(|f| f.1).unwrap_or(0);
                        vec![i, 0, version]
                    }
                    None => vec![0, 0, 0],
                }
            }
            (id::ROOT, 1) => vec![4, 5, p(2)],
            (id::FEATURE_SET, 0) => vec![spec.features.len() as u8],
            (id::FEATURE_SET, 1) => {
                let (fid, version) = spec.feature_at(p(0)).ok_or(0x02u8)?;
                let b = fid.to_be_bytes();
                vec![b[0], b[1], 0, version]
            }
            (id::DEVICE_FW_VERSION, 0) => {
                let mut v = vec![2];
                v.extend_from_slice(&spec.unit_id.to_be_bytes());
                v.extend_from_slice(&[0x00, 0x02]);
                v.extend_from_slice(&spec.model_pid.to_be_bytes());
                v.extend_from_slice(&[0, 0, 0, 0, 0x01, 0x01]);
                v
            }
            (id::DEVICE_FW_VERSION, 1) => match p(0) {
                0 => vec![0x01, b'B', b'L', b'1', 0x69, 0x01, 0x00, 0x06, 0x00],
                1 => vec![0x00, b'R', b'B', b'M', 0x22, 0x01, 0x00, 0x06, 0x01],
                _ => return Err(0x02),
            },
            (id::DEVICE_FW_VERSION, 2) => spec.serial.as_bytes().to_vec(),
            (id::DEVICE_NAME, 0) => vec![spec.name.len() as u8],
            (id::DEVICE_NAME, 1) => spec.name.as_bytes().iter().skip(p(0) as usize).take(16).copied().collect(),
            (id::DEVICE_NAME, 2) => vec![spec.kind],
            (id::DEVICE_FRIENDLY_NAME, 0) => vec![spec.name.len() as u8, 18, spec.name.len() as u8],
            (id::DEVICE_FRIENDLY_NAME, 1) => {
                let mut v = vec![p(0)];
                v.extend(spec.name.as_bytes().iter().skip(p(0) as usize).take(15));
                v
            }
            (id::UNIFIED_BATTERY, 0) => vec![0x0F, 0x03],
            (id::UNIFIED_BATTERY, 1) => battery_status(spec.battery_percent),
            (id::REPROG_CONTROLS_V4, 0) => vec![spec.controls.len() as u8],
            (id::REPROG_CONTROLS_V4, 1) => {
                let c = spec.controls.get(p(0) as usize).ok_or(0x02u8)?;
                let cid = c.cid.to_be_bytes();
                let task = c.task.to_be_bytes();
                vec![cid[0], cid[1], task[0], task[1], c.flags as u8, 0, c.group, c.group_mask, (c.flags >> 8) as u8]
            }
            (id::REPROG_CONTROLS_V4, 2) => {
                let cid = u16::from_be_bytes([p(0), p(1)]);
                if !spec.controls.iter().any(|c| c.cid == cid) {
                    return Err(0x02);
                }
                let flags = self.reporting.get(&cid).copied().unwrap_or(0);
                vec![p(0), p(1), flags as u8, 0, 0, (flags >> 8) as u8]
            }
            (id::REPROG_CONTROLS_V4, 3) => {
                let cid = u16::from_be_bytes([p(0), p(1)]);
                let Some(control) = spec.controls.iter().find(|c| c.cid == cid).copied() else { return Err(0x02) };
                let bits = p(2) as u16 | (p(5) as u16) << 8;
                let mut flags = self.reporting.get(&cid).copied().unwrap_or(0);
                for (value, valid, capability) in [
                    (0x01u16, 0x02u16, 0x0020u16),
                    (0x04, 0x08, 0x0040),
                    (0x10, 0x20, 0x0100),
                    (0x40, 0x80, 0x0200),
                    (0x100, 0x200, 0x0400),
                ] {
                    if bits & valid != 0 {
                        if bits & value != 0 {
                            if control.flags & capability == 0 {
                                return Err(0x02);
                            }
                            flags |= value;
                        } else {
                            flags &= !value;
                        }
                    }
                }
                self.reporting.insert(cid, flags);
                vec![p(0), p(1), p(2), p(3), p(4), p(5)]
            }
            (id::CHANGE_HOST, 0) => vec![spec.hosts.len() as u8, spec.current_host],
            (id::CHANGE_HOST, 1) => vec![],
            (id::HOSTS_INFO, 0) => vec![0x13, 0x08, spec.hosts.len() as u8, spec.current_host],
            (id::HOSTS_INFO, 1) => {
                let name = spec.hosts.get(p(0) as usize).ok_or(0x02u8)?;
                vec![p(0), 1, 4, 1, name.len() as u8, 24]
            }
            (id::HOSTS_INFO, 3) => {
                let name = spec.hosts.get(p(0) as usize).ok_or(0x02u8)?;
                let mut v = vec![p(0), p(1)];
                v.extend(name.as_bytes().iter().skip(p(1) as usize).take(14));
                v
            }
            (id::HOSTS_INFO, 4) => {
                let host = p(0) as usize;
                if host >= self.spec.hosts.len() {
                    return Err(0x02);
                }
                let offset = p(1) as usize;
                let chunk: Vec<u8> = (2..16).map(&p).take_while(|b| *b != 0).collect();
                let mut name = self.spec.hosts[host].as_bytes().to_vec();
                name.truncate(offset);
                name.extend_from_slice(&chunk);
                self.spec.hosts[host] = String::from_utf8_lossy(&name).into_owned();
                vec![p(0), name.len() as u8]
            }
            (id::ADJUSTABLE_DPI, 0) => vec![1],
            (id::ADJUSTABLE_DPI, 1) => {
                let mut v = vec![0];
                v.extend_from_slice(&spec.dpi_min.to_be_bytes());
                v.extend_from_slice(&(0xE000 | spec.dpi_step).to_be_bytes());
                v.extend_from_slice(&spec.dpi_max.to_be_bytes());
                v.extend_from_slice(&[0, 0]);
                v
            }
            (id::ADJUSTABLE_DPI, 2) => {
                let mut v = vec![0];
                v.extend_from_slice(&self.dpi.to_be_bytes());
                v.extend_from_slice(&spec.dpi_default.to_be_bytes());
                v
            }
            (id::ADJUSTABLE_DPI, 3) => {
                let dpi = u16::from_be_bytes([p(1), p(2)]);
                if dpi < spec.dpi_min || dpi > spec.dpi_max || (dpi - spec.dpi_min) % spec.dpi_step != 0 {
                    return Err(0x03);
                }
                self.dpi = dpi;
                vec![0, p(1), p(2)]
            }
            (id::SMART_SHIFT, 0) => vec![self.wheel_mode, self.auto_disengage, 10],
            (id::SMART_SHIFT, 1) => {
                if p(0) != 0 {
                    self.wheel_mode = p(0);
                }
                if p(1) != 0 {
                    self.auto_disengage = p(1);
                }
                vec![self.wheel_mode, self.auto_disengage, 10]
            }
            (id::SMART_SHIFT_ENHANCED, 0) => vec![spec.torque as u8, 10, 50, 0],
            (id::SMART_SHIFT_ENHANCED, 1) => vec![self.wheel_mode, self.auto_disengage, self.torque],
            (id::SMART_SHIFT_ENHANCED, 2) => {
                if p(0) != 0 {
                    self.wheel_mode = p(0);
                }
                if p(1) != 0 {
                    self.auto_disengage = p(1);
                }
                if p(2) != 0 {
                    self.torque = p(2);
                }
                vec![self.wheel_mode, self.auto_disengage, self.torque]
            }
            (id::HIRES_WHEEL, 0) => vec![15, 0x1C, 0x18, 0x18],
            (id::HIRES_WHEEL, 1) => vec![self.hires_mode],
            (id::HIRES_WHEEL, 2) => {
                self.hires_mode = p(0) & 0x07;
                vec![self.hires_mode]
            }
            (id::HIRES_WHEEL, 3) => vec![self.ratchet as u8],
            (id::THUMB_WHEEL, 0) => vec![0x00, 0x12, 0x00, 0x78, 0x00, 0x0F, 0x03, 0xE8],
            (id::THUMB_WHEEL, 1) => vec![self.thumb_diverted as u8, self.thumb_inverted as u8],
            (id::THUMB_WHEEL, 2) => {
                self.thumb_diverted = p(0) != 0;
                self.thumb_inverted = p(1) != 0;
                vec![p(0), p(1)]
            }
            (id::HAPTIC, 0) => vec![0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xFF],
            (id::HAPTIC, 1) => vec![self.haptic_enabled as u8, self.haptic_level, 0x00],
            (id::HAPTIC, 2) => {
                self.haptic_enabled = p(0) & 0x01 != 0;
                self.haptic_level = p(1).min(100);
                vec![p(0), p(1)]
            }
            (id::HAPTIC, 4) => {
                self.played.push(p(0));
                vec![]
            }
            _ => return Err(0x07),
        };
        Ok(reply)
    }
}

fn battery_status(percent: u8) -> Vec<u8> {
    let level = match percent {
        0..=10 => 0x01,
        11..=30 => 0x02,
        31..=80 => 0x04,
        _ => 0x08,
    };
    vec![percent, level, 0, 0]
}

/// How the simulated hardware is connected.
#[derive(Debug, Clone)]
pub enum Topology {
    /// A device connected over Bluetooth.
    Direct(DeviceSpec),
    /// A receiver with devices paired in slots 1, 2, ...
    Receiver { kind: ReceiverKind, devices: Vec<DeviceSpec> },
}

/// Things a test (or the demo) can make the simulated hardware do.
#[derive(Debug, Clone)]
pub enum SimCommand {
    /// The set of controls held down on the device in `slot` (0xFF for direct devices).
    Buttons {
        slot: u8,
        cids: Vec<u16>,
    },
    /// Pointer motion; reported over HID++ only while a raw XY diverted control is held.
    Motion {
        slot: u8,
        dx: i16,
        dy: i16,
    },
    /// Thumb wheel rotation; reported only while the wheel is diverted.
    Thumbwheel {
        slot: u8,
        rotation: i16,
        status: u8,
        flags: u8,
    },
    /// The ratchet engaged or disengaged (mode shift button).
    Ratchet {
        slot: u8,
        engaged: bool,
    },
    Battery {
        slot: u8,
        percent: u8,
    },
    /// A device paired with a receiver goes on- or offline.
    Online {
        slot: u8,
        online: bool,
    },
    /// The device loses all volatile settings and asks to be reconfigured.
    PowerCycle {
        slot: u8,
    },
}

/// A probe into the simulation state, for assertions.
#[derive(Debug, Clone, Default)]
pub struct SimSnapshot {
    pub dpi: u16,
    pub reporting: HashMap<u16, u16>,
    pub thumb_diverted: bool,
    pub thumb_inverted: bool,
    pub wheel_mode: u8,
    pub auto_disengage: u8,
    pub hires_mode: u8,
    pub haptics_played: Vec<u8>,
}

enum Control {
    Command(SimCommand),
    Snapshot(u8, tokio::sync::oneshot::Sender<Option<SimSnapshot>>),
}

#[derive(Clone)]
pub struct SimHandle {
    tx: mpsc::UnboundedSender<Control>,
}

impl SimHandle {
    pub fn send(&self, command: SimCommand) {
        let _ = self.tx.send(Control::Command(command));
    }

    /// Current state of the device in `slot` (0xFF for direct devices).
    pub async fn snapshot(&self, slot: u8) -> Option<SimSnapshot> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.tx.send(Control::Snapshot(slot, tx)).ok()?;
        rx.await.ok().flatten()
    }

    pub fn press(&self, slot: u8, cids: &[u16]) {
        self.send(SimCommand::Buttons { slot, cids: cids.to_vec() });
    }

    pub fn release_all(&self, slot: u8) {
        self.send(SimCommand::Buttons { slot, cids: Vec::new() });
    }

    pub fn motion(&self, slot: u8, dx: i16, dy: i16) {
        self.send(SimCommand::Motion { slot, dx, dy });
    }
}

struct Sim {
    io: AsyncFd<File>,
    topology: SimTopology,
    notification_flags: [u8; 3],
}

enum SimTopology {
    Direct(Box<DeviceState>),
    Receiver { kind: ReceiverKind, slots: Vec<DeviceState> },
}

fn socketpair() -> io::Result<(OwnedFd, OwnedFd)> {
    let mut fds = [0; 2];
    let r = unsafe {
        libc::socketpair(
            libc::AF_UNIX,
            libc::SOCK_SEQPACKET | libc::SOCK_NONBLOCK | libc::SOCK_CLOEXEC,
            0,
            fds.as_mut_ptr(),
        )
    };
    if r != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { (OwnedFd::from_raw_fd(fds[0]), OwnedFd::from_raw_fd(fds[1])) })
}

/// Starts a simulation. Returns the host side descriptor (for
/// [`Hidraw::from_fd`](crate::hidraw::Hidraw::from_fd)), a matching
/// [`NodeInfo`] and a handle to drive the simulation.
pub fn spawn(topology: Topology) -> io::Result<(OwnedFd, NodeInfo, SimHandle)> {
    let (host, device) = socketpair()?;
    let (info, topology) = match topology {
        Topology::Direct(spec) => {
            let info = NodeInfo {
                devnode: PathBuf::from(format!("/sim/{}", spec.name.to_lowercase().replace(' ', "-"))),
                syspath: PathBuf::from("/sys/devices/virtual/sim"),
                bus: Bus::Bluetooth,
                vendor_id: 0x046D,
                product_id: spec.model_pid,
                name: format!("Logitech {}", spec.name),
                uniq: format!("{:08x}", spec.unit_id),
                reports: ReportSupport { short: false, long: true, very_long: false, dj: false },
                is_dj_child: false,
            };
            (info, SimTopology::Direct(Box::new(DeviceState::new(spec))))
        }
        Topology::Receiver { kind, devices } => {
            let (pid, name) = match kind {
                ReceiverKind::Bolt => (0xC548, "Logitech USB Receiver (Bolt)"),
                ReceiverKind::Unifying => (0xC52B, "Logitech USB Receiver"),
                ReceiverKind::Lightspeed => (0xC539, "Logitech USB Receiver (Lightspeed)"),
                ReceiverKind::Nano => (0xC534, "Logitech USB Receiver (Nano)"),
            };
            let info = NodeInfo {
                devnode: PathBuf::from(format!("/sim/receiver-{pid:04x}")),
                syspath: PathBuf::from("/sys/devices/virtual/sim"),
                bus: Bus::Usb,
                vendor_id: 0x046D,
                product_id: pid,
                name: name.into(),
                uniq: String::new(),
                reports: ReportSupport { short: true, long: true, very_long: false, dj: true },
                is_dj_child: false,
            };
            (info, SimTopology::Receiver { kind, slots: devices.into_iter().map(DeviceState::new).collect() })
        }
    };
    let (tx, rx) = mpsc::unbounded_channel();
    let sim = Sim { io: AsyncFd::new(File::from(device))?, topology, notification_flags: [0; 3] };
    tokio::spawn(sim.run(rx));
    Ok((host, info, SimHandle { tx }))
}

impl Sim {
    async fn run(mut self, mut rx: mpsc::UnboundedReceiver<Control>) {
        let mut buf = [0u8; 64];
        loop {
            tokio::select! {
                ready = self.io.readable() => {
                    let Ok(mut guard) = ready else { return };
                    let n = match guard.try_io(|inner| {
                        let mut file: &File = inner.get_ref();
                        file.read(&mut buf)
                    }) {
                        Ok(Ok(0)) => return, // host side closed
                        Ok(Ok(n)) => n,
                        Ok(Err(_)) => return,
                        Err(_would_block) => continue,
                    };
                    drop(guard);
                    let request = buf[..n].to_vec();
                    for reply in self.handle_request(&request) {
                        if self.write(&reply).await.is_err() {
                            return;
                        }
                    }
                }
                control = rx.recv() => {
                    let Some(control) = control else { return };
                    match control {
                        Control::Command(cmd) => {
                            for report in self.handle_command(cmd) {
                                if self.write(&report).await.is_err() {
                                    return;
                                }
                            }
                        }
                        Control::Snapshot(slot, tx) => {
                            let _ = tx.send(self.device(slot).map(|d| SimSnapshot {
                                dpi: d.dpi,
                                reporting: d.reporting.clone(),
                                thumb_diverted: d.thumb_diverted,
                                thumb_inverted: d.thumb_inverted,
                                wheel_mode: d.wheel_mode,
                                auto_disengage: d.auto_disengage,
                                hires_mode: d.hires_mode,
                                haptics_played: d.played.clone(),
                            }));
                        }
                    }
                }
            }
        }
    }

    async fn write(&self, report: &[u8]) -> io::Result<()> {
        loop {
            let mut guard = self.io.writable().await?;
            match guard.try_io(|inner| {
                let mut file: &File = inner.get_ref();
                file.write(report)
            }) {
                Ok(result) => return result.map(|_| ()),
                Err(_would_block) => continue,
            }
        }
    }

    fn device(&mut self, slot: u8) -> Option<&mut DeviceState> {
        match &mut self.topology {
            SimTopology::Direct(d) => (slot == 0xFF || slot == 0).then_some(d),
            SimTopology::Receiver { slots, .. } => slots.get_mut((slot as usize).checked_sub(1)?),
        }
    }

    fn long(index: u8, sub: u8, address: u8, params: &[u8]) -> Vec<u8> {
        let mut r = vec![0x11, index, sub, address];
        r.extend_from_slice(&params[..params.len().min(16)]);
        r.resize(20, 0);
        r
    }

    fn short(index: u8, sub: u8, address: u8, params: &[u8]) -> Vec<u8> {
        let mut r = vec![0x10, index, sub, address];
        r.extend_from_slice(&params[..params.len().min(3)]);
        r.resize(7, 0);
        r
    }

    fn handle_request(&mut self, request: &[u8]) -> Vec<Vec<u8>> {
        if request.len() < 4 || !matches!(request[0], 0x10 | 0x11) {
            return Vec::new();
        }
        let (index, sub, address) = (request[1], request[2], request[3]);
        let params = &request[4..];
        if let SimTopology::Receiver { .. } = self.topology {
            if index == 0xFF {
                return self.receiver_register(sub, address, params);
            }
        }
        let Some(dev) = self.device(index) else {
            return vec![Self::short(index, 0x8F, sub, &[address, 0x08])];
        };
        if !dev.online {
            // The receiver answers for unreachable devices.
            return vec![Self::short(index, 0x8F, sub, &[address, 0x09])];
        }
        match dev.handle(sub, address >> 4, params) {
            Ok(reply) => vec![Self::long(index, sub, address, &reply)],
            Err(code) => vec![Self::long(index, 0xFF, sub, &[address, code])],
        }
    }

    fn receiver_register(&mut self, sub: u8, address: u8, p: &[u8]) -> Vec<Vec<u8>> {
        let SimTopology::Receiver { kind, slots } = &self.topology else { return Vec::new() };
        let kind = *kind;
        let p0 = p.first().copied().unwrap_or(0);
        match (sub, address) {
            (0x81, 0x00) => vec![Self::short(0xFF, sub, address, &self.notification_flags.clone())],
            (0x80, 0x00) => {
                self.notification_flags = [p0, p.get(1).copied().unwrap_or(0), p.get(2).copied().unwrap_or(0)];
                vec![Self::short(0xFF, sub, address, &[])]
            }
            (0x81, 0x02) => {
                let online = slots.iter().filter(|d| d.online).count() as u8;
                vec![Self::short(0xFF, sub, address, &[0, online, 0])]
            }
            (0x80, 0x02) => {
                let mut out = vec![Self::short(0xFF, sub, address, &[])];
                if p0 == 0x02 {
                    for slot in 1..=slots.len() as u8 {
                        out.push(self.connection_notification(slot));
                    }
                }
                out
            }
            (0x83, 0xB5) => {
                let reply = match (kind, p0) {
                    (ReceiverKind::Bolt, 0x51..=0x56) => {
                        let Some(d) = slots.get((p0 - 0x51) as usize) else {
                            return vec![Self::short(0xFF, 0x8F, sub, &[address, 0x02])];
                        };
                        let w = d.spec.wireless_pid.to_le_bytes();
                        let mut v = vec![p0, 0x02, w[0], w[1]];
                        v.extend_from_slice(&d.spec.unit_id.to_be_bytes());
                        v
                    }
                    (ReceiverKind::Bolt, 0x61..=0x66) => {
                        let Some(d) = slots.get((p0 - 0x61) as usize) else {
                            return vec![Self::short(0xFF, 0x8F, sub, &[address, 0x02])];
                        };
                        let mut v = vec![p0, 0x01, d.spec.name.len().min(13) as u8];
                        v.extend(d.spec.name.as_bytes().iter().take(13));
                        v
                    }
                    (_, 0x03) => vec![0x03, 0x12, 0x34, 0x56, 0x78, 0x00, 0x06],
                    (_, 0x20..=0x25) => {
                        let Some(d) = slots.get((p0 - 0x20) as usize) else {
                            return vec![Self::short(0xFF, 0x8F, sub, &[address, 0x02])];
                        };
                        let w = d.spec.wireless_pid.to_be_bytes();
                        vec![p0, 0x00, 0x08, w[0], w[1], 0x00, 0x00, 0x02]
                    }
                    (_, 0x30..=0x35) => {
                        let Some(d) = slots.get((p0 - 0x30) as usize) else {
                            return vec![Self::short(0xFF, 0x8F, sub, &[address, 0x02])];
                        };
                        let mut v = vec![p0];
                        v.extend_from_slice(&d.spec.unit_id.to_be_bytes());
                        v.extend_from_slice(&[0, 0, 0, 0, 0x01]);
                        v
                    }
                    (_, 0x40..=0x45) => {
                        let Some(d) = slots.get((p0 - 0x40) as usize) else {
                            return vec![Self::short(0xFF, 0x8F, sub, &[address, 0x02])];
                        };
                        let mut v = vec![p0, d.spec.name.len().min(14) as u8];
                        v.extend(d.spec.name.as_bytes().iter().take(14));
                        v
                    }
                    _ => return vec![Self::short(0xFF, 0x8F, sub, &[address, 0x02])],
                };
                vec![Self::long(0xFF, sub, address, &reply)]
            }
            (0x83, 0xFB) if kind == ReceiverKind::Bolt => {
                vec![Self::long(0xFF, sub, address, &[0xB0, 0x17, 0x5E, 0xED])]
            }
            _ => vec![Self::short(0xFF, 0x8F, sub, &[address, 0x02])],
        }
    }

    fn connection_notification(&self, slot: u8) -> Vec<u8> {
        let SimTopology::Receiver { kind, slots } = &self.topology else { return Vec::new() };
        let d = &slots[slot as usize - 1];
        let w = d.spec.wireless_pid.to_le_bytes();
        let protocol = if *kind == ReceiverKind::Bolt { 0x10 } else { 0x04 };
        let flags = 0x02 | if d.online { 0 } else { 0x40 };
        Self::short(slot, 0x41, protocol, &[flags, w[0], w[1]])
    }

    fn event(index: u8, dev: &DeviceState, feature: u16, function: u8, params: &[u8]) -> Option<Vec<u8>> {
        let fi = dev.spec.feature_index(feature)?;
        Some(Self::long(index, fi, function << 4, params))
    }

    fn handle_command(&mut self, cmd: SimCommand) -> Vec<Vec<u8>> {
        let receiver = matches!(self.topology, SimTopology::Receiver { .. });
        let slot = match &cmd {
            SimCommand::Buttons { slot, .. }
            | SimCommand::Motion { slot, .. }
            | SimCommand::Thumbwheel { slot, .. }
            | SimCommand::Ratchet { slot, .. }
            | SimCommand::Battery { slot, .. }
            | SimCommand::Online { slot, .. }
            | SimCommand::PowerCycle { slot } => *slot,
        };
        let index = if receiver { slot } else { 0xFF };
        if let SimCommand::Online { online, .. } = cmd {
            let Some(dev) = self.device(slot) else { return Vec::new() };
            dev.online = online;
            if !online {
                dev.reset_volatile();
            }
            return if receiver { vec![self.connection_notification(slot)] } else { Vec::new() };
        }
        let Some(dev) = self.device(slot) else { return Vec::new() };
        if !dev.online {
            return Vec::new();
        }
        let mut out = Vec::new();
        match cmd {
            SimCommand::Buttons { cids, .. } => {
                let before = std::mem::replace(&mut dev.pressed, cids);
                // Analytics key events for controls that keep their native function.
                let mut analytics = Vec::new();
                for cid in dev.pressed.iter().filter(|c| !before.contains(c)) {
                    if dev.analytics(*cid) && !dev.diverted(*cid) {
                        analytics.extend_from_slice(&cid.to_be_bytes());
                        analytics.push(1);
                    }
                }
                for cid in before.iter().filter(|c| !dev.pressed.contains(c)) {
                    if dev.analytics(*cid) && !dev.diverted(*cid) {
                        analytics.extend_from_slice(&cid.to_be_bytes());
                        analytics.push(0);
                    }
                }
                if !analytics.is_empty() {
                    out.extend(Self::event(index, dev, id::REPROG_CONTROLS_V4, 2, &analytics));
                }
                let held: Vec<u16> = dev.pressed.iter().copied().filter(|c| dev.diverted(*c)).take(4).collect();
                let was: Vec<u16> = before.iter().copied().filter(|c| dev.diverted(*c)).take(4).collect();
                if held != was {
                    let mut params = Vec::new();
                    for cid in &held {
                        params.extend_from_slice(&cid.to_be_bytes());
                    }
                    // Every change of the diverted set is reported, including "none held".
                    out.extend(Self::event(index, dev, id::REPROG_CONTROLS_V4, 0, &params));
                }
            }
            SimCommand::Motion { dx, dy, .. } => {
                if dev.pressed.iter().any(|c| dev.raw_xy(*c)) {
                    let mut params = dx.to_be_bytes().to_vec();
                    params.extend_from_slice(&dy.to_be_bytes());
                    out.extend(Self::event(index, dev, id::REPROG_CONTROLS_V4, 1, &params));
                }
            }
            SimCommand::Thumbwheel { rotation, status, flags, .. } => {
                if dev.thumb_diverted {
                    let mut params = rotation.to_be_bytes().to_vec();
                    params.extend_from_slice(&[0x01, 0x02, status, flags]);
                    out.extend(Self::event(index, dev, id::THUMB_WHEEL, 0, &params));
                }
            }
            SimCommand::Ratchet { engaged, .. } => {
                dev.ratchet = engaged;
                dev.wheel_mode = if engaged { 2 } else { 1 };
                out.extend(Self::event(index, dev, id::HIRES_WHEEL, 1, &[engaged as u8]));
            }
            SimCommand::Battery { percent, .. } => {
                dev.spec.battery_percent = percent;
                out.extend(Self::event(index, dev, id::UNIFIED_BATTERY, 0, &battery_status(percent)));
            }
            SimCommand::PowerCycle { .. } => {
                dev.reset_volatile();
                out.extend(Self::event(index, dev, id::WIRELESS_DEVICE_STATUS, 0, &[1, 1, 1]));
            }
            SimCommand::Online { .. } => {}
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::{device_info, dpi, reprog_controls};
    use crate::hidraw::Hidraw;
    use crate::receiver::{self, Receiver, ReceiverEvent};
    use crate::{Channel, DEVICE_INDEX_SELF, Device};
    use std::time::Duration;

    async fn direct() -> (Channel, crate::EventReceiver, SimHandle) {
        let (fd, info, handle) = spawn(Topology::Direct(DeviceSpec::mx_master_3s())).unwrap();
        let hidraw = Hidraw::from_fd(fd, &info.devnode).unwrap();
        let (channel, events) = Channel::with_hidraw(hidraw, info);
        (channel, events, handle)
    }

    #[tokio::test]
    async fn direct_device_round_trip() {
        let (channel, mut events, sim) = direct().await;
        let dev = Device::open(channel, DEVICE_INDEX_SELF).await.unwrap();
        assert_eq!(dev.protocol(), (4, 5));
        assert_eq!(device_info::name(&dev).await.unwrap(), "MX Master 3S");
        let identity = device_info::identity(&dev).await.unwrap();
        assert_eq!(identity.unit_id, "5C8F7AF8");
        assert_eq!(identity.model_ids.primary(), Some(0xB034));
        let d = dpi::read(&dev).await.unwrap();
        assert_eq!((d.range.min(), d.range.max(), d.current), (200, 8000, 1000));
        dpi::set(&dev, 1600).await.unwrap();
        assert_eq!(dpi::read(&dev).await.unwrap().current, 1600);
        assert!(dpi::set(&dev, 1601).await.is_err());

        // Divert the gesture button with raw XY and press it.
        reprog_controls::set_reporting(&dev, 0x00C3, reprog_controls::ReportingChange::divert(true, true))
            .await
            .unwrap();
        let state = sim.snapshot(0xFF).await.unwrap();
        assert_eq!(state.reporting.get(&0x00C3), Some(&0x11));
        sim.press(0xFF, &[0x00C3]);
        sim.motion(0xFF, 3, -4);
        sim.release_all(0xFF);
        let mut got = Vec::new();
        for _ in 0..3 {
            let r = tokio::time::timeout(Duration::from_secs(1), events.recv()).await.unwrap().unwrap();
            let f = dev.feature_at(r.feature_index()).unwrap();
            got.push(reprog_controls::decode_event(r.function(), &crate::Params::from(&r)).map(|e| (f, e)));
        }
        assert_eq!(got[0], Some((id::REPROG_CONTROLS_V4, reprog_controls::Event::DivertedButtons(vec![0x00C3]))));
        assert_eq!(got[1], Some((id::REPROG_CONTROLS_V4, reprog_controls::Event::RawXy { dx: 3, dy: -4 })));
        assert_eq!(got[2], Some((id::REPROG_CONTROLS_V4, reprog_controls::Event::DivertedButtons(vec![]))));
    }

    #[tokio::test]
    async fn receiver_lists_and_reaches_devices() {
        let devices = vec![DeviceSpec::mx_master_3s(), DeviceSpec::mx_master_4()];
        let (fd, info, sim) = spawn(Topology::Receiver { kind: ReceiverKind::Bolt, devices }).unwrap();
        let model = receiver::receiver_model(info.vendor_id, info.product_id).unwrap();
        let hidraw = Hidraw::from_fd(fd, &info.devnode).unwrap();
        let (channel, mut events) = Channel::with_hidraw(hidraw, info);
        let rx = Receiver::new(channel.clone(), model);
        rx.enable_notifications().await.unwrap();
        assert_eq!(rx.serial().await.unwrap(), "B0175EED");
        let paired = rx.paired_device(2).await.unwrap();
        assert_eq!(paired.wireless_pid, 0xB042);
        assert_eq!(paired.codename.as_deref(), Some("MX Master 4"));
        rx.trigger_device_arrival().await.unwrap();
        for slot in 1..=2 {
            let r = tokio::time::timeout(Duration::from_secs(1), events.recv()).await.unwrap().unwrap();
            assert!(
                matches!(receiver::decode_event(&r), Some(ReceiverEvent::Connection { slot: s, online: true, .. }) if s == slot)
            );
        }
        let dev = Device::open(channel.clone(), 2).await.unwrap();
        assert_eq!(device_info::name(&dev).await.unwrap(), "MX Master 4");

        sim.send(SimCommand::Online { slot: 2, online: false });
        let r = tokio::time::timeout(Duration::from_secs(1), events.recv()).await.unwrap().unwrap();
        assert!(matches!(receiver::decode_event(&r), Some(ReceiverEvent::Connection { slot: 2, online: false, .. })));
        assert!(device_info::name(&dev).await.unwrap_err().is_offline());
    }
}
