//! `sd_notify`: tell the supervisor the host is ready, alive, and stopping.
//!
//! The protocol is one datagram per message to the unix socket named by
//! `$NOTIFY_SOCKET` (a path, or `@name` for the abstract namespace). With
//! no socket set every call is a no-op, so the host runs the same outside
//! systemd. The watchdog interval comes from `$WATCHDOG_USEC`; the loop
//! pings at each boundary and during waits, never from a side thread, so a
//! wedged loop misses its pings and the supervisor restarts it.

use std::os::unix::net::UnixDatagram;
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct Notifier {
    socket: Option<(UnixDatagram, String)>,
    watchdog: Option<Duration>,
    last_ping: Mutex<Option<Instant>>,
}

impl Notifier {
    /// From `$NOTIFY_SOCKET` and `$WATCHDOG_USEC`.
    pub fn from_env() -> Self {
        let path = std::env::var("NOTIFY_SOCKET")
            .ok()
            .filter(|s| !s.is_empty());
        let socket = path.and_then(|p| UnixDatagram::unbound().ok().map(|s| (s, p)));
        let watchdog = std::env::var("WATCHDOG_USEC")
            .ok()
            .and_then(|s| s.parse::<u64>().ok())
            .filter(|n| *n > 0)
            .map(Duration::from_micros);
        Self {
            socket,
            watchdog,
            last_ping: Mutex::new(None),
        }
    }

    /// A notifier that sends nothing.
    pub fn none() -> Self {
        Self {
            socket: None,
            watchdog: None,
            last_ping: Mutex::new(None),
        }
    }

    fn send(&self, msg: &str) {
        let Some((sock, path)) = &self.socket else {
            return;
        };
        let _ = if let Some(name) = path.strip_prefix('@') {
            send_abstract(sock, name, msg)
        } else {
            sock.send_to(msg.as_bytes(), path).map(|_| ())
        };
    }

    pub fn ready(&self) {
        self.send("READY=1");
    }

    pub fn stopping(&self) {
        self.send("STOPPING=1");
    }

    pub fn status(&self, s: &str) {
        self.send(&format!("STATUS={}", s.replace('\n', " ")));
    }

    /// A watchdog ping, at most every half interval (and at least every
    /// [`PING_FLOOR`] while a socket is set).
    pub fn alive(&self) {
        if self.socket.is_none() {
            return;
        }
        let every = self
            .watchdog
            .map(|w| (w / 2).min(PING_FLOOR))
            .unwrap_or(PING_FLOOR);
        let mut last = self.last_ping.lock().expect("notify");
        if last.is_none_or(|t| t.elapsed() >= every) {
            *last = Some(Instant::now());
            self.send("WATCHDOG=1");
        }
    }
}

/// The longest gap between pings while the loop is healthy.
pub const PING_FLOOR: Duration = Duration::from_secs(10);

#[cfg(target_os = "linux")]
fn send_abstract(sock: &UnixDatagram, name: &str, msg: &str) -> std::io::Result<()> {
    use std::os::linux::net::SocketAddrExt;
    let addr = std::os::unix::net::SocketAddr::from_abstract_name(name.as_bytes())?;
    sock.send_to_addr(msg.as_bytes(), &addr).map(|_| ())
}

#[cfg(not(target_os = "linux"))]
fn send_abstract(_sock: &UnixDatagram, _name: &str, _msg: &str) -> std::io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_reach_the_socket() {
        let guard = crate::sim::temp_dir_guard("notify");
        let path = guard.path().join("notify.sock");
        let server = UnixDatagram::bind(&path).unwrap();
        server
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let n = Notifier {
            socket: Some((UnixDatagram::unbound().unwrap(), path.display().to_string())),
            watchdog: Some(Duration::from_secs(4)),
            last_ping: Mutex::new(None),
        };
        n.ready();
        n.alive();
        n.alive(); // within the interval: not sent again
        n.stopping();
        let mut got = Vec::new();
        let mut buf = [0u8; 256];
        for _ in 0..3 {
            let k = server.recv(&mut buf).unwrap();
            got.push(String::from_utf8_lossy(&buf[..k]).to_string());
        }
        assert_eq!(got, vec!["READY=1", "WATCHDOG=1", "STOPPING=1"]);
        let _ = std::fs::remove_file(&path);
    }
}
