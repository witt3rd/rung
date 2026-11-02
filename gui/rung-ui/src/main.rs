//! `rung-ui`: a window onto the instances behind a rung gateway.
//!
//! ```text
//! rung-ui [--gateway URL] [--select ID] [--controls] [--list | --show ID]
//! ```
//!
//! The gateway is `--gateway`, else `$RUNG_GATEWAY`, else
//! `http://127.0.0.1:8787`. A read-only token, when the gateway wants one, is
//! read from `$RUNG_GATEWAY_TOKEN` and never shown. `--list` prints the list
//! once as text and exits; `--show ID` prints one instance's page the same way
//! (neither needs a display). In the window a click on an instance opens its
//! page; `--select ID` opens with one already chosen. The window starts
//! read-only: its owner controls (send, reorder, cancel, stop, release) are off
//! until "Turn controls on" is clicked, or `--controls` says so at the start;
//! writes then go to the gateway with no token (the owner role), and a
//! read-only connection is refused by the gateway in words.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::*;
use rung_client::{Client, Event, Follow, Intent, QueueList, follow};
use rung_ui::{
    Detail, Feed, Link, Row, Snapshot, Way, as_text, detail_text, fresh_key, look, move_target,
    queue_line, said, utc,
};

const DEFAULT_GATEWAY: &str = "http://127.0.0.1:8787";
const EVERY: Duration = Duration::from_secs(3);

/// What a look at the gateway sent back to the window.
type Look = Result<Snapshot, String>;

struct InstanceList {
    gateway: String,
    link: Link,
    snapshot: Snapshot,
    /// The instance whose page is open (an id).
    selected: Option<String>,
    client: Client,
    /// The followed record of the selected instance.
    feed: Feed,
    /// The id the running stream follows, and the flag that stops it.
    following: Option<(String, Arc<AtomicBool>)>,
    /// Which stream the window listens to (older ones are ignored).
    generation: u64,
    /// The selected instance's queue (read; refreshed every few seconds).
    queue: Option<QueueList>,
    /// The owner's controls are off until a click turns them on; then writes
    /// go through this client, which sends no token (the owner role).
    writer: Option<Client>,
    /// The message to send, what it asks, what the last write said.
    input: Entity<InputState>,
    intent: Intent,
    said: Option<String>,
    /// A stop or release waits for a second click.
    confirm: Option<Confirm>,
    writes: u64,
    _subscription: Subscription,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Confirm {
    Stop,
    Release,
}

/// What the workers of the selected instance send to the window.
enum Msg {
    Stream(Event),
    Queue(Result<QueueList, String>),
}

/// How many lines back from the newest the page starts its record.
const BACKFILL: u64 = 100;

impl InstanceList {
    fn new(
        client: Client,
        selected: Option<String>,
        controls: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let gateway = client.base().to_string();
        let own = client.clone();
        // the owner role sends no token
        let writer = controls.then(|| Client::new(client.base()).ok()).flatten();
        let input = cx
            .new(|cx| InputState::new(window, cx).placeholder("Message to send to this instance…"));
        let subscription = cx.subscribe_in(&input, window, |this, _, event, window, cx| {
            if let InputEvent::PressEnter {
                secondary: false, ..
            } = event
            {
                this.send(window, cx);
            }
        });
        // A worker thread looks every few seconds and hands the result over;
        // the window never waits on the network. It stops when the window's
        // end of the channel is gone.
        let (tx, rx) = async_channel::unbounded::<Look>();
        std::thread::spawn(move || {
            loop {
                if tx.send_blocking(look(&client)).is_err() {
                    return;
                }
                std::thread::sleep(EVERY);
            }
        });
        cx.spawn(async move |this, cx| {
            while let Ok(result) = rx.recv().await {
                if this.update(cx, |v, cx| v.on_look(result, cx)).is_err() {
                    return;
                }
            }
        })
        .detach();
        let this = InstanceList {
            gateway,
            link: Link::Connecting,
            snapshot: Snapshot::default(),
            selected,
            client: own,
            feed: Feed::default(),
            following: None,
            generation: 0,
            queue: None,
            writer,
            input,
            intent: Intent::Next,
            said: None,
            confirm: None,
            writes: 0,
            _subscription: subscription,
        };
        if this.writer.is_some() {
            this.input.update(cx, |s, cx| s.focus(window, cx));
        }
        this
    }

