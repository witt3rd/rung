//! `rung-ui`: a window onto the instances behind a rung gateway.
//!
//! ```text
//! rung-ui [--gateway URL] [--list]
//! ```
//!
//! The gateway is `--gateway`, else `$RUNG_GATEWAY`, else
//! `http://127.0.0.1:8787`. A read-only token, when the gateway wants one, is
//! read from `$RUNG_GATEWAY_TOKEN` and never shown. `--list` prints the list
//! once as text and exits (no window, no display needed). The window only
//! reads: nothing in it writes to a host.

use std::time::Duration;

use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::*;
use rung_client::Client;
use rung_ui::{Link, Row, Snapshot, as_text, look};

const DEFAULT_GATEWAY: &str = "http://127.0.0.1:8787";
const EVERY: Duration = Duration::from_secs(3);

/// What a look at the gateway sent back to the window.
type Look = Result<Snapshot, String>;

struct InstanceList {
    gateway: String,
    link: Link,
    snapshot: Snapshot,
}

impl InstanceList {
    fn new(client: Client, cx: &mut Context<Self>) -> Self {
        let gateway = client.base().to_string();
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
        InstanceList {
            gateway,
            link: Link::Connecting,
            snapshot: Snapshot::default(),
        }
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
        cx.notify();
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
                sections.push(row(r, &theme));
            }
        }
        let rows = sections;

        let empty = (rows.is_empty() && !down && self.link == Link::Up).then(|| {
            div()
                .text_color(muted)
                .child("No instances are registered with this gateway.")
        });

        v_flex()
            .size_full()
            .p_4()
            .gap_3()
            .bg(theme.background)
            .text_color(theme.foreground)
            .child(
                h_flex()
                    .gap_3()
                    .items_baseline()
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
            .child(div().text_color(muted).text_sm().child("read-only view"))
    }
}

/// One instance's line: name, state word with its mark, what it is doing.
/// `●` (red) needs you; `○` (amber) the gateway cannot read it.
fn row(r: &Row, theme: &gpui_kit::component::Theme) -> AnyElement {
    let (state, color) = if r.needs_you {
        (format!("● {}", r.state), theme.danger)
    } else if r.unreachable {
        (format!("○ {}", r.state), theme.warning)
    } else {
        (r.state.clone(), theme.foreground)
    };
    h_flex()
        .gap_4()
        .py_1()
        .border_b_1()
        .border_color(theme.border)
        .child(div().w(px(220.)).flex_none().pl_3().child(r.name.clone()))
        .child(div().w(px(140.)).flex_none().text_color(color).child(state))
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

/// `--gateway URL` and `--list`; anything else is a usage error.
fn args() -> Result<(String, bool), String> {
    let mut gateway = std::env::var("RUNG_GATEWAY").unwrap_or_else(|_| DEFAULT_GATEWAY.into());
    let mut list = false;
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--gateway" => gateway = it.next().ok_or("--gateway needs an address")?,
            "--list" => list = true,
            other => return Err(format!("unknown argument {other:?}")),
        }
    }
    Ok((gateway, list))
}

fn main() {
    let (gateway, list) = match args() {
        Ok(a) => a,
        Err(e) => {
            eprintln!("rung-ui: {e}\nusage: rung-ui [--gateway URL] [--list]");
            std::process::exit(2);
        }
    };
    let mut client = match Client::new(&gateway) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("rung-ui: {e}");
            std::process::exit(2);
        }
    };
    if let Ok(token) = std::env::var("RUNG_GATEWAY_TOKEN") {
        client = client.with_token(token);
    }

    if list {
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
                size(px(900.), px(480.)),
                cx,
            ))),
            ..Default::default()
        };
        open_window(options, cx, |_, cx| {
            cx.new(|cx| InstanceList::new(client, cx))
        })
        .expect("the window opens");
        cx.activate(true);
    });
}
