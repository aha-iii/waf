//! Control-plane process infra: AppState, platform services, logging, middleware, bootstrap.

pub mod app_state;
pub mod bootstrap;
/// Unix-socket inspect daemon: Unix domain sockets only（Windows 不可用）。
#[cfg(unix)]
pub mod inspect_daemon;
pub mod logging;
pub mod phase11_middleware;
pub mod platform;
pub mod trace_bind;
pub mod trace_middleware;
pub mod triage_ai;
pub mod triage_service;

pub use app_state::AppState;
