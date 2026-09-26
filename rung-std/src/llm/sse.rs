//! Blocking SSE body read with a per-line idle deadline.
//!
//! The deadline resets only on lines that carry content. SSE comments
//! (`: OPENROUTER PROCESSING`) and blank frame separators are keepalives:
//! a provider that sends only those is stalled, and must time out.

use super::error::RawCallError;
use std::io::{BufRead, BufReader, Read};
use std::sync::mpsc;
use std::time::{Duration, Instant};

fn is_progress(line: &str) -> bool {
    !line.is_empty() && !line.starts_with(':')
}

/// Read the response body as lines, failing with [`RawCallError::IdleTimeout`]
/// if no content line arrives within `idle`.
pub fn read_lines_idle(
    response: impl Read + Send + 'static,
    idle: Duration,
) -> Result<Vec<String>, RawCallError> {
    let mut out = Vec::new();
    read_lines_idle_each(response, idle, |line| {
        out.push(line.to_string());
        Ok(())
    })?;
    Ok(out)
}

/// Read response lines under the same idle deadline, delivering each line to
/// the parser immediately instead of buffering the complete SSE response.
pub fn read_lines_idle_each(
    response: impl Read + Send + 'static,
    idle: Duration,
    mut each: impl FnMut(&str) -> Result<(), RawCallError>,
) -> Result<(), RawCallError> {
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

    let mut observed = false;
    let mut deadline = Instant::now() + idle;
    loop {
        match rx.recv_timeout(deadline.saturating_duration_since(Instant::now())) {
            Ok(None) => return Ok(()),
            Ok(Some(Ok(line))) => {
                observed = true;
                if is_progress(&line) {
                    deadline = Instant::now() + idle;
                }
                each(&line)?;
            }
            Ok(Some(Err(e))) => {
                return Err(RawCallError::Transport {
                    message: format!("body read error: {e}"),
                    observed,
                });
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                return Err(RawCallError::IdleTimeout {
                    elapsed_secs: idle.as_secs(),
                });
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    /// A body that yields the given chunks with a pause before each, then
    /// either ends or stalls forever.
    struct Paced {
        chunks: Vec<(Duration, Vec<u8>)>,
        stall: bool,
    }

    impl Read for Paced {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if self.chunks.is_empty() {
                if self.stall {
                    std::thread::sleep(Duration::from_secs(3600));
                }
                return Ok(0);
            }
            let (pause, bytes) = self.chunks.remove(0);
            std::thread::sleep(pause);
            let n = bytes.len().min(buf.len());
            buf[..n].copy_from_slice(&bytes[..n]);
            if n < bytes.len() {
                self.chunks.insert(0, (Duration::ZERO, bytes[n..].to_vec()));
            }
            Ok(n)
        }
    }

    fn paced(chunks: &[(u64, &str)], stall: bool) -> Paced {
        Paced {
            chunks: chunks
                .iter()
                .map(|(ms, s)| (Duration::from_millis(*ms), s.as_bytes().to_vec()))
                .collect(),
            stall,
        }
    }

    #[test]
    fn stalled_provider_times_out_instead_of_hanging() {
        let body = paced(&[(0, "data: {\"a\":1}\n")], true);
        let started = Instant::now();
        let err = read_lines_idle(body, Duration::from_millis(300)).unwrap_err();
        assert!(matches!(err, RawCallError::IdleTimeout { .. }), "{err:?}");
        assert!(started.elapsed() < Duration::from_secs(5));
    }

    #[test]
    fn keepalive_comments_do_not_reset_the_idle_deadline() {
        let keepalives: Vec<(u64, &str)> = (0..20)
            .map(|_| (100, ": OPENROUTER PROCESSING\n\n"))
            .collect();
        let started = Instant::now();
        let err =
            read_lines_idle(paced(&keepalives, true), Duration::from_millis(500)).unwrap_err();
        assert!(matches!(err, RawCallError::IdleTimeout { .. }), "{err:?}");
        assert!(started.elapsed() < Duration::from_millis(1500));
    }

    #[test]
    fn content_lines_reset_the_idle_deadline() {
        let data: Vec<(u64, &str)> = (0..6).map(|_| (200, "data: {}\n")).collect();
        let lines = read_lines_idle(paced(&data, false), Duration::from_millis(500)).unwrap();
        assert_eq!(lines.len(), 6);
    }
}
