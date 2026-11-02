//! What the window shows, with no GUI in it: the rows of the instance list
//! and the state of the link to the gateway, built from `rung-client`'s typed
//! answers. Everything here is plain data, so it is tested without a display.
//!
//! The words are the host's own (`rung-client` reads them from the summary
//! the gateway lists); nothing here folds a record or removes a secret.

use std::collections::VecDeque;

use rung_client::{
    Band, Client, ContextView, Error, Event, Frame, Instance, Instances, QueueItem, Role,
};

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
    /// Its newest record line number: where a followed stream may start.
    pub last_seq: Option<u64>,
    /// The turn it is on (the context inspector starts there).
    pub turn: Option<u64>,
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
                last_seq: i.summary.as_ref().and_then(|s| s.last_seq),
                turn: i.summary.as_ref().and_then(|s| s.turn),
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

/// The most record lines the page keeps (older ones fall off the far end).
pub const FEED_CAP: usize = 300;

/// One line of the followed record, as one line of text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeedLine {
    pub seq: u64,
    /// When the host wrote it (ms since the epoch).
    pub at: Option<i64>,
    pub kind: String,
    /// The first readable words of the line, if it has any.
    pub text: String,
}

/// The running turn's partial text and tool progress: short-lived, never in
/// the record. The finished turn is in the record lines.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Partial {
    pub turn: Option<u64>,
    pub text: String,
    /// Tools begun or done in this turn, in order (`name`, `name ✓`, `name ✗`).
    pub tools: Vec<String>,
    /// Partial frames were missed (the reader fell behind); the record has them.
    pub gap: bool,
}

/// Where a followed stream stands.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum StreamState {
    #[default]
    Connecting,
    Live,
    /// The connection is gone; it will be tried again.
    Lost(String),
    /// The instance's stream ended (`stopped`, `down`, ...); a restart is waited for.
    Ended(String),
}

impl StreamState {
    pub fn words(&self) -> String {
        match self {
            StreamState::Connecting => "connecting…".into(),
            StreamState::Live => "live".into(),
            StreamState::Lost(why) => format!("reconnecting ({})", clip(why, 60)),
            StreamState::Ended(reason) => format!("ended: {reason}; waiting for it"),
        }
    }
}

/// One followed instance: its newest record lines (newest first) and the
/// partial text of the turn that is running.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Feed {
    pub lines: VecDeque<FeedLine>,
    pub partial: Partial,
    pub state: StreamState,
}

/// The first readable words of a record line: the first of a few common text
/// fields, one line, cut. A display aid only: the host's line is untouched.
pub fn describe(line: &serde_json::Value) -> String {
    for key in [
        "text", "content", "message", "title", "name", "reason", "why",
    ] {
        if let Some(t) = line.get(key).and_then(|v| v.as_str()) {
            let t = clip(t, 140);
            if !t.is_empty() {
                return t;
            }
        }
    }
    String::new()
}

impl Feed {
    /// Take one thing the follower reported.
    pub fn apply(&mut self, e: &Event) {
        match e {
            Event::Connected(_) => self.state = StreamState::Live,
            // the follower reports the end and then the wait: keep the end,
            // which says more than the wait
            Event::Lost { .. } if matches!(self.state, StreamState::Ended(_)) => {}
            Event::Lost { why, .. } => self.state = StreamState::Lost(why.clone()),
            Event::Frame(Frame::Record { seq, line }) => {
                self.state = StreamState::Live;
                self.lines.push_front(FeedLine {
                    seq: *seq,
                    at: line.get("at").and_then(|v| v.as_i64()),
                    kind: line
                        .get("kind")
                        .and_then(|v| v.as_str())
                        .unwrap_or("?")
                        .to_string(),
                    text: describe(line),
                });
                self.lines.truncate(FEED_CAP);
            }
            Event::Frame(Frame::End(end)) => self.state = StreamState::Ended(end.reason.clone()),
            Event::Frame(Frame::Delta(d)) => {
                if d.kind == "turn_end" {
                    // the finished turn is in the record
                    self.partial = Partial::default();
                    return;
                }
                if self.partial.turn != Some(d.turn) {
                    self.partial = Partial {
                        turn: Some(d.turn),
                        ..Partial::default()
                    };
                }
                let name = d.name.clone().unwrap_or_default();
                match d.kind.as_str() {
                    "text" => self.partial.text.push_str(d.text.as_deref().unwrap_or("")),
                    "tool_call" => self.partial.tools.push(name),
                    "tool_done" => {
                        let mark = if d.ok == Some(false) { '✗' } else { '✓' };
                        self.partial.tools.push(format!("{name} {mark}"));
                    }
                    "behind" => self.partial.gap = true,
                    _ => {}
                }
            }
            Event::Frame(Frame::Other { .. }) | Event::Frame(Frame::Open(_)) => {}
        }
    }
}