    fn on_look(&mut self, result: Look, cx: &mut Context<Self>) {
        match result {
            Ok(snap) => {
                self.link = Link::Up;
                self.snapshot = snap;
            }
            // keep the last good rows on screen, and say the link is down
            Err(why) => self.link = Link::Down(why),
        }
        self.ensure_stream(cx);
        cx.notify();
    }

    /// Turn the owner's controls on (a click) or off.
    fn toggle_controls(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.writer = match self.writer {
            Some(_) => None,
            // the owner role sends no token
            None => Client::new(self.client.base()).ok(),
        };
        self.confirm = None;
        self.said = None;
        if self.writer.is_some() {
            // the box is ready to type in
            self.input.update(cx, |s, cx| s.focus(window, cx));
        }
        cx.notify();
    }

    /// Run one write off the UI thread; say what happened; look at the queue again.
    fn write(
        &mut self,
        what: &'static str,
        cx: &mut Context<Self>,
        op: impl FnOnce(Client, String) -> Result<(), rung_client::Error> + Send + 'static,
    ) {
        let (Some(writer), Some(id)) = (self.writer.clone(), self.selected.clone()) else {
            return;
        };
        self.said = Some(format!("{what}…"));
        cx.notify();
        let reader = self.client.clone();
        cx.spawn(async move |this, cx| {
            let qid = id.clone();
            let out = cx
                .background_spawn(async move {
                    let r = op(writer, id);
                    (r, reader.queue(&qid, None).ok())
                })
                .await;
            _ = this.update(cx, |v, cx| {
                v.said = Some(match &out.0 {
                    Ok(()) => format!("{what}: done"),
                    Err(e) => said(e),
                });
                if let Some(q) = out.1 {
                    v.queue = Some(q);
                }
                cx.notify();
            });
        })
        .detach();
    }

    fn next_key(&mut self) -> String {
        self.writes += 1;
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis());
        fresh_key(now, self.writes)
    }

    /// Send the typed message with the chosen intent.
    fn send(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.writer.is_none() {
            return;
        }
        let text = self.input.read(cx).value().to_string();
        if text.trim().is_empty() {
            return;
        }
        let (key, intent) = (self.next_key(), self.intent);
        self.input.update(cx, |s, cx| s.set_value("", window, cx));
        self.write("Send", cx, move |c, id| {
            c.queue_add(&id, &text, intent, &key).map(|_| ())
        });
    }

    fn cancel_item(&mut self, item: String, cx: &mut Context<Self>) {
        self.write("Cancel", cx, move |c, id| {
            c.queue_cancel(&id, &item).map(|_| ())
        });
    }

    fn move_item(&mut self, item: String, way: Way, cx: &mut Context<Self>) {
        let Some(q) = &self.queue else { return };
        let Some(before) = move_target(&q.items, &item, way) else {
            self.said = Some("It is already at that end.".into());
            cx.notify();
            return;
        };
        let key = self.next_key();
        self.write("Move", cx, move |c, id| {
            c.queue_move(&id, &item, before.as_deref(), &key)
                .map(|_| ())
        });
    }

    /// A stop or a release asks twice: the first click arms, the second goes.
    fn ask(&mut self, what: Confirm, cx: &mut Context<Self>) {
        if self.confirm == Some(what) {
            self.confirm = None;
            match what {
                Confirm::Stop => self.write("Stop", cx, |c, id| c.stop(&id).map(|_| ())),
                Confirm::Release => {
                    self.write("Release", cx, |c, id| c.release(&id, None).map(|_| ()))
                }
            }
        } else {
            self.confirm = Some(what);
            cx.notify();
        }
    }

