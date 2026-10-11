//! Hyprland IPC: requests over `.socket.sock` and events from `.socket2.sock`.

use std::io;
use std::path::PathBuf;

use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

use crate::env::runtime_dir;

#[derive(Debug, Clone)]
pub struct Instance {
    dir: PathBuf,
}

impl Instance {
    /// Finds the running Hyprland instance: the one named by
    /// `HYPRLAND_INSTANCE_SIGNATURE`, otherwise the most recent instance
    /// directory whose request socket exists.
    pub fn find() -> Option<Instance> {
        let bases = [runtime_dir().join("hypr"), PathBuf::from("/tmp/hypr")];
        if let Ok(sig) = std::env::var("HYPRLAND_INSTANCE_SIGNATURE") {
            for base in &bases {
                let dir = base.join(&sig);
                if dir.join(".socket.sock").exists() {
                    return Some(Instance { dir });
                }
            }
        }
        let mut candidates: Vec<(std::time::SystemTime, PathBuf)> = Vec::new();
        for base in &bases {
            let Ok(entries) = std::fs::read_dir(base) else { continue };
            for entry in entries.flatten() {
                let dir = entry.path();
                let sock = dir.join(".socket.sock");
                if let Ok(meta) = std::fs::metadata(&sock) {
                    // A stale socket file cannot be connected to; check cheaply.
                    if std::os::unix::net::UnixStream::connect(&sock).is_ok() {
                        candidates.push((meta.modified().unwrap_or(std::time::UNIX_EPOCH), dir));
                    }
                }
            }
        }
        candidates.sort();
        candidates.pop().map(|(_, dir)| Instance { dir })
    }

    /// Sends a request (e.g. `dispatch workspace e+1` or `j/activewindow`) and returns the reply.
    pub async fn request(&self, command: &str) -> io::Result<String> {
        let mut stream = UnixStream::connect(self.dir.join(".socket.sock")).await?;
        stream.write_all(command.as_bytes()).await?;
        let mut reply = String::new();
        stream.read_to_string(&mut reply).await?;
        Ok(reply)
    }

    pub async fn dispatch(&self, dispatcher: &str) -> io::Result<()> {
        let reply = self.request(&format!("dispatch {dispatcher}")).await?;
        if reply.trim() == "ok" { Ok(()) } else { Err(io::Error::other(reply.trim().to_string())) }
    }

    /// Opens the event socket; returns a line reader of `event>>data` lines.
    pub async fn events(&self) -> io::Result<tokio::io::Lines<BufReader<UnixStream>>> {
        let stream = UnixStream::connect(self.dir.join(".socket2.sock")).await?;
        Ok(BufReader::new(stream).lines())
    }
}
