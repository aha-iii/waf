//! CLI to compile/sync WAF-L3 plans to Nerve Agent.

use clap::Parser;
use nerve_waf_client::NerveClient;
use nerve_waf_engine::sync_config_with_runtime;
use nerve_waf_plugin_host::{PluginHost, resolve_plugins_dir};
use nerve_waf_store::open_config_backend;
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(name = "nerve-waf-sync", about = "Compile routes and sync to Nerve Agent")]
struct Args {
    #[arg(long, env = "NERVE_URL", default_value = "http://127.0.0.1:8766")]
    nerve_url: String,
    #[arg(long, env = "NERVE_WAF_DATA")]
    data_dir: Option<String>,
    #[arg(long, env = "DATABASE_URL")]
    database_url: Option<String>,
    #[arg(long, default_value = "data/routes.json")]
    config: PathBuf,
}

#[tokio::main]
async fn main() -> ExitCode {
    let _ = dotenvy::dotenv();
    let args = Args::parse();
    let data_path = args
        .data_dir
        .clone()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("data"));

    let config_backend = match open_config_backend(args.database_url.as_deref(), &data_path).await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("config store: {e}");
            return ExitCode::from(1);
        }
    };

    let config = match config_backend.load_config().await {
        Ok(c) => c,
        Err(e) => {
            eprintln!("load config: {e}");
            return ExitCode::from(1);
        }
    };

    if config.routes.is_empty() && !args.config.exists() {
        eprintln!("No routes in config store and no file at {}", args.config.display());
        return ExitCode::from(1);
    }

    let modules = config_backend
        .load_modules()
        .await
        .unwrap_or_default();
    let runtime = config_backend
        .active_runtime_deny_ips()
        .await
        .unwrap_or_default();

    let host = PluginHost::new(resolve_plugins_dir(Some(&data_path)));
    let l7_rule_count = host.total_rule_count();
    let client = NerveClient::new(&args.nerve_url);
    let result = sync_config_with_runtime(&client, &config, &modules, l7_rule_count, &runtime).await;
    println!("{}", serde_json::to_string_pretty(&result).unwrap());
    if result.ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::from(1)
    }
}
