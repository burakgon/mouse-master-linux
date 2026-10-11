//! Dumps everything Mouse Master can read from the connected Logitech devices.
//!
//! ```sh
//! cargo run -p hidpp --example probe
//! ```

use hidpp::features::{
    self, battery, change_host, device_info, dpi, haptic, hires_wheel, hosts_info, report_rate, reprog_controls,
    smartshift, thumbwheel,
};
use hidpp::receiver::{self, Receiver};
use hidpp::{Channel, DEVICE_INDEX_SELF, Device};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let nodes = hidpp::hidraw::enumerate()?;
    if nodes.is_empty() {
        println!("No Logitech HID++ devices found (check permissions of /dev/hidraw*).");
    }
    for node in nodes {
        println!("== {} {:04x}:{:04x} {:?} \"{}\" reports={:?}", node.devnode.display(), node.vendor_id, node.product_id, node.bus, node.name, node.reports);
        let model = receiver::receiver_model(node.vendor_id, node.product_id);
        let (channel, _events) = Channel::open(node)?;
        if let Some(model) = model {
            let rx = Receiver::new(channel.clone(), model);
            println!("   receiver: {} serial={:?} connected={:?}", model.name, rx.serial().await, rx.connected_devices().await);
            for slot in 1..=model.max_devices {
                match rx.paired_device(slot).await {
                    Ok(p) => {
                        println!("   slot {slot}: {p:?}");
                        dump(&channel, slot).await;
                    }
                    Err(e) => println!("   slot {slot}: {e}"),
                }
            }
        } else {
            dump(&channel, DEVICE_INDEX_SELF).await;
        }
    }
    Ok(())
}

async fn dump(channel: &Channel, index: u8) {
    let dev = match Device::open(channel.clone(), index).await {
        Ok(d) => d,
        Err(e) => {
            println!("   device {index:#04x}: {e}");
            return;
        }
    };
    println!("   protocol {:?}", dev.protocol());
    println!("   name: {:?}  kind: {:?}", device_info::name(&dev).await, device_info::kind(&dev).await);
    if dev.has(features::id::DEVICE_FRIENDLY_NAME) {
        println!("   friendly name: {:?}", device_info::friendly_name(&dev).await);
    }
    match device_info::identity(&dev).await {
        Ok(id) => {
            println!("   unit id {} models {:?} primary {:04x?}", id.unit_id, id.model_ids, id.model_ids.primary());
            for fw in &id.firmware {
                println!("     fw kind={} {} {} active={}", fw.kind, fw.name, fw.version, fw.active);
            }
            if id.serial_supported {
                println!("   serial: {:?}", device_info::serial_number(&dev).await);
            }
        }
        Err(e) => println!("   identity: {e}"),
    }
    println!("   features:");
    for f in dev.features().iter() {
        println!("     [{:2}] {:#06x} v{} {}{}", f.index, f.id, f.version, features::feature_name(f.id), if f.is_hidden() { " (hidden)" } else { "" });
    }
    println!("   battery: {:?}", battery::read(&dev).await);
    if dpi::dpi_feature(&dev).is_some() {
        println!("   dpi: {:?}", dpi::read(&dev).await);
    }
    if smartshift::smartshift_feature(&dev).is_some() {
        println!("   smartshift: {:?}", smartshift::read(&dev).await);
    }
    if dev.has(features::id::HIRES_WHEEL) {
        println!("   wheel: {:?} {:?} ratchet={:?}", hires_wheel::capability(&dev).await, hires_wheel::mode(&dev).await, hires_wheel::ratchet_engaged(&dev).await);
    }
    if dev.has(features::id::THUMB_WHEEL) {
        println!("   thumbwheel: {:?} {:?}", thumbwheel::info(&dev).await, thumbwheel::status(&dev).await);
    }
    if dev.has(features::id::CHANGE_HOST) {
        println!("   hosts: {:?}", change_host::info(&dev).await);
    }
    if dev.has(features::id::HOSTS_INFO) {
        println!("   hosts info: {:?}", hosts_info::read(&dev).await);
    }
    if dev.has(features::id::HAPTIC) {
        println!("   haptic: {:?}", haptic::read(&dev).await);
    }
    if report_rate::report_rate_feature(&dev).is_some() {
        println!("   report rate: {:?}", report_rate::read(&dev).await);
    }
    if dev.has(features::id::REPROG_CONTROLS_V4) {
        match reprog_controls::controls(&dev).await {
            Ok(controls) => {
                for c in controls {
                    let rep = reprog_controls::reporting(&dev, c.cid).await;
                    println!(
                        "     control {:#06x} {:<24} task={:#06x} flags={:?} group={} mask={:#04x} reporting={:?}",
                        c.cid,
                        hidpp::controls::control_name(c.cid),
                        c.task,
                        c.flags,
                        c.group,
                        c.group_mask,
                        rep.map(|r| (r.flags, r.remapped_to))
                    );
                }
            }
            Err(e) => println!("   controls: {e}"),
        }
    }
}
