use nerve_waf_dataplane::{GlobalInspectOverlay, SiteInspectSnapshot};
use nerve_waf_embed::{
    EmbedInspectRequest, EmbedInspectResponse, EmbeddedWaf, InspectPreparedOptions,
};

use crate::dataplane::inspect::cache::SharedInspectCache;

pub fn build_embed_request(
    method: String,
    host: String,
    uri: String,
    headers_json: String,
    body: String,
    src_ip: String,
    user_agent: String,
) -> EmbedInspectRequest {
    EmbedInspectRequest {
        method,
        host,
        uri,
        headers: headers_json,
        body,
        src_ip,
        user_agent,
        response_body: None,
    }
}

fn prepared_for_site(
    waf: &EmbeddedWaf,
    cache: &SharedInspectCache,
    generation: u64,
    site_id: &str,
    site_snap: &SiteInspectSnapshot,
) -> std::sync::Arc<nerve_waf_embed::PreparedSiteInspect> {
    if let Ok(guard) = cache.lock() {
        if let Some(prepared) = guard.get_prepared(generation, site_id) {
            return prepared;
        }
    }
    let prepared = waf.prepare_site_inspect(site_snap);
    let mut guard = cache.lock().expect("inspect cache lock");
    if let Some(hit) = guard.get_prepared(generation, site_id) {
        return hit;
    }
    guard.put_prepared(generation, site_id, prepared)
}

/// Run inspect against snapshot (blocking thread; CRS rules cached by generation).
pub fn inspect_for_site_blocking(
    waf: &EmbeddedWaf,
    cache: &SharedInspectCache,
    site_id: &str,
    generation: u64,
    site_snap: Option<SiteInspectSnapshot>,
    global: GlobalInspectOverlay,
    req: EmbedInspectRequest,
    opts: InspectPreparedOptions,
) -> EmbedInspectResponse {
    if let Some(site_snap) = site_snap {
        let prepared = prepared_for_site(waf, cache, generation, site_id, &site_snap);
        let verdict = waf.inspect_prepared_verdict(req, &prepared, &global, opts);
        EmbeddedWaf::response_from_verdict(verdict)
    } else {
        waf.inspect(req)
    }
}
