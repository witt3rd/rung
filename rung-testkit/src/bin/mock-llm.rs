//! Scripted OpenAI-compatible server for the example agents.
//!
//! usage: mock-llm PORT_FILE REPLY [REPLY ...]
//! Answers the Nth chat request with text REPLY N, writes the port to
//! PORT_FILE and appends every request body to PORT_FILE.requests.
use std::io::Write;

use rung_testkit::llm::serve_llm;
use serde_json::json;

fn main() {
    let mut args = std::env::args().skip(1);
    let port_file = args.next().expect("PORT_FILE");
    let replies = args
        .map(|t| json!({"id": "c", "model": "m", "choices": [{"message": {"content": t}, "finish_reason": "stop"}]}))
        .collect();
    let log = format!("{port_file}.requests");
    let url = serve_llm(replies, move |r| {
        let body = r.json().to_string();
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log)
            .unwrap();
        writeln!(f, "{body}").unwrap();
    });
    let port = url.rsplit(':').next().unwrap();
    std::fs::write(&port_file, port).unwrap();
    loop {
        std::thread::sleep(std::time::Duration::from_secs(3600));
    }
}
