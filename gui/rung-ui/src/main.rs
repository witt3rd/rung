//! `rung-ui`: a window onto the instances behind a rung gateway.
//!
//! ```text
//! rung-ui [--gateway URL] [--select ID] [--turn N] [--open N] [--controls] [--list | --show ID]
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
use rung_client::{
    Client, ContextView, Event, Follow, Intent, QueueList, TurnDetail, TurnRow, follow,
};
use rung_ui::{
    Cache, Detail, Feed, Link, Row, Snapshot, Way, as_text, asked_line, band_fold, band_preview,
    call_body, call_line, detail_text, fresh_key, look, move_target, queue_line, said, strata_of,
    turn_changed, turn_facts, turn_headline, turn_meta, utc,
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
    /// The context inspector: what was sent to the model on one call of one turn.
    ctx: Option<CtxState>,
    ctx_more: bool,
    /// A turn to open the inspector at once the instance is listed (`--turn`).
    ctx_pending: Option<u64>,
    ctx_band: Option<String>,
    /// The turns of the selected instance, newest first (as many as are loaded).
    turns: Vec<TurnRow>,
    turns_total: u64,
    turns_next: Option<u64>,
    older_pending: bool,
    turns_error: Option<String>,
    /// The turn whose page is open, and its detail (or why there is none).
    turn_open: Option<u64>,
    turn_open_pending: Option<u64>,
    turn_detail: Option<Result<TurnDetail, String>>,
    /// Show more / less of the open turn's reply, and the open tool call.
    turn_more: bool,
    turn_call: Option<usize>,
}

