//! What the window shows, with no GUI in it: the rows of the instance list
//! and the state of the link to the gateway, built from `rung-client`'s typed
//! answers. Everything here is plain data, so it is tested without a display.
//!
//! The words are the host's own (`rung-client` reads them from the summary
//! the gateway lists); nothing here folds a record or removes a secret.

use rung_client::{Client, Instances, Role};

/// One line of the list: name, state word, what it is doing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub name: String,
    /// Chief, steward or neither, from the instance id.
    pub role: Role,
    /// The gateway cannot read it (no answer, or an unreadable entry).
    pub unreachable: bool,
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

/// The rows of one role, in the gateway's order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group<'a> {
    pub role: Role,
    pub rows: Vec<&'a Row>,
}

/// How many of each, for the header.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Counts {
    pub chiefs: usize,
    pub stewards: usize,
    pub others: usize,
    pub needs_you: usize,
    pub unreachable: usize,
}

impl Snapshot {
    /// The rows grouped by role: chiefs, then stewards, then the rest; a role
    /// with no rows is left out. The id does not say which chief a steward
    /// serves (the fleet props give the registry no such field), so a steward
    /// is listed under its role and never under a chief it may not have.
    pub fn groups(&self) -> Vec<Group<'_>> {
        [Role::Chief, Role::Steward, Role::Other]
            .into_iter()
            .map(|role| Group {
                role,
                rows: self.rows.iter().filter(|r| r.role == role).collect(),
            })
            .filter(|g| !g.rows.is_empty())
            .collect()
    }

    pub fn counts(&self) -> Counts {
        let mut c = Counts::default();
        for r in &self.rows {
            match r.role {
                Role::Chief => c.chiefs += 1,
                Role::Steward => c.stewards += 1,
                Role::Other => c.others += 1,
            }
            c.needs_you += usize::from(r.needs_you);
            c.unreachable += usize::from(r.unreachable);
        }
        c
    }
}

