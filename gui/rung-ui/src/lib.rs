//! What the window shows, with no GUI in it: the rows of the instance list
//! and the state of the link to the gateway, built from `rung-client`'s typed
//! answers. Everything here is plain data, so it is tested without a display.
//!
//! The words are the host's own (`rung-client` reads them from the summary
//! the gateway lists); nothing here folds a record or removes a secret.

use rung_client::{Client, Instance, Instances, Role};

/// One line of the list: name, state word, what it is doing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    /// The instance id (what a selection names).
    pub id: String,
    pub name: String,
    /// Chief, steward or neither, from the instance id.
    pub role: Role,
    /// The gateway cannot read it (no answer, or an unreadable entry).
    pub unreachable: bool,
    pub state: String,
    pub doing: String,
    /// The host asks for the owner (`Stuck` or `Down`).
    pub needs_you: bool,
    /// Everything the host's summary says, for the instance's own page.
    pub detail: Detail,
}

/// One instance's page: sections of label and value, all of it the host's own
/// words and numbers laid out for reading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Detail {
    pub sections: Vec<Section>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    pub heading: &'static str,
    pub lines: Vec<(&'static str, String)>,
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
                id: i.id.clone(),
                detail: detail_of(i),
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

/// Milliseconds since the epoch as `YYYY-MM-DD HH:MM:SS UTC`.
pub fn utc(ms: i64) -> String {
    let secs = ms.div_euclid(1000);
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // civil-from-days (Howard Hinnant's algorithm)
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02} UTC",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

fn usd(v: f64) -> String {
    if v != 0.0 && v.abs() < 0.01 {
        format!("${v:.4}")
    } else {
        format!("${v:.2}")
    }
}

fn clip(s: &str, n: usize) -> String {
    let line = s.lines().next().unwrap_or("").trim();
    if line.chars().count() > n || s.trim().lines().count() > 1 {
        format!("{}…", line.chars().take(n).collect::<String>())
    } else {
        line.to_string()
    }
}

fn waiting_words(w: &rung_client::WaitingFor) -> String {
    let at = |t: Option<i64>| t.map(utc).unwrap_or_else(|| "an unknown time".into());
    match w.kind.as_str() {
        "idle" => format!("resting until {}", at(w.until)),
        "degraded" => format!(
            "degraded{} until {}",
            w.why
                .as_deref()
                .map(|y| format!(" ({y})"))
                .unwrap_or_default(),
            at(w.until)
        ),
        "turn" => match w.turn {
            Some(n) => format!("turn {n} in progress"),
            None => "a turn in progress".into(),
        },
        "boundary" => format!("{} item(s) at the next boundary", w.pending.unwrap_or(0)),
        "calendar" => format!(
            "calendar: {} at {}",
            clip(w.text.as_deref().unwrap_or(""), 80),
            at(w.until)
        ),
        other => other.to_string(),
    }
}

/// The page for one instance, from the host's summary: Now, Queue, Spend,
/// Desk and Governor. A section with nothing to say is left out; an instance
/// the gateway cannot read says why instead.
pub fn detail_of(i: &Instance) -> Detail {
    let mut sections = Vec::new();
    let Some(s) = &i.summary else {
        sections.push(Section {
            heading: "Not read",
            lines: vec![(
                "Why",
                clip(
                    if i.doing().is_empty() {
                        "the gateway has no summary"
                    } else {
                        i.doing()
                    },
                    200,
                ),
            )],
        });
        return Detail { sections };
    };
    let mut now = vec![
        ("State", i.state_word().to_string()),
        ("Doing", i.doing().to_string()),
    ];
    if let Some(w) = &s.waiting_for {
        now.push(("Waiting", waiting_words(w)));
    }
    if let Some(t) = s.turn {
        now.push(("Turn", t.to_string()));
    }
    if let Some(t) = s.last_at {
        now.push(("Last activity", utc(t)));
    }
    sections.push(Section {
        heading: "Now",
        lines: now,
    });

    if let Some(q) = &s.queue {
        let mut lines = vec![(
            "Items",
            format!("{} waiting, {} in flight", q.waiting, q.in_flight),
        )];
        if let Some(h) = &q.head {
            let kind = h.intent.as_deref().unwrap_or("next");
            lines.push(("Next", format!("[{kind}] {}", clip(&h.text, 120))));
        }
        sections.push(Section {
            heading: "Queue",
            lines,
        });
    }

    let mut spend = Vec::new();
    if let Some(v) = s.spend_usd_day {
        spend.push(("Paid today", usd(v)));
    }
    if let Some(r) = s.requests {
        spend.push((
            "Requests today",
            match s.quota {
                Some(q) => format!("{r} of {q}"),
                None => r.to_string(),
            },
        ));
    }
    if let Some(d) = &s.desk
        && let Some(v) = d.spent_today_usd
    {
        let cap = d.limits.as_ref().and_then(|l| l.cap_usd_day);
        spend.push((
            "Desk spent today",
            match cap {
                Some(c) => format!("{} of {} a day", usd(v), usd(c)),
                None => usd(v),
            },
        ));
    }
    if !spend.is_empty() {
        sections.push(Section {
            heading: "Spend",
            lines: spend,
        });
    }

    if let Some(d) = &s.desk {
        let mut lines = Vec::new();
        if let Some(b) = &d.backend {
            lines.push(("Backend", b.clone()));
        }
        if let Some(m) = &d.mode {
            lines.push(("Mode", m.clone()));
        }
        if let Some(n) = d.interrupts_last_hour {
            lines.push(("Interrupts, last hour", n.to_string()));
        }
        if !d.enabled_groups.is_empty() {
            lines.push(("Tool groups on", d.enabled_groups.join(", ")));
        }
        if !lines.is_empty() {
            sections.push(Section {
                heading: "Desk",
                lines,
            });
        }
    }

    if let Some(g) = &s.governor {
        let mut lines = Vec::new();
        if let Some(m) = &g.model {
            lines.push((
                "Model",
                match (g.rung, g.ladder.len()) {
                    (Some(r), n) if n > 0 => format!("{m} (rung {} of {n})", r + 1),
                    _ => m.clone(),
                },
            ));
        }
        if g.failures.unwrap_or(0) > 0 {
            lines.push(("Failed turns in a row", g.failures.unwrap_or(0).to_string()));
        }
        if g.blocked {
            lines.push(("Blocked", "yes: a blocked incident is open".into()));
        }
        let off: Vec<String> = g
            .ladder
            .iter()
            .filter(|r| !r.available)
            .map(|r| r.model.clone().unwrap_or_else(|| "(unnamed)".into()))
            .collect();
        if !off.is_empty() {
            lines.push(("Unavailable rungs", off.join(", ")));
        }
        if !lines.is_empty() {
            sections.push(Section {
                heading: "Governor",
                lines,
            });
        }
    }
    Detail { sections }
}

/// One instance's page as plain text (`--show ID`).
pub fn detail_text(row: &Row) -> String {
    let mut out = format!("{}\n", row.name);
    for sec in &row.detail.sections {
        out.push_str(&format!("\n{}\n", sec.heading));
        let w = sec
            .lines
            .iter()
            .map(|(k, _)| k.chars().count())
            .max()
            .unwrap_or(0);
        for (k, v) in &sec.lines {
            out.push_str(&format!("  {k:w$}  {v}\n"));
        }
    }
    out
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

    const NOW: &str = r#"{"id":"chief-main","name":"chief-main","reachable":true,"summary":{
      "state":"Working","doing":"Committed to p: t","needs_you":false,
      "turn":4,"last_at":1000000000000,"requests":5,"quota":1000,"spend_usd_day":0.0042,
      "waiting_for":{"kind":"turn","turn":4},
      "queue":{"waiting":2,"in_flight":1,"head":{"text":"do the thing\nsecond line","intent":"steer"}},
      "desk":{"backend":"none","mode":"rule_only","spent_today_usd":0.5,"interrupts_last_hour":1,
              "enabled_groups":["core","read"],"limits":{"cap_usd_day":5.0}},
      "governor":{"rung":1,"model":"m-b","failures":2,"blocked":true,
                  "ladder":[{"model":"m-a","available":false},{"model":"m-b","available":true}]}}}"#;

    fn detail(json: &str) -> Detail {
        detail_of(&serde_json::from_str(json).expect("an instance"))
    }

    fn lines<'a>(d: &'a Detail, heading: &str) -> Vec<(&'a str, &'a str)> {
        d.sections
            .iter()
            .find(|s| s.heading == heading)
            .unwrap_or_else(|| panic!("no section {heading}: {d:?}"))
            .lines
            .iter()
            .map(|(k, v)| (*k, v.as_str()))
            .collect()
    }

    #[test]
    fn utc_is_the_calendar_time_of_a_millisecond_count() {
        assert_eq!(utc(0), "1970-01-01 00:00:00 UTC");
        assert_eq!(utc(1_000_000_000_000), "2001-09-09 01:46:40 UTC");
        assert_eq!(
            utc(1_709_164_800_000 + 86_399_000),
            "2024-02-29 23:59:59 UTC",
            "a leap day"
        );
        assert_eq!(utc(-1000), "1969-12-31 23:59:59 UTC");
    }

    #[test]
    fn the_page_lays_out_the_hosts_own_summary() {
        let d = detail(NOW);
        let heads: Vec<_> = d.sections.iter().map(|s| s.heading).collect();
        assert_eq!(heads, ["Now", "Queue", "Spend", "Desk", "Governor"]);
        assert_eq!(
            lines(&d, "Now"),
            [
                ("State", "Working"),
                ("Doing", "Committed to p: t"),
                ("Waiting", "turn 4 in progress"),
                ("Turn", "4"),
                ("Last activity", "2001-09-09 01:46:40 UTC"),
            ]
        );
        assert_eq!(
            lines(&d, "Queue"),
            [
                ("Items", "2 waiting, 1 in flight"),
                ("Next", "[steer] do the thing…")
            ]
        );
        assert_eq!(
            lines(&d, "Spend"),
            [
                ("Paid today", "$0.0042"),
                ("Requests today", "5 of 1000"),
                ("Desk spent today", "$0.50 of $5.00 a day"),
            ]
        );
        assert_eq!(
            lines(&d, "Desk"),
            [
                ("Backend", "none"),
                ("Mode", "rule_only"),
                ("Interrupts, last hour", "1"),
                ("Tool groups on", "core, read"),
            ]
        );
        assert_eq!(
            lines(&d, "Governor"),
            [
                ("Model", "m-b (rung 2 of 2)"),
                ("Failed turns in a row", "2"),
                ("Blocked", "yes: a blocked incident is open"),
                ("Unavailable rungs", "m-a"),
            ]
        );
    }

    #[test]
    fn a_thin_summary_gives_only_what_it_has() {
        let d = detail(r#"{"id":"a","summary":{"state":"Free time","doing":"Free time"}}"#);
        let heads: Vec<_> = d.sections.iter().map(|s| s.heading).collect();
        assert_eq!(heads, ["Now"]);
        assert_eq!(
            lines(&d, "Now"),
            [("State", "Free time"), ("Doing", "Free time")]
        );
    }

    #[test]
    fn a_resting_or_degraded_host_says_until_when() {
        let d = detail(
            r#"{"id":"a","summary":{"state":"Idle","doing":"Resting",
                "waiting_for":{"kind":"idle","until":1000000000000}}}"#,
        );
        assert_eq!(
            lines(&d, "Now")[2],
            ("Waiting", "resting until 2001-09-09 01:46:40 UTC")
        );
        let d = detail(
            r#"{"id":"a","summary":{"state":"Waiting","doing":"x",
                "waiting_for":{"kind":"degraded","why":"rate limit","until":0}}}"#,
        );
        assert_eq!(
            lines(&d, "Now")[2],
            (
                "Waiting",
                "degraded (rate limit) until 1970-01-01 00:00:00 UTC"
            )
        );
    }

    #[test]
    fn an_unreadable_instance_says_why_it_has_no_page() {
        let d = detail(r#"{"id":"far","reachable":false,"summary":null,"error":"no answer"}"#);
        assert_eq!(lines(&d, "Not read"), [("Why", "no answer")]);
    }

    #[test]
    fn text_of_a_page_is_the_name_then_each_section() {
        let snap = snapshot_of(&list(&format!(r#"{{"instances":[{NOW}]}}"#)));
        let text = detail_text(&snap.rows[0]);
        assert!(
            text.starts_with("chief-main\n\nNow\n  State          Working\n"),
            "{text}"
        );
        assert!(
            text.contains(
                "\nQueue\n  Items  2 waiting, 1 in flight\n  Next   [steer] do the thing…\n"
            ),
            "{text}"
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