/// One answer of the context door, or why there is none.
struct CtxState {
    turn: u64,
    call: u64,
    result: Result<ContextView, String>,
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
        turn: Option<u64>,
        open_turn: Option<u64>,
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
            ctx: None,
            ctx_more: false,
            ctx_pending: turn,
            ctx_band: None,
            turns: Vec::new(),
            turns_total: 0,
            turns_next: None,
            older_pending: false,
            turns_error: None,
            turn_open: None,
            turn_open_pending: open_turn,
            turn_detail: None,
            turn_more: false,
            turn_call: None,
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
        if let Some(t) = self.ctx_pending
            && self.selected_row().is_some()
        {
            self.ctx_pending = None;
            self.load_context(t, None, cx);
        }
        cx.notify();
    }

    fn selected_row(&self) -> Option<&Row> {
        let id = self.selected.as_ref()?;
        self.snapshot.rows.iter().find(|r| &r.id == id)
    }

    /// Ask the host what the model was sent on `call` of `turn`, off the UI thread.
    fn load_context(&mut self, turn: u64, call: Option<u64>, cx: &mut Context<Self>) {
        let Some(id) = self.selected.clone() else {
            return;
        };
        let client = self.client.clone();
        let generation = self.generation;
        self.ctx = Some(CtxState {
            turn,
            call: call.unwrap_or(1),
            result: Err("asking the host…".into()),
        });
        self.ctx_band = None;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let got = cx
                .background_spawn(async move {
                    client.context(&id, turn, call).map_err(|e| e.to_string())
                })
                .await;
            _ = this.update(cx, |v, cx| {
                // the answer is for the instance that was open when it was asked
                if v.generation != generation {
                    return;
                }
                v.ctx = Some(CtxState {
                    turn,
                    call: call.unwrap_or(1),
                    result: got,
                });
                cx.notify();
            });
        })
        .detach();
    }

    /// Step the inspector to another turn or call of the one open.
    fn step_context(&mut self, dturn: i64, dcall: i64, cx: &mut Context<Self>) {
        let Some(c) = &self.ctx else { return };
        let (mut turn, mut call) = (c.turn as i64, c.call as i64);
        if dturn != 0 {
            turn = (turn + dturn).max(1);
            call = 1;
        } else {
            let calls = c.result.as_ref().map_or(1, |v| v.calls.max(1) as i64);
            call = (call + dcall).clamp(1, calls);
        }
        self.load_context(turn as u64, Some(call as u64), cx);
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

    /// The context inspector: the request of one call as stripes, in the order the
    /// model sees them, with the provider's counts. Reading only.
    fn context_block(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let mut block = v_flex().gap_1().child(
            div()
                .pt_2()
                .text_sm()
                .text_color(muted)
                .child("Context · what was sent to the model"),
        );
        let Some(c) = &self.ctx else {
            let turn = self.selected_row().and_then(|r| r.turn);
            return block
                .child(
                    Button::new("ctx-open")
                        .label("Show the latest turn's request")
                        .on_click(cx.listener(move |this, _, _, cx| {
                            if let Some(t) = turn {
                                this.load_context(t, None, cx);
                            }
                        })),
                )
                .into_any_element();
        };
        let nav = h_flex()
            .gap_1()
            .child(
                Button::new("t-")
                    .ghost()
                    .label("◀ turn")
                    .on_click(cx.listener(|t, _, _, cx| t.step_context(-1, 0, cx))),
            )
            .child(
                Button::new("t+")
                    .ghost()
                    .label("turn ▶")
                    .on_click(cx.listener(|t, _, _, cx| t.step_context(1, 0, cx))),
            )
            .child(
                Button::new("c-")
                    .ghost()
                    .label("◀ call")
                    .on_click(cx.listener(|t, _, _, cx| t.step_context(0, -1, cx))),
            )
            .child(
                Button::new("c+")
                    .ghost()
                    .label("call ▶")
                    .on_click(cx.listener(|t, _, _, cx| t.step_context(0, 1, cx))),
            );
        block = block.child(nav);
        let v = match &c.result {
            Ok(v) => v,
            Err(why) => {
                return block
                    .child(
                        div()
                            .text_color(theme.warning)
                            .child(format!("Turn {}: {why}", c.turn)),
                    )
                    .into_any_element();
            }
        };
        let strata = strata_of(v);
        let color = |k: Cache| match k {
            Cache::Cached => theme.success,
            Cache::Partly => theme.warning,
            Cache::New => theme.primary,
            Cache::Unknown => muted,
        };
        block = block.child(div().child(strata.title.clone()));
        // the stripes: width is the band's share of the request
        let mut bar = h_flex().h(px(18.)).w_full().gap_px();
        for g in &strata.segs {
            bar = bar.child(
                div()
                    .w(relative(g.share.max(0.01)))
                    .h_full()
                    .bg(color(g.cache)),
            );
        }
        block = block.child(bar);
        for (n, g) in strata.segs.iter().enumerate() {
            let name = g.band.clone();
            let open = self.ctx_band.as_deref() == Some(g.band.as_str());
            let mut line = h_flex()
                .id(("band", n))
                .gap_2()
                .items_baseline()
                .cursor_pointer()
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.ctx_more = false;
                    this.ctx_band = if this.ctx_band.as_deref() == Some(name.as_str()) {
                        None
                    } else {
                        Some(name.clone())
                    };
                    cx.notify();
                }))
                .child(div().w(px(10.)).h(px(10.)).flex_none().bg(color(g.cache)))
                .child(div().flex_1().min_w_0().truncate().child(g.label.clone()))
                .child(div().flex_none().text_color(muted).child(format!(
                    "{} tok · {}",
                    rung_ui::grouped(g.tokens),
                    g.cache.word()
                )));
            if open {
                line = line.bg(theme.secondary);
            }
            block = block.child(line);
        }
        if let Some(b) = self
            .ctx_band
            .as_ref()
            .and_then(|n| v.bands.iter().find(|b| &b.band == n))
        {
            let full = band_preview(b);
            let (part, total, folded) = band_fold(&full, self.ctx_more);
            block = block.child(
                div()
                    .id("band-text")
                    .max_h(px(220.))
                    .overflow_y_scroll()
                    .p_2()
                    .rounded_md()
                    .bg(theme.secondary)
                    .text_sm()
                    .child(part.to_string()),
            );
            let mut foot =
                h_flex()
                    .gap_2()
                    .items_center()
                    .child(div().text_sm().text_color(muted).child(format!(
                        "{} characters in all",
                        rung_ui::grouped(total as u64)
                    )));
            if folded || self.ctx_more {
                foot = foot.child(
                    Button::new("band-more")
                        .ghost()
                        .label(if self.ctx_more {
                            "Show less"
                        } else {
                            "Show more"
                        })
                        .on_click(cx.listener(|t, _, _, cx| {
                            t.ctx_more = !t.ctx_more;
                            cx.notify();
                        })),
                );
            }
            block = block.child(foot);
        }
        for f in &strata.facts {
            block = block.child(div().text_sm().text_color(muted).child(f.clone()));
        }
        for n in &strata.notes {
            block = block.child(div().text_sm().text_color(theme.warning).child(n.clone()));
        }
        block.into_any_element()
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
        self.ctx = None;
        self.ctx_band = None;
        self.turns.clear();
        self.turns_total = 0;
        self.turns_next = None;
        self.older_pending = false;
        self.turns_error = None;
        self.turn_open = None;
        self.turn_detail = None;
        self.turn_more = false;
        self.turn_call = None;
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
                    let mut turns_moved = false;
                    for m in batch {
                        match m {
                            Msg::Stream(e) => {
                                turns_moved |= turn_changed(&e);
                                v.feed.apply(&e)
                            }
                            Msg::Queue(Ok(q)) => v.queue = Some(q),
                            // keep the last good queue; the link's state is the header's
                            Msg::Queue(Err(_)) => {}
                        }
                    }
                    if turns_moved {
                        // a turn began or ended: ask the host for the turns again
                        v.load_turns(false, cx);
                        if let Some(n) = v.turn_open {
                            v.open_turn(n, cx);
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
        self.load_turns(false, cx);
    }

    /// Ask the host for the turns (its own fold: nothing is folded here): the
    /// newest ones, or (`older`) the page below the oldest loaded. A refresh
    /// asks for as many as are loaded, so a window that has paged back stays put.
    fn load_turns(&mut self, older: bool, cx: &mut Context<Self>) {
        let Some(id) = self.selected.clone() else {
            return;
        };
        let client = self.client.clone();
        let generation = self.generation;
        if older {
            if self.older_pending {
                return;
            }
            self.older_pending = true;
        }
        let (limit, before) = if older {
            (20, self.turns_next)
        } else {
            ((self.turns.len() as u64).max(20), None)
        };
        cx.spawn(async move |this, cx| {
            let got = cx
                .background_spawn(async move {
                    client.turns(&id, limit, before).map_err(|e| e.to_string())
                })
                .await;
            _ = this.update(cx, |v, cx| {
                if v.generation != generation {
                    return;
                }
                if older {
                    v.older_pending = false;
                }
                match got {
                    Ok(list) => {
                        v.turns_error = None;
                        v.turns_total = list.total;
                        if older {
                            v.turns_next = list.next;
                            rung_ui::merge_older(&mut v.turns, list.turns);
                        } else {
                            // a refresh that lands after an older page keeps the
                            // rows already paged in below its own window
                            let old = std::mem::replace(&mut v.turns, list.turns);
                            let fresh = v.turns.len();
                            rung_ui::merge_older(&mut v.turns, old);
                            if v.turns.len() == fresh {
                                v.turns_next = list.next;
                            }
                        }
                    }
                    Err(why) => v.turns_error = Some(why),
                }
                if let Some(n) = v.turn_open_pending.take() {
                    v.open_turn(n, cx);
                }
                cx.notify();
            });
        })
        .detach();
    }

    /// Open turn `n`'s page: its reply, what it was asked, its calls, whole.
    fn open_turn(&mut self, n: u64, cx: &mut Context<Self>) {
        let Some(id) = self.selected.clone() else {
            return;
        };
        if self.turn_open != Some(n) {
            self.turn_more = false;
            self.turn_call = None;
        }
        self.turn_open = Some(n);
        let client = self.client.clone();
        let generation = self.generation;
        cx.notify();
        cx.spawn(async move |this, cx| {
            let got = cx
                .background_spawn(async move { client.turn(&id, n).map_err(|e| e.to_string()) })
                .await;
            _ = this.update(cx, |v, cx| {
                if v.generation == generation && v.turn_open == Some(n) {
                    v.turn_detail = Some(got);
                    cx.notify();
                }
            });
        })
        .detach();
    }

    /// The turns of the selected instance, newest first; a click opens one.
    fn turns_block(&self, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let mut block = v_flex().gap_1().child(
            div()
                .pt_2()
                .text_sm()
                .text_color(muted)
                .child(format!("Turns · {} in the record", self.turns_total)),
        );
        if let Some(why) = &self.turns_error {
            block = block.child(div().text_color(theme.warning).child(why.clone()));
        }
        // the open turn's page sits right under the heading, above the list
        if let Some(n) = self.turn_open {
            block = block.child(self.turn_page(n, cx));
        }
        for (n, t) in self.turns.iter().enumerate() {
            let num = t.turn;
            let open = self.turn_open == Some(num);
            let color = match t.status.as_str() {
                "failed" => theme.danger,
                "running" => theme.primary,
                "interrupted" => theme.warning,
                _ => theme.foreground,
            };
            let mut row = v_flex()
                .id(("turn", n))
                .cursor_pointer()
                .py_1()
                .on_click(cx.listener(move |this, _, _, cx| this.open_turn(num, cx)))
                .child(
                    h_flex()
                        .gap_2()
                        .items_baseline()
                        .child(
                            div()
                                .w(px(46.))
                                .flex_none()
                                .text_color(color)
                                .child(format!("#{num}")),
                        )
                        .child(div().flex_1().min_w_0().truncate().child(turn_headline(t))),
                )
                .child(
                    div()
                        .pl(px(54.))
                        .text_sm()
                        .text_color(muted)
                        .truncate()
                        .child(turn_meta(t)),
                );
            if open {
                row = row.bg(theme.secondary);
            }
            block = block.child(row);
        }
        if self.turns_next.is_some() {
            block = block.child(
                Button::new("older-turns")
                    .ghost()
                    .label("Older turns")
                    .on_click(cx.listener(|this, _, _, cx| this.load_turns(true, cx))),
            );
        }
        block.into_any_element()
    }

    /// One turn whole: facts, the reply (folded in place, never cut), what it was
    /// asked, its tool calls (a click opens one's input and result).
    fn turn_page(&self, n: u64, cx: &mut Context<Self>) -> AnyElement {
        let theme = cx.theme().clone();
        let muted = theme.muted_foreground;
        let mut page = v_flex()
            .gap_1()
            .p_2()
            .mt_1()
            .rounded_md()
            .bg(theme.secondary)
            .child(div().text_lg().child(format!("Turn {n}")));
        let d = match &self.turn_detail {
            Some(Ok(d)) if d.row.turn == n => d,
            Some(Err(why)) => {
                return page
                    .child(div().text_color(theme.warning).child(why.clone()))
                    .into_any_element();
            }
            _ => {
                return page
                    .child(div().text_color(muted).child("asking the host…"))
                    .into_any_element();
            }
        };
        if !d.asked_items.is_empty() {
            page = page.child(div().pt_2().text_sm().text_color(muted).child("Asked"));
            for a in &d.asked_items {
                page = page.child(div().child(asked_line(a)));
            }
        }
        // the reply, whole: drawn in part past a limit, with Show more / Show less
        page = page.child(div().pt_2().text_sm().text_color(muted).child("Reply"));
        if d.row.text.is_empty() {
            page = page.child(div().text_color(muted).child(turn_headline(&d.row)));
        } else {
            let (part, total, folded) = band_fold(&d.row.text, self.turn_more);
            page = page.child(div().child(part.to_string()));
            if folded || self.turn_more {
                page = page.child(
                    h_flex()
                        .gap_2()
                        .items_center()
                        .child(div().text_sm().text_color(muted).child(format!(
                            "{} characters in all",
                            rung_ui::grouped(total as u64)
                        )))
                        .child(
                            Button::new("reply-more")
                                .ghost()
                                .label(if self.turn_more {
                                    "Show less"
                                } else {
                                    "Show more"
                                })
                                .on_click(cx.listener(|this, _, _, cx| {
                                    this.turn_more = !this.turn_more;
                                    cx.notify();
                                })),
                        ),
                );
            }
        }
        if !d.calls.is_empty() {
            page = page.child(div().pt_2().text_sm().text_color(muted).child("Tool calls"));
            for (i, c) in d.calls.iter().enumerate() {
                let open = self.turn_call == Some(i);
                let mut line = v_flex()
                    .id(("call", i))
                    .cursor_pointer()
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.turn_call = if this.turn_call == Some(i) {
                            None
                        } else {
                            Some(i)
                        };
                        cx.notify();
                    }))
                    .child(div().child(call_line(c)));
                if open {
                    let body = call_body(c);
                    // whole input and result, in a box that scrolls
                    line = line.child(
                        div()
                            .id(("call-body", i))
                            .max_h(px(220.))
                            .overflow_y_scroll()
                            .p_2()
                            .rounded_md()
                            .bg(theme.background)
                            .text_sm()
                            .child(if body.is_empty() {
                                "the record kept no input or result for this call".to_string()
                            } else {
                                body
                            }),
                    );
                }
                page = page.child(line);
            }
        }
        page = page.child(div().pt_2().text_sm().text_color(muted).child("Facts"));
        for (k, v) in turn_facts(d) {
            page = page.child(
                h_flex()
                    .gap_2()
                    .items_start()
                    .child(div().w(px(100.)).flex_none().text_color(muted).child(k))
                    .child(div().flex_1().min_w_0().child(v)),
            );
        }
        page.into_any_element()
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
        let context_block = self.context_block(cx);
        let turns_block = self.turns_block(cx);
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
                    context_block,
                    turns_block,
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
    context_block: AnyElement,
    turns_block: AnyElement,
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
    let mut context_block = Some(context_block);
    let mut turns_block = Some(turns_block);
    for (n, sec) in detail.sections.iter().enumerate() {
        if n == 1 {
            // the queue and the followed record sit right under Now
            col = col
                .children(queue_block.take())
                .child(context_block.take().expect("once"))
                .children(turns_block.take())
                .child(record(feed, theme));
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
        col = col
            .children(queue_block.take())
            .children(context_block.take())
            .children(turns_block.take())
            .child(record(feed, theme));
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
    turn: Option<u64>,
    open: Option<u64>,
    list: bool,
    show: Option<String>,
}

/// `--gateway URL` and `--list`; anything else is a usage error.
fn args() -> Result<Opts, String> {
    let mut o = Opts {
        gateway: std::env::var("RUNG_GATEWAY").unwrap_or_else(|_| DEFAULT_GATEWAY.into()),
        select: None,
        controls: false,
        turn: None,
        open: None,
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
            "--open" => {
                o.open = Some(
                    it.next()
                        .and_then(|n| n.parse().ok())
                        .ok_or("--open needs a turn number")?,
                )
            }
            "--turn" => {
                o.turn = Some(
                    it.next()
                        .and_then(|n| n.parse().ok())
                        .ok_or("--turn needs a turn number")?,
                )
            }
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
                "rung-ui: {e}\nusage: rung-ui [--gateway URL] [--select ID] [--turn N] [--open N] [--controls] [--list | --show ID]"
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
            cx.new(|cx| {
                InstanceList::new(
                    client,
                    opts.select,
                    opts.controls,
                    opts.turn,
                    opts.open,
                    window,
                    cx,
                )
            })
        })
        .expect("the window opens");
        cx.activate(true);
    });
}
