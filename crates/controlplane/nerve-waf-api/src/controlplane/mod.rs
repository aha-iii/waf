//! 控制面：Console HTTP + 进程基础设施。

pub mod shared;

pub mod ai_ml_labels;
pub mod ai_ops;
pub mod ai_policy_sim;
pub mod ai_rate_limit;
pub mod ai_settings;
pub mod ai_posture;
pub mod alert_proxy;
pub mod app_crs;
pub mod custom_rules;
pub mod events;
pub mod extensions;
pub mod fp_remediation;
pub mod ingress;
pub mod network;
pub mod np;
pub mod phase8;
pub mod phase10;
pub mod phase12;
pub mod plugin_l7;
pub mod routes;
pub mod security;
pub mod sites;
pub mod topology;
pub mod traffic_stats;
pub mod triage;
pub mod users;
pub mod public_demo;

use axum::Router;
use std::sync::Arc;

use shared::AppState;

/// 合并全部控制面路由（Console 调用的 `/api/*`，不含 `dataplane/`）。
pub fn controlplane_router() -> Router<Arc<AppState>> {
    Router::new()
        .merge(routes::router())
        .merge(custom_rules::custom_rules_router())
        .merge(plugin_l7::plugin_l7_router())
        .merge(triage::triage_router())
        .merge(ai_settings::ai_settings_router())
        .merge(ai_posture::posture_router())
        .merge(ai_ml_labels::ml_labels_router())
        .merge(ai_ops::ai_ops_router())
        .merge(ai_policy_sim::policy_sim_router())
        .merge(alert_proxy::router())
        .merge(topology::topology_router())
        .merge(network::network_router())
        .merge(sites::platform_router())
        .merge(ingress::ingress_router())
        .merge(np::np_router())
        .merge(extensions::extensions_router())
        .merge(users::users_router())
        .merge(public_demo::public_demo_router())
        .merge(security::security_router())
        .merge(app_crs::app_crs_router())
        .merge(fp_remediation::fp_remediation_router())
        .merge(traffic_stats::traffic_router())
        .merge(phase8::phase8_router())
        .merge(phase10::phase10_router())
        .merge(phase12::phase12_router())
}

// 兼容旧名
pub use controlplane_router as control_router;
