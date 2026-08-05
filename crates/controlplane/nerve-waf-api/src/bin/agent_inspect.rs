//! Agent 本地 WAF 检测守护进程（Unix Socket，内嵌 inspect 流水线）

use clap::Parser;
use nerve_waf_api::bootstrap::{self, BootstrapOptions};
use nerve_waf_api::inspect_daemon;
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
        std::env::set_var("RUST_LOG", "nerve_waf_agent_inspect=info,nerve_waf_api=info");
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
