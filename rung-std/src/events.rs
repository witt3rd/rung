//! Where diagnostics go.
//!
//! A diagnostic is one line for whoever watches a run: an LLM call starting,
//! a retry, an elision, a tool and its result. By default each is written to
//! standard error, byte for byte as the blocks always wrote it. A caller that
//! keeps its own record installs an [`EventSink`] for the work it runs on the
//! current thread, and gets each diagnostic as an [`Event`] instead.
//!
//! The sink is scoped to the thread, like the work: the agent loop, its tool
//! calls and its LLM calls run on the thread that called it. [`install`]
//! returns a guard; dropping it puts back whatever sink was there before, so
//! a nested run (a `task` child) can install its own.

use std::cell::RefCell;
use std::sync::Arc;

/// One diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Event<'a> {
    /// Who emitted it: `rung-std`, `rung-agent`, `mcp`.
    pub source: &'a str,
    /// What happened, as a dotted tag (`llm.call`, `llm.retry`,
    /// `llm.overflow`, `tool.result`, `memory.unavailable`, …). Stable for
    /// a caller to match on; the line is for people.
    pub kind: &'a str,
    /// The line exactly as standard error shows it, without the newline.
    pub line: &'a str,
}

/// Receives diagnostics.
pub trait EventSink: Send + Sync {
    fn event(&self, e: &Event<'_>);
}

/// The default: each event's line on standard error.
#[derive(Debug, Clone, Copy, Default)]
pub struct Stderr;

impl EventSink for Stderr {
    fn event(&self, e: &Event<'_>) {
        eprintln!("{}", e.line);
    }
}

thread_local! {
    static SINK: RefCell<Option<Arc<dyn EventSink>>> = const { RefCell::new(None) };
}

/// Restores the sink that was installed before it.
#[must_use = "the sink is uninstalled when the guard drops"]
pub struct SinkGuard {
    prev: Option<Arc<dyn EventSink>>,
}

impl Drop for SinkGuard {
    fn drop(&mut self) {
        let prev = self.prev.take();
        SINK.with(|s| *s.borrow_mut() = prev);
    }
}

/// The sink installed on this thread, if any: for work handed to another
/// thread that should report to the same place.
pub fn current() -> Option<Arc<dyn EventSink>> {
    SINK.with(|s| s.borrow().clone())
}

/// Send this thread's diagnostics to `sink` until the guard drops.
pub fn install(sink: Arc<dyn EventSink>) -> SinkGuard {
    let prev = SINK.with(|s| s.borrow_mut().replace(sink));
    SinkGuard { prev }
}

/// Emit one diagnostic to this thread's sink, or to standard error.
pub fn emit(source: &str, kind: &str, line: &str) {
    let e = Event { source, kind, line };
    let sink = SINK.with(|s| s.borrow().clone());
    match sink {
        Some(sink) => sink.event(&e),
        None => Stderr.event(&e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[derive(Default)]
    struct Keep(Mutex<Vec<String>>);

    impl EventSink for Keep {
        fn event(&self, e: &Event<'_>) {
            self.0
                .lock()
                .unwrap()
                .push(format!("{} {} {}", e.source, e.kind, e.line));
        }
    }

    #[test]
    fn an_installed_sink_gets_events_until_its_guard_drops() {
        let outer = Arc::new(Keep::default());
        let inner = Arc::new(Keep::default());
        {
            let _a = install(outer.clone());
            emit("rung-std", "llm.call", "one");
            {
                let _b = install(inner.clone());
                emit("rung-std", "llm.retry", "two");
            }
            emit("mcp", "mcp.error", "three");
        }
        emit("rung-std", "llm.call", "to standard error");
        assert_eq!(
            *outer.0.lock().unwrap(),
            ["rung-std llm.call one", "mcp mcp.error three"]
        );
        assert_eq!(*inner.0.lock().unwrap(), ["rung-std llm.retry two"]);
    }

    #[test]
    fn a_sink_is_scoped_to_its_thread() {
        let keep = Arc::new(Keep::default());
        let _g = install(keep.clone());
        std::thread::spawn(|| emit("rung-std", "llm.call", "elsewhere"))
            .join()
            .unwrap();
        assert!(keep.0.lock().unwrap().is_empty());
    }
}
