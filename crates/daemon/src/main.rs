//! Mouse Master service.

mod actions;
mod config;
mod dbus;
mod devices;
mod env;
mod gestures;
mod hyprland;
mod input;
mod notify;
mod thumbwheel;
mod window;

#[cfg(test)]
mod tests;

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use clap::Parser;
use tokio::sync::{broadcast, watch};
use tracing::{info, warn};
use tracing_subscriber::EnvFilter;

use crate::devices::{DaemonEvent, DeviceMsg, Shared};
use crate::window::ActiveApp;

#[derive(Parser, Debug)]
#[command(name = "mouse-masterd", version, about = "Mouse Master background service")]
struct Args {
    /// Configuration file (default: $XDG_CONFIG_HOME/mouse-master/config.toml).
    #[arg(long)]
    config: Option<PathBuf>,
    /// More logging (-v debug, -vv trace).
    #[arg(short, long, action = clap::ArgAction::Count)]
    verbose: u8,
    /// Use simulated devices instead of real hardware (for trying the app and screenshots).
    #[arg(long)]
    demo: bool,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let default_level = match args.verbose {
        0 => "info,zbus=warn",
        1 => "debug,zbus=warn",
        _ => "trace,zbus=info",
    };
    let filter = EnvFilter::try_from_env("MOUSE_MASTER_LOG").unwrap_or_else(|_| EnvFilter::new(default_level));
    // systemd adds its own timestamps.
    let under_systemd = std::env::var_os("JOURNAL_STREAM").is_some();
    let builder = tracing_subscriber::fmt().with_env_filter(filter).with_target(args.verbose > 0);
    if under_systemd {
        builder.without_time().init()
    } else {
        builder.init()
    }

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .thread_name("mouse-masterd")
        .build()?;
    runtime.block_on(run(args))
}

async fn run(args: Args) -> Result<()> {
    info!("Mouse Master {} starting", env!("CARGO_PKG_VERSION"));
    let config_path = match (args.config, args.demo) {
        (Some(path), _) => path,
        (None, true) => env::runtime_dir().join("mouse-master-demo.toml"),
        (None, false) => config::config_path(),
    };
    let config = config::ConfigStore::open(config_path.clone());
    info!("configuration: {}", config_path.display());

    let input: Option<input::SharedInput> = match input::VirtualInput::new() {
        Ok(i) => Some(Arc::new(i)),
        Err(e) => {
            warn!("cannot create virtual input devices ({e}); button actions are disabled. Install the udev rules.");
            None
        }
    };

    let conn = zbus::Connection::session().await.context("connecting to the session bus")?;
    let (desktop_tx, desktop_rx) = watch::channel(env::detect(&conn).await);
    let (app_tx, app_rx) = watch::channel(ActiveApp::default());
    let (events_tx, _) = broadcast::channel(512);

    let executor = actions::exec::Executor::new(input.clone(), Some(conn.clone()), desktop_rx.clone());
    let notifier = notify::Notifier::new(Some(conn.clone()), desktop_rx.clone());
    let shared = Arc::new(Shared {
        config: config.clone(),
        executor,
        notifier,
        registry: Default::default(),
        events: events_tx.clone(),
        active_app: app_rx.clone(),
        monitoring: AtomicBool::new(false),
    });

    conn.object_server()
        .at(
            dbus::OBJECT_PATH,
            dbus::DaemonIface { shared: shared.clone(), desktop: desktop_rx.clone(), input_available: input.is_some() },
        )
        .await?;
    conn.object_server().at(dbus::OBJECT_PATH, dbus::TrackerIface { app_tx: app_tx.clone() }).await?;
    let reply = conn
        .request_name_with_flags(dbus::BUS_NAME, zbus::fdo::RequestNameFlags::DoNotQueue.into())
        .await
        .context("requesting the bus name")?;
    if reply != zbus::fdo::RequestNameReply::PrimaryOwner {
        bail!("another instance of mouse-masterd is already running");
    }

    tokio::spawn(dbus::pump_events(conn.clone(), shared.clone(), events_tx.subscribe()));
    {
        let shared = shared.clone();
        let mut rx = app_rx.clone();
        tokio::spawn(async move {
            while rx.changed().await.is_ok() {
                let app = rx.borrow_and_update().clone();
                tracing::debug!("focused: {} ({})", app.app_id, app.title);
                shared.emit(DaemonEvent::ActiveApp(app));
            }
        });
    }
    tokio::spawn(window::run(conn.clone(), desktop_tx, app_tx));
    let mut _simulations = Vec::new();
    if args.demo {
        use hidpp::sim::{DeviceSpec, Topology};
        info!("demo mode: using simulated devices");
        _simulations.push(devices::node::spawn_sim(shared.clone(), Topology::Direct(DeviceSpec::mx_master_3s()))?);
        _simulations.push(devices::node::spawn_sim(
            shared.clone(),
            Topology::Receiver { kind: hidpp::receiver::ReceiverKind::Bolt, devices: vec![DeviceSpec::mx_master_4()] },
        )?);
    } else {
        tokio::spawn(devices::node::run(shared.clone()));
    }

    wait_for_exit().await;
    info!("shutting down");

    // Give every device a chance to restore its native behaviour.
    let mut acks = Vec::new();
    for mailbox in shared.registry.mailboxes() {
        let (tx, rx) = tokio::sync::oneshot::channel();
        if mailbox.send(DeviceMsg::Stop(tx)).is_ok() {
            acks.push(rx);
        }
    }
    let _ = tokio::time::timeout(Duration::from_secs(3), futures::future::join_all(acks)).await;
    if *desktop_rx.borrow() == env::Desktop::Kde {
        window::kwin_unload_script(&conn).await;
    }
    if let Some(input) = &input {
        input.release_all();
    }
    config.flush();
    Ok(())
}

async fn wait_for_exit() {
    use tokio::signal::unix::{SignalKind, signal};
    let mut term = signal(SignalKind::terminate()).expect("SIGTERM handler");
    let mut int = signal(SignalKind::interrupt()).expect("SIGINT handler");
    let mut hup = signal(SignalKind::hangup()).expect("SIGHUP handler");
    tokio::select! {
        _ = term.recv() => {},
        _ = int.recv() => {},
        _ = hup.recv() => {},
    }
}