    /// The selected instance's queue, and (with controls on) what can be done to it.
    fn queue_block(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let armed = self.writer.is_some();
        let mut block = v_flex()
            .gap_1()
            .child(
                div()
                    .pt_2()
                    .text_sm()
                    .text_color(muted)
                    .child(match &self.queue {
                        Some(q) => format!(
                            "Queue · {} waiting, {} being answered",
                            q.counts.waiting, q.counts.answering
                        ),
                        None => "Queue".into(),
                    }),
            );
        let items: Vec<&rung_client::QueueItem> = self
            .queue
            .as_ref()
            .map(|q| q.items.iter().collect())
            .unwrap_or_default();
        if items.is_empty() && self.queue.is_some() {
            block = block.child(div().text_color(muted).child("Nothing is waiting."));
        }
        for (n, it) in items.iter().enumerate() {
            let mut line = h_flex()
                .gap_2()
                .items_start()
                .child(div().flex_1().min_w_0().child(queue_line(it)));
            if armed && it.state == "waiting" {
                let (a, b, c) = (it.id.clone(), it.id.clone(), it.id.clone());
                line = line
                    .child(Button::new(("up", n)).ghost().label("↑").on_click(
                        cx.listener(move |this, _, _, cx| this.move_item(a.clone(), Way::Up, cx)),
                    ))
                    .child(Button::new(("down", n)).ghost().label("↓").on_click(
                        cx.listener(move |this, _, _, cx| this.move_item(b.clone(), Way::Down, cx)),
                    ))
                    .child(Button::new(("cancel", n)).ghost().label("Cancel").on_click(
                        cx.listener(move |this, _, _, cx| this.cancel_item(c.clone(), cx)),
                    ));
            }
            block = block.child(line);
        }

        if !armed {
            return block
                .child(
                    div()
                        .pt_1()
                        .text_sm()
                        .text_color(muted)
                        .child("Controls are off. Turn them on (top left) to send, reorder, cancel, stop or release."),
                )
                .into_any_element();
        }
        // sending: what it asks, the box, the button
        let pick = |intent: Intent, label: &'static str, cx: &mut Context<Self>| {
            let b = Button::new(label)
                .label(label)
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.intent = intent;
                    cx.notify();
                }));
            if self.intent == intent {
                b.primary()
            } else {
                b.ghost()
            }
        };
        let next = pick(Intent::Next, "next", cx);
        let steer = pick(Intent::Steer, "steer", cx);
        let free = pick(Intent::Free, "free", cx);
        block = block
            .child(
                div()
                    .pt_2()
                    .text_sm()
                    .text_color(muted)
                    .child("Send to this instance"),
            )
            .child(Input::new(&self.input))
            .child(
                h_flex().gap_1().child(next).child(steer).child(free).child(
                    Button::new("send")
                        .primary()
                        .label("Send")
                        .on_click(cx.listener(|this, _, window, cx| this.send(window, cx))),
                ),
            );
        // stop and release ask twice
        let ask_button = |id: &'static str,
                          what: Confirm,
                          idle: &'static str,
                          sure: &'static str,
                          cx: &mut Context<Self>| {
            let label = if self.confirm == Some(what) {
                sure
            } else {
                idle
            };
            let b = Button::new(id)
                .label(label)
                .on_click(cx.listener(move |this, _, _, cx| this.ask(what, cx)));
            if self.confirm == Some(what) {
                b.primary()
            } else {
                b.ghost()
            }
        };
        let stop = ask_button("stop", Confirm::Stop, "Stop…", "Click again to stop it", cx);
        let release = ask_button(
            "release",
            Confirm::Release,
            "Release…",
            "Click again to release",
            cx,
        );
        block = block.child(h_flex().gap_1().pt_1().child(stop).child(release));
        if let Some(t) = &self.said {
            block = block.child(div().text_color(theme.warning).child(t.clone()));
        }
        block.into_any_element()
    }

    /// Select an instance: its page opens and its record is followed.
    fn select(&mut self, id: String, cx: &mut Context<Self>) {
        self.selected = Some(id);
        self.ensure_stream(cx);
        cx.notify();
    }

    /// Follow the selected instance's record. The stream starts a little
    /// behind its newest line (not at the first: a long record is not
    /// replayed), is resumed by the client after a drop, and is stopped when
    /// another instance is chosen.
    fn ensure_stream(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.selected.clone() else {
            return;
        };
        if self.following.as_ref().is_some_and(|(f, _)| *f == id) {
            return;
        }
        // wait for the first look: it says where the record is
        let Some(row) = self.snapshot.rows.iter().find(|r| r.id == id) else {
            return;
        };
        let after = row.last_seq.map(|n| n.saturating_sub(BACKFILL));
        if let Some((_, stop)) = self.following.take() {
            stop.store(true, Ordering::Relaxed);
        }
        self.generation += 1;
        let generation = self.generation;
        self.feed = Feed::default();
        self.queue = None;
        self.said = None;
        self.confirm = None;
        let stop = Arc::new(AtomicBool::new(false));
        self.following = Some((id.clone(), stop.clone()));

        let (tx, rx) = async_channel::unbounded::<Msg>();
        let client = self.client.clone();
        // the record, followed (and resumed by the client after a drop)
        let (stream_tx, stream_stop, stream_id, stream_client) =
            (tx.clone(), stop.clone(), id.clone(), client.clone());
        std::thread::spawn(move || {
            follow(
                &stream_client,
                &stream_id,
                after,
                Follow::default(),
                &stream_stop,
                |e| stream_tx.send_blocking(Msg::Stream(e)).is_ok(),
            );
        });
        // the queue, looked at every few seconds
        std::thread::spawn(move || {
            while !stop.load(Ordering::Relaxed) {
                let q = client.queue(&id, None).map_err(|e| e.to_string());
                if tx.send_blocking(Msg::Queue(q)).is_err() {
                    return;
                }
                std::thread::sleep(EVERY);
            }
        });
        cx.spawn(async move |this, cx| {
            while let Ok(first) = rx.recv().await {
                let mut batch = vec![first];
                while let Ok(e) = rx.try_recv() {
                    batch.push(e);
                }
                let live = this.update(cx, |v, cx| {
                    if v.generation != generation {
                        return false;
                    }
                    for m in batch {
                        match m {
                            Msg::Stream(e) => v.feed.apply(&e),
                            Msg::Queue(Ok(q)) => v.queue = Some(q),
                            // keep the last good queue; the link's state is the header's
                            Msg::Queue(Err(_)) => {}
                        }
                    }
                    cx.notify();
                    true
                });
                if !matches!(live, Ok(true)) {
                    return; // the window is gone, or another stream took over
                }
            }
        })
        .detach();
    }
}

