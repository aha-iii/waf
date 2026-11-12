use crate::events::{execute_inspect, InspectBody};
use crate::AppState;
use serde_json::Value;
use std::path::Path;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};

pub async fn run_unix_socket_server(
    state: Arc<AppState>,
    socket_path: &str,
) -> std::io::Result<()> {
    if let Some(parent) = Path::new(socket_path).parent() {
        std::fs::create_dir_all(parent)?;
    }
    let _ = std::fs::remove_file(socket_path);
    let listener = UnixListener::bind(socket_path)?;
    tracing::info!("nerve-waf-agent-inspect listening on unix://{socket_path}");

    loop {
        let (stream, _) = listener.accept().await?;
        let st = state.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_client(st, stream).await {
                tracing::warn!("agent inspect client error: {e}");
            }
        });
    }
}

async fn handle_client(state: Arc<AppState>, stream: UnixStream) -> Result<(), String> {
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();
    let line = lines
        .next_line()
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| "empty request".to_string())?;

    let body: InspectBody =
        serde_json::from_str(&line).map_err(|e| format!("invalid json: {e}"))?;
    let resp: Value = execute_inspect(&state, body)
        .await
        .map_err(|e| e.to_string())?;
    let out = format!("{}\n", resp);
    writer
        .write_all(out.as_bytes())
        .await
        .map_err(|e| e.to_string())?;
    writer.flush().await.map_err(|e| e.to_string())?;
    Ok(())
}
