use axum::http::StatusCode;

use nerve_waf_dataplane::{build_waf_edge_snapshot, WafEdgeBuildInput, WafEdgeSnapshot};

use crate::dataplane::generation::dataplane_generation;
use crate::site_inspect::load_global_inspect_overlay;
use crate::AppState;

use super::routes::collect_edge_routes;

pub async fn compile_waf_edge_snapshot(state: &AppState) -> Result<WafEdgeSnapshot, StatusCode> {
    let (routes, upstreams, sites, site_inspect) = collect_edge_routes(state).await?;
    let global = load_global_inspect_overlay(state);
    let input = WafEdgeBuildInput {
        generation: dataplane_generation(state),
        routes,
        upstreams,
        sites,
        site_inspect,
        global,
    };
    Ok(build_waf_edge_snapshot(input))
}
