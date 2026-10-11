//! `mouse-masterctl`: control the Mouse Master service from a terminal.

use std::io::IsTerminal;

use anyhow::{Context, Result, anyhow, bail};
use clap::{Parser, Subcommand};
use futures::StreamExt;
use serde_json::{Value, json};

const BUS_NAME: &str = "io.github.burakgon.MouseMaster.Daemon";
const OBJECT_PATH: &str = "/io/github/burakgon/MouseMaster/Daemon";
const INTERFACE: &str = "io.github.burakgon.MouseMaster.Daemon1";

#[derive(Parser)]
#[command(name = "mouse-masterctl", version, about = "Control Mouse Master from the command line")]
struct Cli {
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// List devices.
    List,
    /// Show everything about a device.
    Info {
        /// Device id or a part of its name (default: the first device).
        device: Option<String>,
        /// Print raw JSON.
        #[arg(long)]
        json: bool,
    },
    /// Change settings, e.g. `set dpi=1600 smartshift=true smartshift_threshold=12`.
    Set {
        #[arg(short, long)]
        device: Option<String>,
        /// key=value pairs; values are JSON (numbers, true/false, "strings").
        settings: Vec<String>,
    },
    /// Bind a control: `bind gesture '{"type":"gestures"}'`, `bind back copy`, `bind middle default`.
    Bind {
        #[arg(short, long)]
        device: Option<String>,
        /// Application profile id (default profile when omitted).
        #[arg(short, long)]
        profile: Option<String>,
        /// Control key (gesture, middle, back, forward, mode-shift, cid-XXXX).
        control: String,
        /// A built-in action id, `default`, `none`, or an action as JSON.
        action: String,
    },
    /// Run an action once: `run overview`, `run '{"type":"keys","keys":["KEY_LEFTCTRL","KEY_T"]}'`.
    Run {
        #[arg(short, long)]
        device: Option<String>,
        action: String,
    },
    /// List the built-in actions available on this desktop.
    Actions,
    /// Print button presses, gestures and focus changes as they happen.
    Monitor {
        /// Also print raw motion while a gesture button is held.
        #[arg(long)]
        motion: bool,
    },
    /// Print the configuration (TOML-equivalent JSON).
    Config,
    /// Check permissions, the service and conflicting tools.
    Doctor,
    /// Switch a device to another Easy-Switch channel (1-based).
    Host {
        #[arg(short, long)]
        device: Option<String>,
        host: u8,
    },
}

struct Client {
    conn: zbus::Connection,
}

impl Client {
    async fn new() -> Result<Client> {
        let conn = zbus::Connection::session().await.context("cannot connect to the session bus")?;
        Ok(Client { conn })
    }

    async fn call<B>(&self, method: &str, body: &B) -> Result<zbus::Message>
    where
        B: serde::Serialize + zbus::zvariant::DynamicType,
    {
        self.conn.call_method(Some(BUS_NAME), OBJECT_PATH, Some(INTERFACE), method, body).await.map_err(|e| match e {
            zbus::Error::MethodError(name, msg, _) if name.as_str() == "org.freedesktop.DBus.Error.ServiceUnknown" => {
                anyhow!(
                    "the Mouse Master service is not running (start it with `systemctl --user start mouse-master`){}",
                    msg.map(|m| format!(": {m}")).unwrap_or_default()
                )
            }
            zbus::Error::MethodError(_, Some(msg), _) => anyhow!(msg),
            other => anyhow!(other),
        })
    }

    async fn json(&self, method: &str) -> Result<Value> {
        let msg = self.call(method, &()).await?;
        let s: String = msg.body().deserialize()?;
        Ok(serde_json::from_str(&s)?)
    }

    async fn devices(&self) -> Result<Vec<Value>> {
        Ok(self.json("ListDevices").await?.as_array().cloned().unwrap_or_default())
    }