impl Render for InstanceList {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let down = matches!(self.link, Link::Down(_));
        let counts = self.snapshot.counts();

        // chiefs, then stewards, then the rest: a heading and its rows
        let mut sections: Vec<AnyElement> = Vec::new();
        for g in self.snapshot.groups() {
            sections.push(
                div()
                    .pt_3()
                    .pb_1()
                    .text_sm()
                    .text_color(muted)
                    .child(format!("{} ({})", g.role.plural(), g.rows.len()))
                    .into_any_element(),
            );
            for r in g.rows {
                let id = r.id.clone();
                let selected = self.selected.as_deref() == Some(r.id.as_str());
                sections.push(row(
                    r,
                    &theme,
                    selected,
                    cx.listener(move |this, _, _, cx| this.select(id.clone(), cx)),
                ));
            }
        }
        let rows = sections;

        let empty = (rows.is_empty() && !down && self.link == Link::Up).then(|| {
            div()
                .text_color(muted)
                .child("No instances are registered with this gateway.")
        });

        let queue_block = self.queue_block(cx);
        let list = v_flex()
            .flex_1()
            .min_w_0()
            .overflow_hidden()
            .h_full()
            .p_4()
            .gap_3()
            .child(
                h_flex()
                    .gap_3()
                    .items_baseline()
                    .overflow_hidden()
                    .child(div().text_xl().child("rung"))
                    .child(
                        div()
                            .text_color(if counts.needs_you > 0 {
                                theme.danger
                            } else {
                                muted
                            })
                            .child(counts.words()),
                    ),
            )
            .child(
                div()
                    .text_color(if down { theme.danger } else { muted })
                    .child(format!("{} · {}", self.gateway, self.link.words())),
            )
            .child(
                h_flex().gap_2().items_center().child(
                    Button::new("controls")
                        .label(if self.writer.is_some() {
                            "Controls on — click to turn off"
                        } else {
                            "Turn controls on"
                        })
                        .on_click(
                            cx.listener(|this, _, window, cx| this.toggle_controls(window, cx)),
                        ),
                ),
            )
            .children(self.snapshot.note.clone().map(|n| {
                div()
                    .text_color(theme.danger)
                    .child(format!("The gateway says: {n}"))
            }))
            .child(
                v_flex()
                    .id("instances")
                    .flex_1()
                    .overflow_y_scroll()
                    .children(rows),
            )
            .children(empty)
            .child(div().text_color(muted).text_sm().child("read-only view"));

