//! `rung-gateway --config <file>`: serve the gateway until killed.

use rung_gateway::Config;

const USAGE: &str = "usage: rung-gateway --config <gateway.yaml>\n\
\n\
The file names the listen address, an optional built app directory, the\n\
variables that hold read-only tokens, and each instance's url and the\n\
variable that holds its key (see docs/rung-host-api.md).";

#[tokio::main]
async fn main() {
    let mut args = std::env::args().skip(1);
    let path = match (args.next().as_deref(), args.next(), args.next()) {
        (Some("--config"), Some(p), None) => p,
        (Some("--help" | "-h"), None, None) => {
            println!("{USAGE}");
            return;
        }
        _ => {
            eprintln!("{USAGE}");
            std::process::exit(2);
        }
    };
    let settings =
        match Config::load(path.as_ref()).and_then(|c| c.resolve(|n| std::env::var(n).ok())) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("rung-gateway: {e}");
                std::process::exit(2);
            }
        };
    let n = settings.instances.len();
    match rung_gateway::start(settings).await {
        Ok(running) => {
            println!(
                "rung-gateway listening on http://{} ({n} instances)",
                running.addr
            );
            std::future::pending::<()>().await;
        }
        Err(e) => {
            eprintln!("rung-gateway: {e}");
            std::process::exit(1);
        }
    }
}
