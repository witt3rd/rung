//! Blocking SSE body read with a per-line idle deadline.

use super::error::RawCallError;
use std::io::{BufRead, BufReader};
use std::sync::mpsc;
use std::time::Duration;

/// Read the response body as lines, failing with [`RawCallError::IdleTimeout`]
/// if no line arrives within `idle`.
pub fn read_lines_idle(
    response: reqwest::blocking::Response,
    idle: Duration,
) -> Result<Vec<String>, RawCallError> {
    let (tx, rx) = mpsc::sync_channel::<Option<Result<String, String>>>(16);
    std::thread::spawn(move || {
        let mut reader = BufReader::new(response);
        let mut line = String::new();
        loop {
            line.clear();
            match reader.read_line(&mut line) {
                Ok(0) => {
                    let _ = tx.send(None);
                    break;
                }
                Ok(_) => {
                    let trimmed = line.trim_end_matches(['\r', '\n']).to_string();
                    if tx.send(Some(Ok(trimmed))).is_err() {
                        break;
                    }
                }
                Err(e) => {
                    let _ = tx.send(Some(Err(e.to_string())));
                    break;
                }
            }
        }
    });

    let mut out = Vec::new();
    loop {
        match rx.recv_timeout(idle) {
            Ok(None) => return Ok(out),
            Ok(Some(Ok(line))) => out.push(line),
            Ok(Some(Err(e))) => {
                return Err(RawCallError::Transport {
                    message: format!("body read error: {e}"),
                    observed: !out.is_empty(),
                });
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                return Err(RawCallError::IdleTimeout {
                    elapsed_secs: idle.as_secs(),
                });
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(out),
        }
    }
}
