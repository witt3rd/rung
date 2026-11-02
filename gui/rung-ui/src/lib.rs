//! What the window shows, with no GUI in it: the rows of the instance list
//! and the state of the link to the gateway, built from `rung-client`'s typed
//! answers. Everything here is plain data, so it is tested without a display.
//!
//! The words are the host's own (`rung-client` reads them from the summary
//! the gateway lists); nothing here folds a record or removes a secret.

use std::collections::VecDeque;

use rung_client::{
    Asked, Band, Call, Client, ContextView, Error, Event, Frame, Instance, Instances, QueueItem,
    Role, TurnDetail, TurnRow,
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

    /// The ids in the order the list shows them (chiefs, stewards, the rest;
    /// the gateway's order within each): what the arrow keys walk.
    pub fn ordered_ids(&self) -> Vec<&str> {
        self.groups()
            .into_iter()
            .flat_map(|g| g.rows.into_iter().map(|r| r.id.as_str()))
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

/// Append an older page to the loaded turns (newest first): only turns below
/// the oldest loaded, so a repeated or stale page adds nothing twice.
pub fn merge_older(loaded: &mut Vec<TurnRow>, page: Vec<TurnRow>) {
    let mut floor = loaded.last().map_or(u64::MAX, |r| r.turn);
    for r in page {
        if r.turn < floor {
            floor = r.turn;
            loaded.push(r);
        }
    }
}

/// The first line of a turn's reply, or (no reply) what happened to it: the
/// words a list shows for a turn. The reply itself is whole in the row.
pub fn turn_headline(r: &TurnRow) -> String {
    let first = r.text.lines().map(str::trim).find(|l| !l.is_empty());
    if let Some(l) = first {
        return clip(l, 140);
    }
    match r.status.as_str() {
        "running" => "In progress".into(),
        "interrupted" => "Cut off before it finished".into(),
        _ => match failure_class(r) {
            Some(c) => format!("No reply ({c})"),
            None => "No reply".into(),
        },
    }
}

fn failure_class(r: &TurnRow) -> Option<String> {
    r.failure
        .as_ref()
        .and_then(|f| f["class"].as_str())
        .map(str::to_string)
}

/// The small print under a turn's headline: kind, model, calls, tools, asked.
pub fn turn_meta(r: &TurnRow) -> String {
    let mut parts = vec![r.status.clone()];
    if let Some(k) = &r.kind {
        parts.push(k.clone());
    }
    if let Some(p) = &r.project {
        parts.push(p.clone());
    }
    if r.llm.calls > 0 {
        parts.push(format!(
            "{} model call{}",
            r.llm.calls,
            if r.llm.calls == 1 { "" } else { "s" }
        ));
    }
    if r.tool_calls > 0 {
        parts.push(format!(
            "{} tool call{}",
            r.tool_calls,
            if r.tool_calls == 1 { "" } else { "s" }
        ));
    }
    if r.asked > 0 {
        parts.push(format!("asked {}", r.asked));
    }
    parts.join(" · ")
}

/// A turn's facts, label and value, for its page.
pub fn turn_facts(d: &TurnDetail) -> Vec<(&'static str, String)> {
    let r = &d.row;
    let mut v = vec![("Status", r.status.clone())];
    if let Some(k) = &r.kind {
        v.push(("Kind", k.clone()));
    }
    if let Some(m) = &r.model {
        v.push(("Model", m.clone()));
    }
    if let Some(t) = r.started_at {
        v.push(("Started", utc(t)));
    }
    if let Some(t) = r.ended_at {
        let took = r
            .elapsed_ms
            .map(|ms| format!(" ({ms} ms)"))
            .unwrap_or_default();
        v.push(("Ended", format!("{}{took}", utc(t))));
    }
    if let Some(f) = &r.failure {
        let class = f["class"].as_str().unwrap_or("failed");
        let msg = f["message"]
            .as_str()
            .map(|m| format!(": {m}"))
            .unwrap_or_default();
        v.push(("Failure", format!("{class}{msg}")));
    }
    if r.llm.calls > 0 {
        v.push((
            "Model use",
            format!(
                "{} calls · {} prompt tokens ({} cached) · {} generated · {}",
                r.llm.calls,
                grouped(r.llm.prompt_tokens),
                grouped(r.llm.cached_tokens),
                grouped(r.llm.completion_tokens),
                usd(r.llm.cost_usd)
            ),
        ));
    }
    v.push(("Record lines", format!("{} to {}", r.first_seq, r.last_seq)));
    v
}

/// One item the turn was asked, as one line of text (the text whole).
pub fn asked_line(a: &Asked) -> String {
    let intent = a
        .intent
        .as_deref()
        .map(|i| format!("[{i}] "))
        .unwrap_or_default();
    let during = a
        .during
        .map(|t| format!(", sent during turn {t}"))
        .unwrap_or_default();
    format!("{intent}{}{during}: {}", a.from, a.text)
}

/// One tool call as a line: its name, how it went, why it was refused.
pub fn call_line(c: &Call) -> String {
    let how = match (&c.refused, c.ok) {
        (Some(why), _) => format!("✗ refused: {why}"),
        (None, Some(true)) => "✓".to_string(),
        (None, Some(false)) => "✗ failed".to_string(),
        (None, None) => "(no result kept)".to_string(),
    };
    format!("{} {how}", c.name)
}

/// What opening a call shows: its input and its result, whole.
pub fn call_body(c: &Call) -> String {
    let mut out = String::new();
    if let Some(i) = &c.input {
        out.push_str("input: ");
        out.push_str(&serde_json::to_string_pretty(i).unwrap_or_default());
    }
    if let Some(r) = &c.result {
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        out.push_str("result: ");
        out.push_str(r);
    }
    out
}

/// Does this event say a turn began or ended (the turns list is then asked again)?
pub fn turn_changed(e: &Event) -> bool {
    matches!(e, Event::Frame(Frame::Record { line, .. })
        if matches!(line["kind"].as_str(), Some("turn.started" | "turn.ended")))
}

/// Which instance the keyboard selects next: one step down or up the list
/// from `current`, stopping at the ends (no wrap). With none selected, down
/// takes the first and up the last. `None` only when the list is empty.
pub fn step_selection(order: &[&str], current: Option<&str>, way: Way) -> Option<String> {
    if order.is_empty() {
        return None;
    }
    let at = current.and_then(|c| order.iter().position(|id| *id == c));
    let to = match (at, way) {
        (None, Way::Down) => 0,
        (None, Way::Up) => order.len() - 1,
        (Some(i), Way::Down) => (i + 1).min(order.len() - 1),
        (Some(i), Way::Up) => i.saturating_sub(1),
    };
    Some(order[to].to_string())
}

// ─── crew and receipts (on fixtures until the fleet's R2 lands) ──────────────

/// Where a delegated task stands on the ladder of the fleet props: spawned,
/// assigned, active, done, retired, with held and stalled as recoverable side
/// states. A rung this reader does not know is kept as it was written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rung {
    Spawned,
    Assigned,
    Active,
    Held,
    Stalled,
    Done,
    Retired,
    Unknown(String),
}

impl Rung {
    pub fn of(word: &str) -> Rung {
        match word {
            "spawned" => Rung::Spawned,
            "assigned" => Rung::Assigned,
            "active" => Rung::Active,
            "held" => Rung::Held,
            "stalled" => Rung::Stalled,
            "done" => Rung::Done,
            "retired" => Rung::Retired,
            other => Rung::Unknown(other.to_string()),
        }
    }

    pub fn word(&self) -> &str {
        match self {
            Rung::Spawned => "spawned",
            Rung::Assigned => "assigned",
            Rung::Active => "active",
            Rung::Held => "held",
            Rung::Stalled => "stalled",
            Rung::Done => "done",
            Rung::Retired => "retired",
            Rung::Unknown(w) => w,
        }
    }

    /// Needs the principal or the steward: a hold waits for a decision only
    /// the principal can make, a stall is escalated, never left.
    pub fn needs_you(&self) -> bool {
        matches!(self, Rung::Held | Rung::Stalled)
    }
}

/// What the task was assigned with (the brief is a typed contract).
#[derive(Debug, Clone, PartialEq, Eq, Default, serde::Deserialize)]
pub struct Brief {
    #[serde(default)]
    pub build: String,
    #[serde(default)]
    pub complete_when: String,
    #[serde(default)]
    pub delivery: String,
    #[serde(default)]
    pub autonomy: String,
}

/// A delivery receipt: the durable record is the delivery, so it names the line.
#[derive(Debug, Clone, PartialEq, Eq, serde::Deserialize)]
pub struct Receipt {
    pub correlation: String,
    #[serde(default)]
    pub what: String,
    pub line: u64,
    #[serde(default)]
    pub at: Option<i64>,
}

/// One crew member as the screen reads it. The shape is the fixture's own:
/// the host's `crew.*` kinds are not settled, so nothing here claims to be
/// their schema.
#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
pub struct Member {
    pub id: String,
    pub task: String,
    pub rung: String,
    #[serde(default)]
    pub since: Option<i64>,
    /// A hold: what it waits for.
    #[serde(default)]
    pub waiting_on: Option<String>,
    /// A stall: how long it has been silent.
    #[serde(default)]
    pub silent_for_s: Option<u64>,
    /// Done: who judged it (not its author).
    #[serde(default)]
    pub validator: Option<String>,
    /// Retired: the proof its work landed.
    #[serde(default)]
    pub landed: Option<bool>,
    #[serde(default)]
    pub brief: Brief,
    #[serde(default)]
    pub receipts: Vec<Receipt>,
}

impl Member {
    pub fn rung(&self) -> Rung {
        Rung::of(&self.rung)
    }
}

/// The crew screen's data, and the plain statement of where it came from.
#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
pub struct CrewFile {
    /// Said on the screen: this is sample data.
    pub fixture: String,
    pub steward: String,
    #[serde(default)]
    pub now: Option<i64>,
    pub crew: Vec<Member>,
}

/// The sample that ships with the app (`--crew-fixture` without a file).
pub const SAMPLE_CREW: &str = include_str!("../fixtures/crew.json");

/// Read a crew file: `Err` says why, in words, and nothing is shown.
pub fn read_crew(text: &str) -> Result<CrewFile, String> {
    serde_json::from_str(text).map_err(|e| format!("the crew file is not readable: {e}"))
}

/// One line on the ladder: how many are on each rung, those that need someone first.
pub fn crew_summary(crew: &[Member]) -> String {
    let mut by: Vec<(Rung, usize)> = Vec::new();
    for m in crew {
        match by.iter_mut().find(|(r, _)| *r == m.rung()) {
            Some((_, n)) => *n += 1,
            None => by.push((m.rung(), 1)),
        }
    }
    by.sort_by_key(|(r, _)| !r.needs_you());
    if by.is_empty() {
        return "no crew".into();
    }
    by.iter()
        .map(|(r, n)| format!("{n} {}", r.word()))
        .collect::<Vec<_>>()
        .join(" · ")
}

/// The facts of one crew member for its page: rung-specific ones first.
pub fn member_facts(m: &Member, now: Option<i64>) -> Vec<(&'static str, String)> {
    let mut v = vec![("Rung", m.rung().word().to_string())];
    if let Some(w) = &m.waiting_on {
        v.push(("Waiting on", w.clone()));
    }
    if let Some(s) = m.silent_for_s {
        v.push(("Silent for", format!("{} min", s / 60)));
    }
    if let (Some(since), Some(now)) = (m.since, now) {
        v.push((
            "On this rung",
            format!("{} min", (now - since).max(0) / 60_000),
        ));
    }
    if let Some(who) = &m.validator {
        v.push(("Judged by", format!("{who} (not its author)")));
    }
    match (m.rung(), m.landed) {
        (Rung::Retired, Some(true)) => v.push(("Landed", "yes: retired with proof".into())),
        (Rung::Done, Some(false)) => v.push((
            "Landed",
            "not yet: it cannot be retired until it has".into(),
        )),
        _ => {}
    }
    v
}

/// A receipt as a line; a correlation delivered twice shows once and says so.
pub fn receipt_lines(m: &Member) -> Vec<String> {
    let mut seen: Vec<(&str, u64)> = Vec::new();
    let mut out = Vec::new();
    for r in &m.receipts {
        let again = seen.contains(&(r.correlation.as_str(), r.line));
        seen.push((&r.correlation, r.line));
        if again {
            // idempotent by correlation id: the same line, not a second delivery
            if let Some(last) = out
                .iter_mut()
                .find(|l: &&mut String| l.starts_with(&format!("{} ", r.correlation)))
            {
                last.push_str(" (delivered again: still one line)");
            }
            continue;
        }
        out.push(format!(
            "{} · record line {} · {}",
            r.correlation, r.line, r.what
        ));
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

    fn trow(json: &str) -> TurnRow {
        serde_json::from_str(json).expect("a turn row")
    }

    #[test]
    fn an_older_page_merges_once_and_in_order() {
        let t = |n: u64| trow(&format!(r#"{{"turn":{n},"status":"completed"}}"#));
        let nums = |v: &Vec<TurnRow>| v.iter().map(|r| r.turn).collect::<Vec<_>>();
        let mut loaded = vec![t(5), t(4)];
        merge_older(&mut loaded, vec![t(3), t(2)]);
        merge_older(&mut loaded, vec![t(3), t(2)]);
        assert_eq!(nums(&loaded), [5, 4, 3, 2]);
        let mut refreshed = vec![t(9), t(8), t(7), t(6), t(5), t(4), t(3)];
        merge_older(&mut refreshed, vec![t(5), t(4), t(3), t(2)]);
        assert_eq!(nums(&refreshed), [9, 8, 7, 6, 5, 4, 3, 2]);
    }

    #[test]
    fn a_turns_headline_is_its_reply_or_what_happened_to_it() {
        assert_eq!(
            turn_headline(&trow(
                r#"{"turn":1,"status":"completed","text":"\n  first line\nsecond"}"#
            )),
            "first line"
        );
        assert_eq!(
            turn_headline(&trow(r#"{"turn":1,"status":"running","text":""}"#)),
            "In progress"
        );
        assert_eq!(
            turn_headline(&trow(r#"{"turn":1,"status":"interrupted"}"#)),
            "Cut off before it finished"
        );
        assert_eq!(
            turn_headline(&trow(
                r#"{"turn":1,"status":"failed","failure":{"class":"provider"}}"#
            )),
            "No reply (provider)"
        );
        assert_eq!(
            turn_headline(&trow(r#"{"turn":1,"status":"completed"}"#)),
            "No reply"
        );
        let long = format!(
            r#"{{"turn":1,"status":"completed","text":"{}"}}"#,
            "x".repeat(300)
        );
        assert!(
            turn_headline(&trow(&long)).ends_with('…'),
            "a headline line is shortened; the reply is whole in the row"
        );
    }

    #[test]
    fn the_small_print_names_status_kind_calls_tools_and_asked() {
        let r = trow(
            r#"{"turn":2,"status":"completed","kind":"committed","project":"notes","llm":{"calls":3},"tool_calls":2,"asked":1}"#,
        );
        assert_eq!(
            turn_meta(&r),
            "completed · committed · notes · 3 model calls · 2 tool calls · asked 1"
        );
        assert_eq!(
            turn_meta(&trow(r#"{"turn":2,"status":"running"}"#)),
            "running"
        );
    }

    const TURN: &str = r#"{"turn":4,"status":"failed","kind":"free","model":"m","started_at":1000000000000,
        "ended_at":1000000001000,"elapsed_ms":930,"failure":{"class":"provider","message":"the model failed"},
        "text":"","llm":{"calls":2,"prompt_tokens":4515,"cached_tokens":2216,"completion_tokens":80,"cost_usd":0.0042},
        "tool_calls":3,"asked":1,"first_seq":33,"last_seq":40,
        "asked_items":[{"id":"q-1","from":"owner","intent":"steer","text":"also the appendix","during":3},
                       {"id":"q-2","from":"peer","text":"hi"}],
        "calls":[{"name":"read","ok":true,"input":{"path":"a.md"},"result":"the file"},
                 {"name":"write","ok":false,"refused":"not_enabled","result":"off"},
                 {"name":"trace","ok":true},{"name":"x"},{"name":"y","ok":false,"result":"boom"}]}"#;

    #[test]
    fn a_turns_page_has_its_facts_asked_items_and_calls() {
        let d: TurnDetail = serde_json::from_str(TURN).unwrap();
        let facts = turn_facts(&d);
        assert_eq!(facts[0], ("Status", "failed".to_string()));
        assert!(facts.contains(&("Started", "2001-09-09 01:46:40 UTC".to_string())));
        assert!(facts.contains(&("Ended", "2001-09-09 01:46:41 UTC (930 ms)".to_string())));
        assert!(facts.contains(&("Failure", "provider: the model failed".to_string())));
        assert!(facts.contains(&(
            "Model use",
            "2 calls · 4,515 prompt tokens (2,216 cached) · 80 generated · $0.0042".to_string()
        )));
        assert!(facts.contains(&("Record lines", "33 to 40".to_string())));
        assert_eq!(
            asked_line(&d.asked_items[0]),
            "[steer] owner, sent during turn 3: also the appendix"
        );
        assert_eq!(asked_line(&d.asked_items[1]), "peer: hi");
        let lines: Vec<String> = d.calls.iter().map(call_line).collect();
        assert_eq!(
            lines,
            [
                "read ✓",
                "write ✗ refused: not_enabled",
                "trace ✓",
                "x (no result kept)",
                "y ✗ failed"
            ]
        );
        assert_eq!(
            call_body(&d.calls[0]),
            "input: {\n  \"path\": \"a.md\"\n}\n\nresult: the file"
        );
        assert_eq!(call_body(&d.calls[3]), "");
    }

    #[test]
    fn a_whole_reply_of_two_million_characters_is_reachable_through_the_fold() {
        let reply = "é".repeat(2_000_000);
        let mut r = trow(r#"{"turn":1,"status":"completed"}"#);
        r.text = reply.clone();
        let (part, total, folded) = band_fold(&r.text, false);
        assert!(folded && total == 2_000_000 && part.chars().count() == BAND_FOLD);
        let (all, _, folded) = band_fold(&r.text, true);
        assert!(!folded);
        assert_eq!(all, reply, "Show more reaches all of it");
    }

    #[test]
    fn a_turn_starting_or_ending_in_the_stream_says_to_ask_again() {
        let rec = |kind: &str| {
            Event::Frame(Frame::Record {
                seq: 1,
                line: serde_json::json!({"kind": kind}),
            })
        };
        assert!(turn_changed(&rec("turn.ended")) && turn_changed(&rec("turn.started")));
        assert!(!turn_changed(&rec("llm.call")));
        assert!(!turn_changed(&Event::Connected(rung_client::Open {
            after: 0,
            live: true,
            partials: true
        })));
    }

    #[test]
    fn the_keyboard_walks_the_list_in_the_order_it_is_shown_and_stops_at_the_ends() {
        let snap = snapshot_of(&list(FLEET));
        // chiefs first, then stewards, then the rest, as drawn
        let order = snap.ordered_ids();
        assert_eq!(
            order,
            [
                "chief-main",
                "chief-second",
                "steward-alpha",
                "steward-beta",
                "steward-ghost",
                "scratch"
            ]
        );
        let down = |c: Option<&str>| step_selection(&order, c, Way::Down);
        let up = |c: Option<&str>| step_selection(&order, c, Way::Up);
        assert_eq!(down(None).as_deref(), Some("chief-main"));
        assert_eq!(up(None).as_deref(), Some("scratch"));
        assert_eq!(down(Some("chief-second")).as_deref(), Some("steward-alpha"));
        assert_eq!(up(Some("steward-alpha")).as_deref(), Some("chief-second"));
        assert_eq!(
            down(Some("scratch")).as_deref(),
            Some("scratch"),
            "the end holds"
        );
        assert_eq!(
            up(Some("chief-main")).as_deref(),
            Some("chief-main"),
            "the start holds"
        );
        assert_eq!(
            down(Some("gone")).as_deref(),
            Some("chief-main"),
            "an unlisted selection starts over"
        );
        assert_eq!(step_selection(&[], None, Way::Down), None);
    }

    #[test]
    fn the_sample_crew_file_reads_and_the_ladder_is_summarised_needing_someone_first() {
        let f = read_crew(SAMPLE_CREW).expect("the shipped sample");
        assert!(f.fixture.starts_with("Sample data, not from any host"));
        assert_eq!(f.steward, "steward-alpha");
        assert_eq!(f.crew.len(), 6);
        assert_eq!(
            crew_summary(&f.crew),
            "1 held · 1 stalled · 1 active · 1 done · 1 retired · 1 assigned"
        );
        assert_eq!(crew_summary(&[]), "no crew");
        assert!(Rung::Held.needs_you() && Rung::Stalled.needs_you() && !Rung::Active.needs_you());
        assert_eq!(
            f.crew[0].brief.autonomy,
            "May read and edit inside its worktree only; may not merge."
        );
    }

    #[test]
    fn a_rung_this_reader_does_not_know_is_kept_as_written_and_a_bad_file_says_why() {
        assert_eq!(Rung::of("quarantined"), Rung::Unknown("quarantined".into()));
        assert_eq!(Rung::of("quarantined").word(), "quarantined");
        assert!(!Rung::of("quarantined").needs_you());
        let e = read_crew(r#"{"steward":"s","crew":[]}"#).unwrap_err();
        assert!(e.starts_with("the crew file is not readable"), "{e}");
    }

    #[test]
    fn a_member_shows_what_its_rung_needs_to_say() {
        let f = read_crew(SAMPLE_CREW).unwrap();
        let by = |id: &str| f.crew.iter().find(|m| m.id == id).unwrap();
        let held = member_facts(by("crew-audit-2"), f.now);
        assert!(
            held.iter()
                .any(|(k, v)| *k == "Waiting on" && v.contains("only the principal can make"))
        );
        let stalled = member_facts(by("crew-fix-3"), f.now);
        assert!(stalled.contains(&("Silent for", "90 min".to_string())));
        let done = member_facts(by("crew-docs-4"), f.now);
        assert!(done.contains(&("Judged by", "steward-alpha (not its author)".to_string())));
        assert!(
            done.iter()
                .any(|(k, v)| *k == "Landed" && v.starts_with("not yet"))
        );
        let retired = member_facts(by("crew-spike-5"), f.now);
        assert!(
            retired
                .iter()
                .any(|(k, v)| *k == "Landed" && v.starts_with("yes"))
        );
        assert!(
            member_facts(by("crew-migrate-1"), f.now)
                .contains(&("On this rung", "30 min".to_string()))
        );
    }

    #[test]
    fn a_receipt_names_its_record_line_and_a_repeat_is_one_line() {
        let f = read_crew(SAMPLE_CREW).unwrap();
        let docs = f.crew.iter().find(|m| m.id == "crew-docs-4").unwrap();
        assert_eq!(
            receipt_lines(docs),
            ["c-064 · record line 701 · completion (delivered again: still one line)"]
        );
        let new = f.crew.iter().find(|m| m.id == "crew-new-6").unwrap();
        assert!(receipt_lines(new).is_empty());
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