    async fn device(&self, query: Option<&str>) -> Result<Value> {
        let devices = self.devices().await?;
        if devices.is_empty() {
            bail!("no devices found");
        }
        let Some(q) = query else {
            return Ok(devices.iter().find(|d| d["online"] == true).unwrap_or(&devices[0]).clone());
        };
        let q = q.to_lowercase();
        devices
            .iter()
            .find(|d| d["id"].as_str().is_some_and(|id| id.to_lowercase() == q))
            .or_else(|| devices.iter().find(|d| d["name"].as_str().is_some_and(|n| n.to_lowercase().contains(&q))))
            .cloned()
            .ok_or_else(|| anyhow!("no device matches {q:?}"))
    }

    async fn command(&self, id: &str, command: Value) -> Result<Value> {
        let msg = self.call("Command", &(id, command.to_string())).await?;
        let s: String = msg.body().deserialize()?;
        Ok(serde_json::from_str(&s).unwrap_or(Value::Null))
    }
}

fn parse_action(s: &str) -> Result<Value> {
    let s = s.trim();
    if s.starts_with('{') {
        return serde_json::from_str(s).context("invalid action JSON");
    }
    Ok(match s {
        "default" => json!({"type": "default"}),
        "none" => json!({"type": "none"}),
        "gestures" => json!({"type": "gestures"}),
        id => json!({"type": "builtin", "id": id}),
    })
}

fn style(code: &str, text: &str) -> String {
    if std::io::stdout().is_terminal() { format!("\x1b[{code}m{text}\x1b[0m") } else { text.to_string() }
}

fn battery_text(d: &Value) -> String {
    let b = &d["battery"];
    if b.is_null() {
        return "—".into();
    }
    let pct = b["percent"]
        .as_u64()
        .map(|p| format!("{p}%"))
        .unwrap_or_else(|| b["level"].as_str().unwrap_or("?").to_string());
    match b["state"].as_str() {
        Some("charging") | Some("charging-slow") => format!("{pct} ⚡"),
        Some("full") => format!("{pct} (full)"),
        _ => pct,
    }
}

fn key_label(name: &str) -> String {
    let k = name.trim_start_matches("KEY_");
    match k {
        "LEFTCTRL" | "RIGHTCTRL" => "Ctrl".into(),
        "LEFTSHIFT" | "RIGHTSHIFT" => "Shift".into(),
        "LEFTALT" => "Alt".into(),
        "RIGHTALT" => "AltGr".into(),
        "LEFTMETA" | "RIGHTMETA" => "Super".into(),
        k if k.len() == 1 => k.into(),
        k => {
            let mut c = k.chars();
            c.next().map(|f| f.to_string() + &c.as_str().to_lowercase()).unwrap_or_default()
        }
    }
}

/// A short human description of an action, using the catalog for built-in labels.
fn describe(action: &Value, catalog: &Value) -> String {
    let builtin = |id: &str| {
        catalog["actions"]
            .as_array()
            .and_then(|a| a.iter().find(|x| x["id"] == id))
            .and_then(|x| x["label"].as_str())
            .unwrap_or(id)
            .to_string()
    };
    match action["type"].as_str().unwrap_or("default") {
        "default" => "default".into(),
        "none" => "disabled".into(),
        "builtin" => builtin(action["id"].as_str().unwrap_or("")),
        "keys" => action["keys"]
            .as_array()
            .map(|k| k.iter().filter_map(|x| x.as_str()).map(key_label).collect::<Vec<_>>().join(" + "))
            .unwrap_or_default(),
        "mouse" => format!("{} button", action["button"].as_str().unwrap_or("?")),
        "command" => format!("run `{}`", action["command"].as_str().unwrap_or("")),
        "open" => format!("open {}", action["target"].as_str().unwrap_or("")),
        "set-dpi" => format!("{} DPI", action["dpi"]),
        "shift-dpi" => format!("{} DPI while held", action["dpi"]),
        "switch-host" => format!("switch to computer {}", action["host"].as_u64().unwrap_or(0) + 1),
        "macro" => format!("macro ({} steps)", action["steps"].as_array().map_or(0, |s| s.len())),
        "gestures" if action["mode"] == "pan" => "pan & scroll".into(),
        "gestures" => {
            let part = |dir: &str, arrow: &str| format!("{arrow} {}", describe(&action[dir], catalog));
            format!(
                "gestures: {}, {}, {}, {}, click {}",
                part("up", "↑"),
                part("down", "↓"),
                part("left", "←"),
                part("right", "→"),
                describe(&action["click"], catalog)
            )
        }
        other => other.to_string(),
    }
}

