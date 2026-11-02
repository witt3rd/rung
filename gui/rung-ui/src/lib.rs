//! What the window shows, with no GUI in it: the rows of the instance list
//! and the state of the link to the gateway, built from `rung-client`'s typed
//! answers. Everything here is plain data, so it is tested without a display.
//!
//! The words are the host's own (`rung-client` reads them from the summary
//! the gateway lists); nothing here folds a record or removes a secret.

use rung_client::{Client, Instances};

/// One line of the list: name, state word, what it is doing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub name: String,
    pub state: String,
    pub doing: String,
    /// The host asks for the owner (`Stuck` or `Down`).
    pub needs_you: bool,
}

/// One successful look at the gateway.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Snapshot {
    pub rows: Vec<Row>,
    /// The gateway's own note when it could not read its registry.
    pub note: Option<String>,
}

/// Where the link to the gateway stands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Link {
    /// Nothing asked yet.
    Connecting,
    /// The last look worked.
    Up,
    /// The last look failed, in plain words; the rows shown are the last good ones.
    Down(String),
}

impl Link {
    /// One line for the header.
    pub fn words(&self) -> String {
        match self {
            Link::Connecting => "connecting…".into(),
            Link::Up => "connected".into(),
            Link::Down(why) => format!("not connected: {why}"),
        }
    }
}

/// The list as the window shows it, in the gateway's order.
pub fn snapshot_of(list: &Instances) -> Snapshot {
    Snapshot {
        rows: list
            .instances
            .iter()
            .map(|i| Row {
                name: i.display_name().to_string(),
                state: i.state_word().to_string(),
                doing: i.doing().to_string(),
                needs_you: i.needs_you(),
            })
            .collect(),
        note: list.registry_error.clone().filter(|n| !n.is_empty()),
    }
}

/// One look: health first (so a dead gateway reads as that), then the list.
pub fn look(client: &Client) -> Result<Snapshot, String> {
    let health = client.health().map_err(|e| e.to_string())?;
    if !health.ok {
        return Err("the gateway says it is not ok".into());
    }
    client
        .instances()
        .map(|l| snapshot_of(&l))
        .map_err(|e| e.to_string())
}

/// The list as plain text, one instance per line (`--list`).
pub fn as_text(snap: &Snapshot) -> String {
    let w = snap
        .rows
        .iter()
        .map(|r| r.name.chars().count())
        .max()
        .unwrap_or(0);
    let s = snap
        .rows
        .iter()
        .map(|r| r.state.chars().count())
        .max()
        .unwrap_or(0);
    let mut out = String::new();
    for r in &snap.rows {
        let mark = if r.needs_you { "!" } else { " " };
        out.push_str(&format!(
            "{mark} {:w$}  {:s$}  {}\n",
            r.name,
            r.state,
            r.doing,
            w = w,
            s = s
        ));
    }
    if let Some(n) = &snap.note {
        out.push_str(&format!("(the gateway says: {n})\n"));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Read a list the way the gateway sends it.
    fn list(json: &str) -> Instances {
        serde_json::from_str(json).expect("a list")
    }

    /// A loopback stub that answers every request from `answer(path)`.
    fn stub(answer: fn(&str) -> (u16, &'static str)) -> String {
        use std::io::{BufRead, BufReader, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            for conn in listener.incoming().flatten() {
                let mut reader = BufReader::new(conn.try_clone().unwrap());
                let mut line = String::new();
                reader.read_line(&mut line).unwrap();
                let path = line.split_whitespace().nth(1).unwrap_or("").to_string();
                let (status, body) = answer(&path);
                let mut conn = conn;
                let _ = write!(
                    conn,
                    "HTTP/1.1 {status} X\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        });
        base
    }

    #[test]
    fn a_look_reads_health_then_the_list() {
        let base = stub(|p| match p {
            "/api/health" => (200, r#"{"ok":true}"#),
            "/api/instances" => (
                200,
                r#"{"instances":[{"id":"chief-a","summary":{"state":"Free time","doing":"Free time"}}]}"#,
            ),
            _ => (404, "{}"),
        });
        let snap = look(&Client::new(&base).unwrap()).unwrap();
        assert_eq!(snap.rows.len(), 1);
        assert_eq!(snap.rows[0].state, "Free time");
    }

    #[test]
    fn a_look_that_fails_says_why_and_a_not_ok_gateway_is_not_listed() {
        let base = stub(|_| (200, r#"{"ok":false}"#));
        let e = look(&Client::new(&base).unwrap()).unwrap_err();
        assert!(e.contains("not ok"), "{e}");
        let base = stub(|_| (500, r#"{"error":{"message":"broken"}}"#));
        let e = look(&Client::new(&base).unwrap()).unwrap_err();
        assert!(e.contains("500") && e.contains("broken"), "{e}");
    }

    #[test]
    fn rows_carry_the_hosts_words_in_the_gateways_order() {
        let snap = snapshot_of(&list(
            r#"{"instances":[
              {"id":"chief-main","name":"chief-main","word":"running","reachable":true,
               "summary":{"state":"Working","doing":"Committed to p: t","needs_you":false}},
              {"id":"steward-a","word":"down","reachable":true,
               "summary":{"state":"Down","doing":"Not running; no record","needs_you":true}},
              {"id":"far","name":"Far","reachable":false,"summary":null,"error":"no answer"}
            ],"registry_error":null}"#,
        ));
        let got: Vec<_> = snap
            .rows
            .iter()
            .map(|r| {
                (
                    r.name.as_str(),
                    r.state.as_str(),
                    r.doing.as_str(),
                    r.needs_you,
                )
            })
            .collect();
        assert_eq!(
            got,
            [
                ("chief-main", "Working", "Committed to p: t", false),
                ("steward-a", "Down", "Not running; no record", true),
                ("Far", "unreachable", "no answer", false),
            ]
        );
        assert_eq!(snap.note, None);
    }

    #[test]
    fn text_lists_one_line_each_and_marks_needs_you() {
        let snap = Snapshot {
            rows: vec![
                Row {
                    name: "a".into(),
                    state: "Working".into(),
                    doing: "x".into(),
                    needs_you: false,
                },
                Row {
                    name: "long-name".into(),
                    state: "Down".into(),
                    doing: "y".into(),
                    needs_you: true,
                },
            ],
            note: Some("registry unreadable".into()),
        };
        assert_eq!(
            as_text(&snap),
            "  a          Working  x\n! long-name  Down     y\n(the gateway says: registry unreadable)\n"
        );
    }

    #[test]
    fn an_empty_registry_note_is_no_note() {
        let snap = snapshot_of(&list(r#"{"instances":[],"registry_error":""}"#));
        assert_eq!(snap, Snapshot::default());
    }

    #[test]
    fn the_link_says_why_it_is_down() {
        assert_eq!(Link::Up.words(), "connected");
        assert!(Link::Down("refused".into()).words().contains("refused"));
    }
}
