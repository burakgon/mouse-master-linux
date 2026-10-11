//! hidraw node discovery (udev) and per-node tasks that route HID++ traffic
//! to device actors.

use std::collections::{HashMap, HashSet};
use std::os::fd::AsRawFd;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use hidpp::hidraw::{self, NodeInfo};
use hidpp::receiver::{self, Receiver, ReceiverEvent};
use hidpp::{Channel, DEVICE_INDEX_SELF};
use tokio::sync::mpsc;
use tracing::{debug, info, warn};

use super::snapshot::ConnectionInfo;
use super::{DeviceMsg, Mailbox, Shared, actor};

#[derive(Debug)]
enum NodeEvent {
    Added(NodeInfo),
    Removed(PathBuf),
    Exited(PathBuf),
}

fn interesting(info: &NodeInfo) -> bool {
    info.is_logitech() && info.reports.is_hidpp() && !info.is_dj_child
}

/// Watches udev for hidraw nodes on a dedicated thread (libudev is not thread safe).
fn spawn_udev_monitor(tx: mpsc::UnboundedSender<NodeEvent>) {
    let result = std::thread::Builder::new().name("udev-monitor".into()).spawn(move || {
        let socket = match udev::MonitorBuilder::new().and_then(|b| b.match_subsystem("hidraw")).and_then(|b| b.listen()) {
            Ok(s) => s,
            Err(e) => {
                warn!("cannot monitor udev, hotplug disabled: {e}");
                return;
            }
        };
        let fd = socket.as_raw_fd();
        loop {
            let mut pfd = libc::pollfd { fd, events: libc::POLLIN, revents: 0 };
            let n = unsafe { libc::poll(&mut pfd, 1, -1) };
            if n < 0 {
                if std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
                    continue;
                }
                warn!("udev monitor poll failed");
                return;
            }
            for event in socket.iter() {
                let dev = event.device();
                match event.event_type() {
                    udev::EventType::Add => {
                        if let Some(info) = NodeInfo::from_udev(&dev) {
                            if interesting(&info) && tx.send(NodeEvent::Added(info)).is_err() {
                                return;
                            }
                        }
                    }
                    udev::EventType::Remove => {
                        if let Some(node) = dev.devnode() {
                            if tx.send(NodeEvent::Removed(node.to_path_buf())).is_err() {
                                return;
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    });
    if let Err(e) = result {
        warn!("cannot start udev monitor thread: {e}");
    }
}

/// Discovers nodes and keeps one task running per node.
pub async fn run(shared: Arc<Shared>) {
    let (tx, mut rx) = mpsc::unbounded_channel();
    spawn_udev_monitor(tx.clone());
    match hidraw::enumerate() {
        Ok(nodes) => {
            for info in nodes {
                let _ = tx.send(NodeEvent::Added(info));
            }
        }
        Err(e) => warn!("cannot enumerate hidraw devices: {e}"),
    }
    let mut active: HashSet<PathBuf> = HashSet::new();
    while let Some(event) = rx.recv().await {
        match event {
            NodeEvent::Added(info) => {
                if !active.insert(info.devnode.clone()) {
                    continue;
                }
                let shared = shared.clone();
                let tx = tx.clone();
                tokio::spawn(async move {
                    let path = info.devnode.clone();
                    run_node(shared, info).await;
                    let _ = tx.send(NodeEvent::Exited(path));
                });
            }
            NodeEvent::Removed(path) => debug!("udev: removed {}", path.display()),
            NodeEvent::Exited(path) => {
                active.remove(&path);
            }
        }
    }
}

async fn open_with_retry(info: &NodeInfo) -> Option<(Channel, hidpp::EventReceiver)> {
    for attempt in 0..6 {
        match Channel::open(info.clone()) {
            Ok(c) => return Some(c),
            Err(hidpp::Error::Io(e)) if e.kind() == std::io::ErrorKind::PermissionDenied => {
                if attempt == 5 {
                    warn!(
                        "no permission to open {} — install the Mouse Master udev rules (see README)",
                        info.devnode.display()
                    );
                    return None;
                }
            }
            Err(e) => {
                if attempt == 5 {
                    warn!("cannot open {}: {e}", info.devnode.display());
                    return None;
                }
            }
        }
        tokio::time::sleep(Duration::from_millis(300 * (attempt + 1))).await;
    }
    None
}

async fn run_node(shared: Arc<Shared>, info: NodeInfo) {
    info!("node {} {:04x}:{:04x} {:?} \"{}\"", info.devnode.display(), info.vendor_id, info.product_id, info.bus, info.name);
    let Some((channel, mut events)) = open_with_retry(&info).await else { return };
    match receiver::receiver_model(info.vendor_id, info.product_id) {
        Some(model) => run_receiver(shared, channel, &mut events, model).await,
        None => run_direct(shared, channel, &mut events).await,
    }
    debug!("node {} finished", info.devnode.display());
}

async fn run_direct(shared: Arc<Shared>, channel: Channel, events: &mut hidpp::EventReceiver) {
    let info = channel.info().clone();
    let connection = ConnectionInfo {
        bus: info.bus,
        receiver: None,
        receiver_kind: None,
        slot: None,
        node: info.devnode.to_string_lossy().into_owned(),
    };
    let mailbox = actor::spawn(shared, channel, DEVICE_INDEX_SELF, connection);
    while let Some(report) = events.recv().await {
        if mailbox.send(DeviceMsg::Report(report)).is_err() {
            // The actor gave up (not a HID++ 2.0 device); keep draining until the node goes away.
            while events.recv().await.is_some() {}
            return;
        }
    }
    let _ = mailbox.send(DeviceMsg::Shutdown);
}

async fn run_receiver(
    shared: Arc<Shared>,
    channel: Channel,
    events: &mut hidpp::EventReceiver,
    model: receiver::ReceiverModel,
) {
    let rx = Receiver::new(channel.clone(), model);
    let serial = rx.serial().await.ok();
    info!("{} {}", model.name, serial.as_deref().unwrap_or(""));
    if let Err(e) = rx.enable_notifications().await {
        warn!("{}: cannot enable notifications: {e}", model.name);
    }
    let receiver_name = model.name.to_string();
    let node = channel.info().devnode.to_string_lossy().into_owned();
    let mut slots: HashMap<u8, Mailbox> = HashMap::new();
    let spawn_slot = |slot: u8, slots: &mut HashMap<u8, Mailbox>| {
        let connection = ConnectionInfo {
            bus: hidpp::hidraw::Bus::Usb,
            receiver: Some(receiver_name.clone()),
            receiver_kind: Some(model.kind),
            slot: Some(slot),
            node: node.clone(),
        };
        let mailbox = actor::spawn(shared.clone(), channel.clone(), slot, connection);
        slots.insert(slot, mailbox);
    };
    // Ask for a connection notification for every paired device.
    if let Err(e) = rx.trigger_device_arrival().await {
        warn!("{}: cannot list paired devices: {e}", model.name);
    }
    while let Some(report) = events.recv().await {
        if let Some(event) = receiver::decode_event(&report) {
            match event {
                ReceiverEvent::Connection { slot, online, wireless_pid, kind, .. } => {
                    debug!("receiver: slot {slot} {:04x} {kind:?} online={online}", wireless_pid);
                    if !(1..=6).contains(&slot) {
                        continue;
                    }
                    match slots.get(&slot) {
                        Some(mailbox) if !mailbox.is_closed() => {
                            let _ = mailbox.send(DeviceMsg::Online(online));
                        }
                        _ => {
                            spawn_slot(slot, &mut slots);
                            if !online {
                                if let Some(m) = slots.get(&slot) {
                                    let _ = m.send(DeviceMsg::Online(false));
                                }
                            }
                        }
                    }
                }
                ReceiverEvent::Unpaired { slot } => {
                    if let Some(mailbox) = slots.remove(&slot) {
                        let _ = mailbox.send(DeviceMsg::Shutdown);
                    }
                }
            }
            continue;
        }
        let index = report.device_index();
        if let Some(mailbox) = slots.get(&index) {
            let _ = mailbox.send(DeviceMsg::Report(report));
        }
    }
    for (_, mailbox) in slots {
        let _ = mailbox.send(DeviceMsg::Shutdown);
    }
}

/// Runs a simulated node through the same code paths as real hardware (demo mode, tests).
pub fn spawn_sim(shared: Arc<Shared>, topology: hidpp::sim::Topology) -> std::io::Result<hidpp::sim::SimHandle> {
    let (fd, info, handle) = hidpp::sim::spawn(topology)?;
    let hidraw = hidpp::hidraw::Hidraw::from_fd(fd, &info.devnode)?;
    let model = receiver::receiver_model(info.vendor_id, info.product_id);
    let (channel, mut events) = Channel::with_hidraw(hidraw, info);
    tokio::spawn(async move {
        match model {
            Some(model) => run_receiver(shared, channel, &mut events, model).await,
            None => run_direct(shared, channel, &mut events).await,
        }
    });
    Ok(handle)
}