/// Which way a queue item is moved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Way {
    Up,
    Down,
}

/// What to send as `before` to move waiting item `id` one place: `Some(Some(x))`
/// puts it in front of `x`, `Some(None)` at the end, `None` means it is
/// already at that edge. Only waiting items count: one being answered is not
/// moved over. (The host refuses a move across groups, the owner's against a
/// peer's, in words; the page shows them.)
pub fn move_target(items: &[QueueItem], id: &str, way: Way) -> Option<Option<String>> {
    let waiting: Vec<&QueueItem> = items.iter().filter(|i| i.state == "waiting").collect();
    let at = waiting.iter().position(|i| i.id == id)?;
    match way {
        Way::Up => (at > 0).then(|| Some(waiting[at - 1].id.clone())),
        Way::Down => match waiting.get(at + 2) {
            Some(after) => Some(Some(after.id.clone())),
            None if at + 1 < waiting.len() => Some(None),
            None => None,
        },
    }
}

/// One queue item as a line: `[steer] text…`, and how it stands.
pub fn queue_line(i: &QueueItem) -> String {
    let kind = i.intent.as_deref().unwrap_or("next");
    let who = if i.role == "owner" {
        ""
    } else {
        " (not yours)"
    };
    let state = match (i.state.as_str(), i.turn) {
        ("answering", Some(t)) => format!(" — being answered in turn {t}"),
        ("answering", None) => " — being answered".to_string(),
        _ => String::new(),
    };
    format!("[{kind}] {}{who}{state}", clip(&i.text, 100))
}

/// A key for a write that may be retried: the same key is the same write.
pub fn fresh_key(now_ms: u128, n: u64) -> String {
    format!("ui-{now_ms}-{n}")
}

/// What a failed write says, in words for the page.
pub fn said(e: &Error) -> String {
    match e {
        Error::Status { code: 403, message } if !message.is_empty() => {
            format!("refused: {message} (turn controls on with a connection that may write)")
        }
        other => format!("did not go: {other}"),
    }
}

/// Whether the provider had a band cached.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cache {
    Cached,
    Partly,
    New,
    Unknown,
}

impl Cache {
    fn of(s: Option<&str>) -> Cache {
        match s {
            Some("cached") => Cache::Cached,
            Some("partly") => Cache::Partly,
            Some("new") => Cache::New,
            _ => Cache::Unknown,
        }
    }

    pub fn word(self) -> &'static str {
        match self {
            Cache::Cached => "cached",
            Cache::Partly => "partly cached",
            Cache::New => "new",
            Cache::Unknown => "cache unknown",
        }
    }
}

/// One band of the request as a stripe: how much of it, and its cache mark.
#[derive(Debug, Clone, PartialEq)]
pub struct Seg {
    /// The host's band name (`tools`, `system`, ...): what a selection names.
    pub band: String,
    pub label: String,
    pub tokens: u64,
    /// Its share of the request, 0 to 1.
    pub share: f32,
    pub cache: Cache,
}

