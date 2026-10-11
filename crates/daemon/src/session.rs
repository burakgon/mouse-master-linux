//! Whether the user owning the service is at the computer.
//!
//! With fast user switching another user's session can take over the seat
//! while this service keeps running. Device access granted by udev follows the
//! active session, but descriptors that are already open stay valid and input
//! from our virtual devices would reach the other user's desktop. So while our
//! user is not in the foreground, devices are handed back to their native
//! behaviour and actions are not performed.

use std::time::Duration;

use tokio::sync::watch;
use tracing::{debug, info};
use zbus::zvariant::OwnedObjectPath;

const POLL: Duration = Duration::from_secs(2);

async fn property(
    conn: &zbus::Connection,
    path: &str,
    interface: &str,
    name: &str,
) -> Option<zbus::zvariant::OwnedValue> {
    let reply = conn
        .call_method(
            Some("org.freedesktop.login1"),
            path,
            Some("org.freedesktop.DBus.Properties"),
            "Get",
            &(interface, name),
        )
        .await
        .ok()?;
    reply.body().deserialize().ok()
}

/// True when the user's graphical ("display") session is in the foreground.
/// Falls back to the user's overall state when there is no display session.
async fn present(conn: &zbus::Connection, user: &OwnedObjectPath) -> Option<bool> {
    if let Some(display) = property(conn, user.as_str(), "org.freedesktop.login1.User", "Display").await {
        if let Ok((_, session)) = <(String, OwnedObjectPath)>::try_from(display) {
            if session.as_str() != "/" {
                let active = property(conn, session.as_str(), "org.freedesktop.login1.Session", "Active").await?;
                return bool::try_from(active).ok();
            }
        }
    }
    let state = property(conn, user.as_str(), "org.freedesktop.login1.User", "State").await?;
    String::try_from(state).ok().map(|s| s == "active")
}

/// Follows logind's view of our user: "active" means one of its sessions is in
/// the foreground. Without logind (containers, unusual setups) the user is
/// always considered present.
pub async fn run(active: watch::Sender<bool>) {
    let Ok(conn) = zbus::Connection::system().await else {
        debug!("no system bus; assuming the user is always present");
        return;
    };
    let uid = unsafe { libc::getuid() };
    let user: OwnedObjectPath = match conn
        .call_method(
            Some("org.freedesktop.login1"),
            "/org/freedesktop/login1",
            Some("org.freedesktop.login1.Manager"),
            "GetUser",
            &(uid,),
        )
        .await
        .and_then(|m| m.body().deserialize::<OwnedObjectPath>())
    {
        Ok(path) => path,
        Err(e) => {
            debug!("logind does not know our user ({e}); assuming the user is always present");
            return;
        }
    };
    loop {
        if let Some(now) = present(&conn, &user).await {
            active.send_if_modified(|current| {
                if *current != now {
                    info!("user session {}", if now { "active again" } else { "inactive: releasing devices" });
                    *current = now;
                    true
                } else {
                    false
                }
            });
        }
        tokio::time::sleep(POLL).await;
    }
}