        h_flex()
            .size_full()
            .bg(theme.background)
            .text_color(theme.foreground)
            .child(list)
            .children(self.selected.as_ref().map(|id| {
                page(
                    id,
                    self.snapshot.rows.iter().find(|r| &r.id == id),
                    &self.feed,
                    queue_block,
                    &theme,
                )
            }))
    }
}

/// One instance's line: name, state word with its mark, what it is doing.
/// `●` (red) needs you; `○` (amber) the gateway cannot read it.
fn row(
    r: &Row,
    theme: &gpui_kit::component::Theme,
    selected: bool,
    on_click: impl Fn(&ClickEvent, &mut Window, &mut App) + 'static,
) -> AnyElement {
    let (state, color) = if r.needs_you {
        (format!("● {}", r.state), theme.danger)
    } else if r.unreachable {
        (format!("○ {}", r.state), theme.warning)
    } else {
        (r.state.clone(), theme.foreground)
    };
    let mut line = h_flex()
        .id(SharedString::from(format!("row-{}", r.id)))
        .on_click(on_click)
        .cursor_pointer();
    if selected {
        line = line.bg(theme.secondary);
    }
    line.gap_4()
        .py_1()
        .border_b_1()
        .border_color(theme.border)
        .child(div().w(px(170.)).flex_none().pl_3().child(r.name.clone()))
        .child(div().w(px(120.)).flex_none().text_color(color).child(state))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_color(theme.muted_foreground)
                .child(r.doing.clone()),
        )
        .into_any_element()
}

/// One instance's page, beside the list: its name, then each section of the
/// host's summary as label and value.
fn page(
    id: &str,
    row: Option<&Row>,
    feed: &Feed,
    queue_block: AnyElement,
    theme: &gpui_kit::component::Theme,
) -> AnyElement {
    let muted = theme.muted_foreground;
    let mut col = v_flex()
        .id("page")
        .w(px(340.))
        .flex_none()
        .h_full()
        .overflow_y_scroll()
        .p_4()
        .gap_3()
        .border_l_1()
        .border_color(theme.border);
    let Some(row) = row else {
        return col
            .child(div().text_lg().child(id.to_string()))
            .child(
                div()
                    .text_color(muted)
                    .child("No longer listed by the gateway."),
            )
            .into_any_element();
    };
    let detail: &Detail = &row.detail;
    col = col.child(div().text_lg().child(row.name.clone()));
    let mut queue_block = Some(queue_block);
    for (n, sec) in detail.sections.iter().enumerate() {
        if n == 1 {
            // the queue and the followed record sit right under Now
            col = col.children(queue_block.take()).child(record(feed, theme));
        }
        let mut block = v_flex().gap_1().child(
            div()
                .pt_2()
                .text_sm()
                .text_color(muted)
                .child(sec.heading.to_string()),
        );
        for (label, value) in &sec.lines {
            block = block.child(
                h_flex()
                    .gap_3()
                    .items_start()
                    .child(
                        div()
                            .w(px(120.))
                            .flex_none()
                            .text_color(muted)
                            .child(label.to_string()),
                    )
                    .child(div().flex_1().min_w_0().child(value.clone())),
            );
        }
        col = col.child(block);
    }
    if detail.sections.len() < 2 {
        col = col.children(queue_block.take()).child(record(feed, theme));
    }
    col.into_any_element()
}