/// What the context inspector shows for one call of one turn.
#[derive(Debug, Clone, PartialEq)]
pub struct Strata {
    pub title: String,
    /// One line each: the size, the provider's count and hit rate, cache events and breakpoints.
    pub facts: Vec<String>,
    pub segs: Vec<Seg>,
    /// What the record cannot give back whole, in the host's words.
    pub notes: Vec<String>,
}

/// A number with thousands separators.
pub fn grouped(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::new();
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

fn band_label(b: &Band) -> String {
    match b.band.as_str() {
        "tools" => format!("tools ({})", b.count.unwrap_or(b.items.len() as u64)),
        "history" => format!("history ({} earlier turns)", b.turns.unwrap_or(0)),
        "this_turn" => format!("this turn ({} messages)", b.messages),
        other => other.to_string(),
    }
}

/// The inspector's view of a context answer: the bands in the order the model
/// sees them, sized by the host's own estimates, marked by the host's own
/// cache marks. Nothing is recomputed here but the shares.
pub fn strata_of(v: &ContextView) -> Strata {
    let total: u64 = v.bands.iter().map(|b| b.est_tokens).sum::<u64>().max(1);
    let segs = v
        .bands
        .iter()
        .map(|b| Seg {
            band: b.band.clone(),
            label: band_label(b),
            tokens: b.est_tokens,
            share: b.est_tokens as f32 / total as f32,
            cache: Cache::of(b.cache.as_deref()),
        })
        .collect();
    let mut title = format!(
        "Turn {} · call {} of {}",
        v.turn,
        v.call.max(1),
        v.calls.max(1)
    );
    if v.live {
        title.push_str(" · the turn is running: the request as assembled");
    }
    let mut facts = vec![format!(
        "Request about {} tokens (the host's estimate)",
        grouped(v.estimate_tokens)
    )];
    if let Some(p) = &v.provider {
        let rate = (p.cached_tokens * 100)
            .checked_div(p.prompt_tokens)
            .map(|r| format!(" ({r}%)"))
            .unwrap_or_default();
        facts.push(format!(
            "The provider counted {} prompt tokens, {} cached{rate}; {} written to cache; {} generated",
            grouped(p.prompt_tokens),
            grouped(p.cached_tokens),
            grouped(p.cache_write_tokens),
            grouped(p.completion_tokens)
        ));
    }
    if let Some(e) = &v.cache_event {
        facts.push(format!(
            "{}{}",
            e.kind,
            e.cause
                .as_deref()
                .map(|c| format!(": {c}"))
                .unwrap_or_default()
        ));
    }
    if !v.boundaries.is_empty() {
        let b: Vec<String> = v
            .boundaries
            .iter()
            .map(|b| {
                format!(
                    "after {} (about {} tokens)",
                    b.after,
                    grouped(b.tokens.unwrap_or(b.est_tokens))
                )
            })
            .collect();
        facts.push(format!("Cache breakpoints {}", b.join(", ")));
    }
    let mut notes = v.problems.clone();
    if let Some(n) = &v.note {
        notes.push(if v.missing.is_empty() {
            n.clone()
        } else {
            format!("{n} (missing: {})", v.missing.join(", "))
        });
    }
    Strata {
        title,
        facts,
        segs,
        notes,
    }
}

/// What opening a band shows: its whole text, or a count.
pub fn band_preview(b: &Band) -> String {
    match b.band.as_str() {
        "tools" => {
            let names: Vec<&str> = b.items.iter().map(|t| t.name.as_str()).collect();
            format!("{} tool definitions: {}", names.len(), names.join(", "))
        }
        "history" => format!(
            "{} earlier turns of this epoch, with what the model was shown for each and the messages that followed",
            b.turns.unwrap_or(0)
        ),
        "this_turn" => format!(
            "{} model calls so far in this turn and {} messages: the earlier calls and their tool results",
            b.calls_so_far.unwrap_or(0),
            b.messages
        ),
        _ => b.text.clone().unwrap_or_default(),
    }
}

/// A band's text cut for drawing: the part shown, the whole length, and
/// whether anything is held back. Nothing is lost: expanding shows it all.
pub fn band_fold(text: &str, expanded: bool) -> (&str, usize, bool) {
    let total = text.chars().count();
    if expanded || total <= BAND_FOLD {
        return (text, total, false);
    }
    let end = text
        .char_indices()
        .nth(BAND_FOLD)
        .map_or(text.len(), |(i, _)| i);
    (&text[..end], total, true)
}

/// How many characters of a band are drawn before "Show more".
pub const BAND_FOLD: usize = 20_000;

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

    fn rec(seq: u64, line: &str) -> Event {
        Event::Frame(Frame::Record {
            seq,
            line: serde_json::from_str(line).unwrap(),
        })
    }

    fn delta(
        turn: u64,
        kind: &str,
        text: Option<&str>,
        name: Option<&str>,
        ok: Option<bool>,
    ) -> Event {
        Event::Frame(Frame::Delta(rung_client::Delta {
            turn,
            call: 0,
            kind: kind.into(),
            text: text.map(Into::into),
            name: name.map(Into::into),
            ok,
            output: None,
            skipped: None,
        }))
    }

    #[test]
    fn records_come_newest_first_with_their_words_and_the_feed_is_capped() {
        let mut f = Feed::default();
        f.apply(&rec(1, r#"{"seq":1,"at":5,"kind":"turn.started"}"#));
        f.apply(&rec(
            2,
            r#"{"seq":2,"at":6,"kind":"message","text":"hello\nsecond line"}"#,
        ));
        assert_eq!(f.state, StreamState::Live);
        let got: Vec<_> = f
            .lines
            .iter()
            .map(|l| (l.seq, l.at, l.kind.as_str(), l.text.as_str()))
            .collect();
        assert_eq!(
            got,
            [
                (2, Some(6), "message", "hello…"),
                (1, Some(5), "turn.started", "")
            ]
        );
        for n in 3..(FEED_CAP as u64 + 50) {
            f.apply(&rec(n, r#"{"kind":"k"}"#));
        }
        assert_eq!(f.lines.len(), FEED_CAP);
        assert_eq!(f.lines[0].seq, FEED_CAP as u64 + 49, "the newest is kept");
    }

    #[test]
    fn streaming_text_builds_up_then_the_finished_turn_replaces_it() {
        let mut f = Feed::default();
        f.apply(&delta(4, "text", Some("Hel"), None, None));
        f.apply(&delta(4, "text", Some("lo"), None, None));
        f.apply(&delta(4, "tool_call", None, Some("read"), None));
        f.apply(&delta(4, "tool_done", None, Some("read"), Some(true)));
        f.apply(&delta(4, "tool_done", None, Some("write"), Some(false)));
        assert_eq!(f.partial.text, "Hello");
        assert_eq!(f.partial.tools, ["read", "read ✓", "write ✗"]);
        assert_eq!(f.partial.turn, Some(4));
        // the next turn starts clean
        f.apply(&delta(5, "text", Some("B"), None, None));
        assert_eq!((f.partial.text.as_str(), f.partial.tools.len()), ("B", 0));
        // the end of the turn drops the partial: the record has the text
        f.apply(&delta(5, "turn_end", None, None, None));
        assert_eq!(f.partial, Partial::default());
    }

    #[test]
    fn a_missed_stretch_is_marked_and_the_link_states_are_worded() {
        let mut f = Feed::default();
        f.apply(&delta(1, "behind", None, None, None));
        assert!(f.partial.gap);
        f.apply(&Event::Lost {
            why: "refused".into(),
            retry_in: std::time::Duration::ZERO,
        });
        assert_eq!(f.state.words(), "reconnecting (refused)");
        f.apply(&Event::Frame(Frame::End(rung_client::End {
            reason: "down".into(),
            last_seq: Some(3),
            problem: None,
        })));
        assert_eq!(f.state.words(), "ended: down; waiting for it");
        f.apply(&Event::Lost {
            why: "the stream ended".into(),
            retry_in: std::time::Duration::ZERO,
        });
        assert_eq!(
            f.state.words(),
            "ended: down; waiting for it",
            "the end is kept over the wait"
        );
        f.apply(&Event::Connected(rung_client::Open {
            after: 3,
            live: true,
            partials: true,
        }));
        assert_eq!(f.state.words(), "live");
        assert_eq!(StreamState::Connecting.words(), "connecting…");
    }

    fn item(id: &str, state: &str) -> QueueItem {
        QueueItem {
            id: id.into(),
            text: format!("text of {id}"),
            role: "owner".into(),
            state: state.into(),
            ..QueueItem::default()
        }
    }

    #[test]
    fn a_move_names_what_to_go_in_front_of_and_stops_at_the_edges() {
        let items = [
            item("a", "answering"),
            item("b", "waiting"),
            item("c", "waiting"),
            item("d", "waiting"),
        ];
        // up: in front of the one above it among the waiting; the answered one is not jumped
        assert_eq!(move_target(&items, "c", Way::Up), Some(Some("b".into())));
        assert_eq!(
            move_target(&items, "b", Way::Up),
            None,
            "the top of the waiting"
        );
        // down: in front of the one after the next, or the end of its group
        assert_eq!(move_target(&items, "b", Way::Down), Some(Some("d".into())));
        assert_eq!(move_target(&items, "c", Way::Down), Some(None));
        assert_eq!(move_target(&items, "d", Way::Down), None, "already last");
        assert_eq!(
            move_target(&items, "a", Way::Up),
            None,
            "one being answered does not move"
        );
        assert_eq!(move_target(&items, "zzz", Way::Up), None);
    }

    #[test]
    fn a_queue_line_shows_intent_text_whose_it_is_and_the_turn() {
        let mut i = item("a", "waiting");
        i.intent = Some("steer".into());
        assert_eq!(queue_line(&i), "[steer] text of a");
        i.role = "peer".into();
        i.intent = None;
        i.state = "answering".into();
        i.turn = Some(7);
        assert_eq!(
            queue_line(&i),
            "[next] text of a (not yours) — being answered in turn 7"
        );
    }

    #[test]
    fn keys_are_distinct_per_write_and_a_refusal_is_said_in_the_gateways_words() {
        assert_ne!(fresh_key(5, 1), fresh_key(5, 2));
        assert_eq!(
            fresh_key(5, 1),
            fresh_key(5, 1),
            "the same write, the same key"
        );
        let e = Error::Status {
            code: 403,
            message: "this token can only read".into(),
        };
        assert!(
            said(&e).starts_with("refused: this token can only read"),
            "{}",
            said(&e)
        );
        let e = Error::Status {
            code: 409,
            message: "start it first".into(),
        };
        assert_eq!(
            said(&e),
            "did not go: the gateway answered 409: start it first"
        );
    }

    fn ctx(json: &str) -> ContextView {
        serde_json::from_str(json).expect("a context")
    }

    const CTX: &str = r#"{"turn":3,"call":2,"calls":3,"live":false,"recorded":true,
      "provider":{"prompt_tokens":4000,"cached_tokens":3000,"cache_write_tokens":10,"completion_tokens":40},
      "cache_event":{"kind":"cache.break","cause":"the system text changed"},
      "estimate_tokens":4010,
      "boundaries":[{"after":"system","est_tokens":2000,"tokens":1990},{"after":"epoch","est_tokens":2500}],
      "problems":["a recall is missing"],"note":"this turn predates context recording","missing":["recall"],
      "bands":[{"band":"tools","count":2,"est_tokens":1000,"cache":"cached","items":[{"name":"read"},{"name":"write"}]},
               {"band":"system","est_tokens":1000,"cache":"cached","text":"You are an agent."},
               {"band":"history","est_tokens":1500,"cache":"partly","turns":2},
               {"band":"message","est_tokens":400,"cache":"new","text":"hi"},
               {"band":"this_turn","est_tokens":110,"cache":"new","calls_so_far":1,"messages":[1,2]}]}"#;

    #[test]
    fn the_strata_are_the_bands_in_order_with_their_marks_and_shares() {
        let s = strata_of(&ctx(CTX));
        assert_eq!(s.title, "Turn 3 · call 2 of 3");
        let segs: Vec<_> = s
            .segs
            .iter()
            .map(|g| (g.label.as_str(), g.tokens, g.cache))
            .collect();
        assert_eq!(
            segs,
            [
                ("tools (2)", 1000, Cache::Cached),
                ("system", 1000, Cache::Cached),
                ("history (2 earlier turns)", 1500, Cache::Partly),
                ("message", 400, Cache::New),
                ("this turn (2 messages)", 110, Cache::New),
            ]
        );
        let sum: f32 = s.segs.iter().map(|g| g.share).sum();
        assert!((sum - 1.0).abs() < 1e-4, "{sum}");
        assert!((s.segs[0].share - 1000.0 / 4010.0).abs() < 1e-5);
        assert_eq!(s.segs[0].band, "tools");
    }

    #[test]
    fn the_facts_give_the_hosts_estimate_the_providers_count_and_the_breakpoints() {
        let s = strata_of(&ctx(CTX));
        assert_eq!(
            s.facts,
            [
                "Request about 4,010 tokens (the host's estimate)",
                "The provider counted 4,000 prompt tokens, 3,000 cached (75%); 10 written to cache; 40 generated",
                "cache.break: the system text changed",
                "Cache breakpoints after system (about 1,990 tokens), after epoch (about 2,500 tokens)",
            ]
        );
        assert_eq!(
            s.notes,
            [
                "a recall is missing",
                "this turn predates context recording (missing: recall)"
            ]
        );
    }

    #[test]
    fn a_running_turn_says_it_is_as_assembled_and_has_no_provider_line() {
        let s = strata_of(&ctx(
            r#"{"turn":7,"call":1,"calls":0,"live":true,"estimate_tokens":10,
                "bands":[{"band":"system","est_tokens":10,"cache":"unknown"}]}"#,
        ));
        assert_eq!(
            s.title,
            "Turn 7 · call 1 of 1 · the turn is running: the request as assembled"
        );
        assert_eq!(s.facts.len(), 1);
        assert_eq!(s.segs[0].cache, Cache::Unknown);
        assert_eq!(Cache::Unknown.word(), "cache unknown");
    }

    #[test]
    fn numbers_are_grouped_and_a_band_opens_to_its_text_or_a_count() {
        assert_eq!(
            [0, 7, 999, 1000, 41_880, 1_234_567].map(grouped),
            ["0", "7", "999", "1,000", "41,880", "1,234,567"]
        );
        let v = ctx(CTX);
        assert_eq!(band_preview(&v.bands[0]), "2 tool definitions: read, write");
        assert_eq!(band_preview(&v.bands[1]), "You are an agent.");
        assert!(band_preview(&v.bands[4]).starts_with("1 model calls so far"));
        let huge = "é".repeat(2_000_000);
        let long = Band {
            band: "system".into(),
            text: Some(huge.clone()),
            ..Band::default()
        };
        assert_eq!(band_preview(&long), huge);
        let (part, total, folded) = band_fold(&huge, false);
        assert!(folded);
        assert_eq!(total, 2_000_000);
        assert_eq!(part.chars().count(), BAND_FOLD);
        let (all, total, folded) = band_fold(&huge, true);
        assert!(!folded);
        assert_eq!(all, huge);
        assert_eq!(total, 2_000_000);
        assert_eq!(band_fold("short", false), ("short", 5, false));
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
