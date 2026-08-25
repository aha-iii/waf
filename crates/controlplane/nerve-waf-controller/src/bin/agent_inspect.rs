//! Local Agent WAF inspect daemon over Unix socket / low-latency path.

use clap::Parser;
use nerve_waf_controller::bootstrap::{self, BootstrapOptions};
use nerve_waf_controller::inspect_daemon;
use std::path::PathBuf;
use tracing_subscriber::EnvFilter;

#[derive(Parser, Debug)]
#[command(name = "nerve-waf-agent-inspect", about = "Agent 本地 WAF inspect（Unix Socket）")]
struct Args {
    #[arg(long, env = "NERVE_WAF_INSPECT_SOCKET", default_value = "/var/run/nerve/waf-inspect.sock")]
    socket: PathBuf,
    #[arg(long, env = "WAF_CONFIG")]
    config: Option<PathBuf>,
}

#[tokio::main]
async fn main() {
    bootstrap::load_env_files();
    let args = Args::parse();

    if std::env::var("RUST_LOG").is_err() {
        std::env::set_var("RUST_LOG", "nerve_waf_agent_inspect=info,nerve_waf_controller=info");
    }
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let opts = BootstrapOptions::from_env_and_config(args.config);
    let state = bootstrap::build_app_state(opts).await;
    let socket = args.socket.to_string_lossy().into_owned();
    if let Err(e) = inspect_daemon::run_unix_socket_server(state, &socket).await {
        eprintln!("agent inspect server failed: {e}");
        std::process::exit(1);
    }
}