/// The followed record: the stream's state, the running turn's text as it
/// arrives, then the newest record lines, newest first.
fn record(feed: &Feed, theme: &gpui_kit::component::Theme) -> AnyElement {
    let muted = theme.muted_foreground;
    let mut block = v_flex().gap_1().child(
        div()
            .pt_2()
            .text_sm()
            .text_color(muted)
            .child(format!("Record · {}", feed.state.words())),
    );
    let p = &feed.partial;
    if !p.text.is_empty() || !p.tools.is_empty() || p.gap {
        let mut live = v_flex()
            .gap_1()
            .p_2()
            .rounded_md()
            .bg(theme.secondary)
            .child(
                div()
                    .text_sm()
                    .text_color(muted)
                    .child(format!("Turn {} as it is written", p.turn.unwrap_or(0))),
            );
        if !p.text.is_empty() {
            // the tail: a long answer shows its newest words
            let n = p.text.chars().count();
            let tail: String = p.text.chars().skip(n.saturating_sub(600)).collect();
            live = live.child(div().child(tail));
        }
        if !p.tools.is_empty() {
            live = live.child(
                div()
                    .text_sm()
                    .text_color(muted)
                    .child(format!("tools: {}", p.tools.join(" → "))),
            );
        }
        if p.gap {
            live = live.child(
                div()
                    .text_sm()
                    .text_color(theme.warning)
                    .child("some live text was missed; the record has it"),
            );
        }
        block = block.child(live);
    }
    for l in feed.lines.iter().take(60) {
        let when = l.at.map(|t| utc(t)[11..19].to_string()).unwrap_or_default();
        block = block.child(
            h_flex()
                .gap_2()
                .items_baseline()
                .child(
                    div()
                        .w(px(46.))
                        .flex_none()
                        .text_color(muted)
                        .child(l.seq.to_string()),
                )
                .child(div().w(px(84.)).flex_none().text_color(muted).child(when))
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .truncate()
                        .child(if l.text.is_empty() {
                            l.kind.clone()
                        } else {
                            format!("{} · {}", l.kind, l.text)
                        }),
                ),
        );
    }
    block.into_any_element()
}

/// The command line.
struct Opts {
    gateway: String,
    select: Option<String>,
    controls: bool,
    list: bool,
    show: Option<String>,
}

/// `--gateway URL` and `--list`; anything else is a usage error.
fn args() -> Result<Opts, String> {
    let mut o = Opts {
        gateway: std::env::var("RUNG_GATEWAY").unwrap_or_else(|_| DEFAULT_GATEWAY.into()),
        select: None,
        controls: false,
        list: false,
        show: None,
    };
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--gateway" => o.gateway = it.next().ok_or("--gateway needs an address")?,
            "--select" => o.select = Some(it.next().ok_or("--select needs an instance id")?),
            "--show" => o.show = Some(it.next().ok_or("--show needs an instance id")?),
            "--controls" => o.controls = true,
            "--list" => o.list = true,
            other => return Err(format!("unknown argument {other:?}")),
        }
    }
    Ok(o)
}

fn main() {
    let opts = match args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!(
                "rung-ui: {e}\nusage: rung-ui [--gateway URL] [--select ID] [--controls] [--list | --show ID]"
            );
            std::process::exit(2);
        }
    };
    let mut client = match Client::new(&opts.gateway) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("rung-ui: {e}");
            std::process::exit(2);
        }
    };
    if let Ok(token) = std::env::var("RUNG_GATEWAY_TOKEN") {
        client = client.with_token(token);
    }

    if let Some(id) = &opts.show {
        match look(&client) {
            Ok(snap) => match snap.rows.iter().find(|r| &r.id == id) {
                Some(row) => print!("{}", detail_text(row)),
                None => {
                    eprintln!("rung-ui: the gateway lists no instance {id:?}");
                    std::process::exit(1);
                }
            },
            Err(e) => {
                eprintln!("rung-ui: {e}");
                std::process::exit(1);
            }
        }
        return;
    }

    if opts.list {
        match look(&client) {
            Ok(snap) => print!("{}", as_text(&snap)),
            Err(e) => {
                eprintln!("rung-ui: {e}");
                std::process::exit(1);
            }
        }
        return;
    }

    application().with_assets(assets::Assets).run(move |cx| {
        init(cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                None,
                size(px(1200.), px(640.)),
                cx,
            ))),
            ..Default::default()
        };
        open_window(options, cx, |window, cx| {
            cx.new(|cx| InstanceList::new(client, opts.select, opts.controls, window, cx))
        })
        .expect("the window opens");
        cx.activate(true);
    });
}