impl Counts {
    /// One line for the header: `1 chief · 2 stewards · 1 needs you · 1 unreachable`;
    /// what is zero is left out, and an empty fleet says so.
    pub fn words(&self) -> String {
        let plural =
            |n: usize, one: &str, many: &str| format!("{n} {}", if n == 1 { one } else { many });
        let mut parts = Vec::new();
        if self.chiefs > 0 {
            parts.push(plural(self.chiefs, "chief", "chiefs"));
        }
        if self.stewards > 0 {
            parts.push(plural(self.stewards, "steward", "stewards"));
        }
        if self.others > 0 {
            parts.push(plural(self.others, "other", "others"));
        }
        if self.needs_you > 0 {
            parts.push(format!("{} needs you", self.needs_you));
        }
        if self.unreachable > 0 {
            parts.push(format!("{} unreachable", self.unreachable));
        }
        if parts.is_empty() {
            "no instances".into()
        } else {
            parts.join(" · ")
        }
    }
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
                role: i.role(),
                unreachable: i.unreachable(),
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

/// The list as plain text, grouped by role, one instance per line
/// (`--list`). `!` marks a host that needs you, `?` one the gateway cannot read.
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
    for g in snap.groups() {
        out.push_str(&format!("{} ({})\n", g.role.plural(), g.rows.len()));
        for r in g.rows {
            let mark = if r.needs_you {
                "!"
            } else if r.unreachable {
                "?"
            } else {
                " "
            };
            out.push_str(&format!(
                "{mark} {:w$}  {:s$}  {}\n",
                r.name,
                r.state,
                r.doing,
                w = w,
                s = s
            ));
        }
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

    /// Two chiefs, two stewards (one down, one unreachable) and a plain
    /// instance, deliberately out of role order.
    const FLEET: &str = r#"{"instances":[
      {"id":"steward-alpha","word":"running","reachable":true,
       "summary":{"state":"Working","doing":"Committed to p: t","needs_you":false}},
      {"id":"scratch","word":"running","reachable":true,
       "summary":{"state":"Free time","doing":"Free time","needs_you":false}},
      {"id":"chief-main","word":"running","reachable":true,
       "summary":{"state":"Free time","doing":"Free time","needs_you":false}},
      {"id":"steward-beta","word":"down","reachable":true,
       "summary":{"state":"Down","doing":"Not running; no record","needs_you":true}},
      {"id":"chief-second","word":"running","reachable":true,
       "summary":{"state":"Working","doing":"Committed to q: u","needs_you":false}},
      {"id":"steward-ghost","name":"Ghost","source":"config","reachable":false,
       "summary":null,"error":"the instance did not answer in time"}
    ],"registry_error":null}"#;

    fn names<'a>(g: &Group<'a>) -> Vec<&'a str> {
        g.rows.iter().map(|r| r.name.as_str()).collect()
    }

    #[test]
    fn rows_carry_the_hosts_words_and_the_role_of_the_id() {
        let snap = snapshot_of(&list(FLEET));
        let got: Vec<_> = snap
            .rows
            .iter()
            .map(|r| {
                (
                    r.name.as_str(),
                    r.role,
                    r.state.as_str(),
                    r.needs_you,
                    r.unreachable,
                )
            })
            .collect();
        assert_eq!(
            got,
            [
                ("steward-alpha", Role::Steward, "Working", false, false),
                ("scratch", Role::Other, "Free time", false, false),
                ("chief-main", Role::Chief, "Free time", false, false),
                ("steward-beta", Role::Steward, "Down", true, false),
                ("chief-second", Role::Chief, "Working", false, false),
                ("Ghost", Role::Steward, "unreachable", false, true),
            ]
        );
        // the role comes from the id, not the display name
        assert_eq!(snap.rows[5].role, Role::Steward);
    }

    #[test]
    fn groups_are_chiefs_then_stewards_then_the_rest_each_in_the_gateways_order() {
        let snap = snapshot_of(&list(FLEET));
        let groups = snap.groups();
        let roles: Vec<_> = groups.iter().map(|g| g.role).collect();
        assert_eq!(roles, [Role::Chief, Role::Steward, Role::Other]);
        assert_eq!(names(&groups[0]), ["chief-main", "chief-second"]);
        assert_eq!(
            names(&groups[1]),
            ["steward-alpha", "steward-beta", "Ghost"]
        );
        assert_eq!(names(&groups[2]), ["scratch"]);
    }

    #[test]
    fn a_role_with_no_rows_has_no_group_and_an_empty_fleet_has_none() {
        let only_stewards = snapshot_of(&list(
            r#"{"instances":[{"id":"steward-a","summary":{"state":"Free time"}}]}"#,
        ));
        let roles: Vec<_> = only_stewards.groups().iter().map(|g| g.role).collect();
        assert_eq!(roles, [Role::Steward]);
        assert!(Snapshot::default().groups().is_empty());
    }

    #[test]
    fn counts_say_how_many_of_each_and_who_needs_you() {
        let snap = snapshot_of(&list(FLEET));
        assert_eq!(
            snap.counts(),
            Counts {
                chiefs: 2,
                stewards: 3,
                others: 1,
                needs_you: 1,
                unreachable: 1
            }
        );
        assert_eq!(
            snap.counts().words(),
            "2 chiefs · 3 stewards · 1 other · 1 needs you · 1 unreachable"
        );
        let one = Counts {
            chiefs: 1,
            ..Counts::default()
        };
        assert_eq!(one.words(), "1 chief");
        assert_eq!(Counts::default().words(), "no instances");
    }

    #[test]
    fn text_is_grouped_and_marks_needs_you_and_unreachable() {
        let mut snap = snapshot_of(&list(FLEET));
        snap.note = Some("registry unreadable".into());
        assert_eq!(
            as_text(&snap),
            "\
Chiefs (2)
  chief-main     Free time    Free time
  chief-second   Working      Committed to q: u
Stewards (3)
  steward-alpha  Working      Committed to p: t
! steward-beta   Down         Not running; no record
? Ghost          unreachable  the instance did not answer in time
Other instances (1)
  scratch        Free time    Free time
(the gateway says: registry unreadable)
"
        );
        assert_eq!(as_text(&Snapshot::default()), "");
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
