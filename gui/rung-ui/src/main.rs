//! `rung-ui`: a window onto the instances behind a rung gateway.
//!
//! ```text
//! rung-ui [--gateway URL] [--select ID] [--list | --show ID]
//! ```
//!
//! The gateway is `--gateway`, else `$RUNG_GATEWAY`, else
//! `http://127.0.0.1:8787`. A read-only token, when the gateway wants one, is
//! read from `$RUNG_GATEWAY_TOKEN` and never shown. `--list` prints the list
//! once as text and exits; `--show ID` prints one instance's page the same way
//! (neither needs a display). In the window a click on an instance opens its
//! page; `--select ID` opens with one already chosen. The window only reads:
//! nothing in it writes to a host.

use std::time::Duration;

use gpui_kit::component::{ActiveTheme as _, h_flex, v_flex};
use gpui_kit::*;
use rung_client::Client;
use rung_ui::{Detail, Link, Row, Snapshot, as_text, detail_text, look};

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
}

impl InstanceList {
    fn new(client: Client, selected: Option<String>, cx: &mut Context<Self>) -> Self {
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
            selected,
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
                let id = r.id.clone();
                let selected = self.selected.as_deref() == Some(r.id.as_str());
                sections.push(row(
                    r,
                    &theme,
                    selected,
                    cx.listener(move |this, _, _, cx| {
                        this.selected = Some(id.clone());
                        cx.notify();
                    }),
                ));
            }
        }
        let rows = sections;

        let empty = (rows.is_empty() && !down && self.link == Link::Up).then(|| {
            div()
                .text_color(muted)
                .child("No instances are registered with this gateway.")
        });

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
            .children(
                self.selected
                    .as_ref()
                    .map(|id| page(id, self.snapshot.rows.iter().find(|r| &r.id == id), &theme)),
            )
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
fn page(id: &str, row: Option<&Row>, theme: &gpui_kit::component::Theme) -> AnyElement {
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
    for sec in &detail.sections {
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
    col.into_any_element()
}

/// The command line.
struct Opts {
    gateway: String,
    select: Option<String>,
    list: bool,
    show: Option<String>,
}

/// `--gateway URL` and `--list`; anything else is a usage error.
fn args() -> Result<Opts, String> {
    let mut o = Opts {
        gateway: std::env::var("RUNG_GATEWAY").unwrap_or_else(|_| DEFAULT_GATEWAY.into()),
        select: None,
        list: false,
        show: None,
    };
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--gateway" => o.gateway = it.next().ok_or("--gateway needs an address")?,
            "--select" => o.select = Some(it.next().ok_or("--select needs an instance id")?),
            "--show" => o.show = Some(it.next().ok_or("--show needs an instance id")?),
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
                "rung-ui: {e}\nusage: rung-ui [--gateway URL] [--select ID] [--list | --show ID]"
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
        open_window(options, cx, |_, cx| {
            cx.new(|cx| InstanceList::new(client, opts.select, cx))
        })
        .expect("the window opens");
        cx.activate(true);
    });
}