fn print_info(d: &Value, catalog: &Value) {
    let online = if d["online"] == true { style("32", "online") } else { style("90", "offline") };
    println!(
        "{}  {}  {}",
        style("1", d["name"].as_str().unwrap_or("?")),
        online,
        style("90", d["id"].as_str().unwrap_or(""))
    );
    let conn = &d["connection"];
    let via = conn["receiver"]
        .as_str()
        .map(str::to_string)
        .unwrap_or_else(|| conn["bus"].as_str().unwrap_or("?").to_string());
    println!("  connection   {via}");
    println!("  battery      {}", battery_text(d));
    if let Some(fw) = d["firmware"].as_array() {
        let list: Vec<String> = fw
            .iter()
            .map(|f| format!("{} {}", f["name"].as_str().unwrap_or(""), f["version"].as_str().unwrap_or("")))
            .collect();
        println!("  firmware     {}", list.join(", "));
    }
    if let Some(s) = d["serial"].as_str() {
        println!("  serial       {s}");
    }
    let st = &d["state"];
    if !st["dpi"].is_null() {
        let caps = &d["capabilities"]["dpi"];
        println!("  pointer      {} DPI (range {}–{})", st["dpi"], caps["min"], caps["max"]);
    }
    if !st["wheel_mode"].is_null() {
        let t = st["smartshift_threshold"].as_u64().unwrap_or(0);
        let ss = if t == 255 || t == 0 { "SmartShift off".to_string() } else { format!("SmartShift threshold {t}") };
        println!("  wheel        {} · {ss}", st["wheel_mode"].as_str().unwrap_or("?"));
    }
    if !st["hires_scroll"].is_null() {
        println!(
            "  scrolling    {} · {}",
            if st["hires_scroll"] == true { "high resolution" } else { "standard resolution" },
            if st["invert_scroll"] == true { "natural direction" } else { "standard direction" }
        );
    }
    if let Some(hosts) = st["hosts"].as_array() {
        for h in hosts {
            let mark = if h["current"] == true { style("32", "●") } else { "○".into() };
            println!(
                "  host {}       {mark} {}",
                h["index"].as_u64().unwrap_or(0) + 1,
                h["name"].as_str().unwrap_or("")
            );
        }
    }
    if let Some(controls) = d["controls"].as_array() {
        let bindings = &d["config"]["bindings"]["buttons"];
        println!("  controls");
        for c in controls.iter().filter(|c| c["is_virtual"] != true) {
            let key = c["key"].as_str().unwrap_or("");
            let binding = bindings.get(key).map(|b| describe(b, catalog)).unwrap_or_else(|| style("90", "default"));
            println!("    {:<14} {:<20} {binding}", key, c["name"].as_str().unwrap_or(""));
        }
    }
    if let Some(profiles) = d["config"]["profiles"].as_array() {
        for p in profiles {
            let active = if d["active_profile"] == p["id"] { style("32", " (active)") } else { String::new() };
            println!(
                "  profile      {} [{}] {}{active}",
                p["name"].as_str().unwrap_or(""),
                p["id"].as_str().unwrap_or(""),
                p["apps"]
            );
        }
    }
}

fn check(ok: bool, text: &str) -> bool {
    let mark = if ok { style("32", "✓") } else { style("31", "✗") };
    println!("{mark} {text}");
    ok
}

fn can_open(path: &std::path::Path) -> bool {
    std::fs::OpenOptions::new().read(true).write(true).open(path).is_ok()
}

async fn doctor(client: &Client) -> Result<()> {
    let mut healthy = true;
    let rules = ["/etc/udev/rules.d", "/usr/lib/udev/rules.d", "/lib/udev/rules.d"]
        .iter()
        .map(|d| std::path::Path::new(d).join("70-mouse-master.rules"))
        .find(|p| p.exists());
    healthy &= check(
        rules.is_some(),
        &match &rules {
            Some(p) => format!("udev rules installed ({})", p.display()),
            None => "udev rules missing: run scripts/install.sh, or use “Grant access” in the app".into(),
        },
    );
    healthy &= check(can_open(std::path::Path::new("/dev/uinput")), "/dev/uinput is accessible");
    let mut nodes = Vec::new();
    if let Ok(entries) = std::fs::read_dir("/sys/class/hidraw") {
        for entry in entries.flatten() {
            let uevent = std::fs::read_to_string(entry.path().join("device/uevent")).unwrap_or_default();
            if uevent.to_uppercase().contains(":0000046D:") {
                let name = uevent.lines().find_map(|l| l.strip_prefix("HID_NAME=")).unwrap_or("?").to_string();
                let dev = std::path::Path::new("/dev").join(entry.file_name());
                nodes.push((dev, name));
            }
        }
    }
    nodes.sort();
    if nodes.is_empty() {
        check(false, "no Logitech HID devices found (is the mouse on and connected?)");
        healthy = false;
    }
    for (dev, name) in &nodes {
        healthy &= check(can_open(dev), &format!("{} ({name}) is accessible", dev.display()));
    }
    match client.json("GetStatus").await {
        Ok(status) => {
            check(
                true,
                &format!(
                    "service {} is running on {}",
                    status["version"].as_str().unwrap_or("?"),
                    status["desktop"].as_str().unwrap_or("?")
                ),
            );
            healthy &= check(status["input_available"] == true, "virtual input devices are ready");
            for c in status["conflicts"].as_array().cloned().unwrap_or_default() {
                healthy &= check(
                    false,
                    &format!("{} is running — it also takes over buttons; quit it", c.as_str().unwrap_or("?")),
                );
            }
            for d in client.devices().await.unwrap_or_default() {
                let via = d["connection"]["receiver"]
                    .as_str()
                    .map(str::to_string)
                    .unwrap_or_else(|| d["connection"]["bus"].as_str().unwrap_or("?").to_string());
                check(
                    d["online"] == true,
                    &format!("{} via {via}, battery {}", d["name"].as_str().unwrap_or("?"), battery_text(&d)),
                );
            }
        }
        Err(e) => {
            healthy &= check(false, &format!("service: {e}"));
        }
    }
    if healthy {
        println!("\nEverything looks good.");
        Ok(())
    } else {
        bail!("some checks failed")
    }
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let client = Client::new().await?;
    match cli.command {
        Cmd::Doctor => doctor(&client).await?,
        Cmd::List => {
            for d in client.devices().await? {
                let online = if d["online"] == true { style("32", "●") } else { style("90", "○") };
                println!(
                    "{online} {:<24} {:<16} battery {}",
                    d["name"].as_str().unwrap_or("?"),
                    d["id"].as_str().unwrap_or(""),
                    battery_text(&d)
                );
            }
        }
        Cmd::Info { device, json } => {
            let d = client.device(device.as_deref()).await?;
            if json {
                println!("{}", serde_json::to_string_pretty(&d)?)
            } else {
                let catalog = client.json("GetCatalog").await.unwrap_or_default();
                print_info(&d, &catalog)
            }
        }
        Cmd::Set { device, settings } => {
            let d = client.device(device.as_deref()).await?;
            let mut patch = serde_json::Map::new();
            for kv in settings {
                let (k, v) = kv.split_once('=').ok_or_else(|| anyhow!("expected key=value, got {kv:?}"))?;
                let value = serde_json::from_str(v).unwrap_or_else(|_| Value::String(v.to_string()));
                patch.insert(k.to_string(), value);
            }
            client
                .command(d["id"].as_str().unwrap_or(""), json!({"command": "set-settings", "settings": patch}))
                .await?;
            println!("ok");
        }
        Cmd::Bind { device, profile, control, action } => {
            let d = client.device(device.as_deref()).await?;
            let action = parse_action(&action)?;
            let action = if action["type"] == "default" { Value::Null } else { action };
            client
                .command(
                    d["id"].as_str().unwrap_or(""),
                    json!({"command": "set-binding", "profile": profile, "control": control, "action": action}),
                )
                .await?;
            println!("ok");
        }
        Cmd::Run { device, action } => {
            let action = parse_action(&action)?;
            match device {
                Some(dev) => {
                    let d = client.device(Some(&dev)).await?;
                    client
                        .command(d["id"].as_str().unwrap_or(""), json!({"command": "run-action", "action": action}))
                        .await?;
                }
                None => {
                    // Device actions need a device; use the first one when there is one.
                    let first = client.device(None).await.ok();
                    match first {
                        Some(d) if d["online"] == true => {
                            client
                                .command(
                                    d["id"].as_str().unwrap_or(""),
                                    json!({"command": "run-action", "action": action}),
                                )
                                .await?;
                        }
                        _ => {
                            client.call("RunAction", &(action.to_string(),)).await?;
                        }
                    }
                }
            }
        }
        Cmd::Actions => {
            let catalog = client.json("GetCatalog").await?;
            println!("Desktop: {}", catalog["desktop"].as_str().unwrap_or("?"));
            for cat in catalog["categories"].as_array().cloned().unwrap_or_default() {
                println!("\n{}", style("1", cat["label"].as_str().unwrap_or("")));
                for a in catalog["actions"]
                    .as_array()
                    .cloned()
                    .unwrap_or_default()
                    .iter()
                    .filter(|a| a["category"] == cat["id"])
                {
                    let avail =
                        if a["available"] == true { String::new() } else { style("90", " (not available here)") };
                    println!("  {:<26} {}{avail}", a["id"].as_str().unwrap_or(""), a["label"].as_str().unwrap_or(""));
                }
            }
        }
        Cmd::Config => {
            println!("{}", serde_json::to_string_pretty(&client.json("GetConfig").await?)?);
        }
        Cmd::Host { device, host } => {
            if host == 0 {
                bail!("hosts are numbered from 1");
            }
            let d = client.device(device.as_deref()).await?;
            client.command(d["id"].as_str().unwrap_or(""), json!({"command": "switch-host", "host": host - 1})).await?;
            println!("ok");
        }
        Cmd::Monitor { motion } => {
            if motion {
                client.call("SetMonitoring", &(true,)).await?;
            }
            let rule = zbus::MatchRule::builder()
                .msg_type(zbus::message::Type::Signal)
                .sender(BUS_NAME)?
                .interface(INTERFACE)?
                .build();
            let mut stream = zbus::MessageStream::for_match_rule(rule, &client.conn, None).await?;
            println!("Listening… press buttons, use gestures, switch windows (Ctrl+C to stop)");
            while let Some(msg) = stream.next().await {
                let msg = msg?;
                let header = msg.header();
                let member = header.member().map(|m| m.to_string()).unwrap_or_default();
                match member.as_str() {
                    "ButtonEvent" => {
                        let (_id, control, pressed): (String, String, bool) = msg.body().deserialize()?;
                        println!("button   {control:<14} {}", if pressed { "pressed" } else { "released" });
                    }
                    "GestureEvent" => {
                        let (_id, control, gesture): (String, String, String) = msg.body().deserialize()?;
                        println!("gesture  {control:<14} {gesture}");
                    }
                    "MotionEvent" => {
                        let (_id, dx, dy): (String, i32, i32) = msg.body().deserialize()?;
                        println!("motion   dx={dx:<5} dy={dy}");
                    }
                    "FocusChanged" => {
                        let (app,): (String,) = msg.body().deserialize()?;
                        let v: Value = serde_json::from_str(&app).unwrap_or_default();
                        println!(
                            "focus    {} — {}",
                            v["app_id"].as_str().unwrap_or(""),
                            v["title"].as_str().unwrap_or("")
                        );
                    }
                    "DeviceChanged" => {
                        let (id, snap): (String, String) = msg.body().deserialize()?;
                        let v: Value = serde_json::from_str(&snap).unwrap_or_default();
                        println!(
                            "device   {id} online={} dpi={} wheel={} battery={}",
                            v["online"],
                            v["state"]["dpi"],
                            v["state"]["wheel_mode"].as_str().unwrap_or("-"),
                            battery_text(&v)
                        );
                    }
                    "DevicesChanged" => println!("devices  changed"),
                    _ => {}
                }
            }
        }
    }
    Ok(())
}
