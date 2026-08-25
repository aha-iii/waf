use nerve_waf_sdk::L7Upstream;
use std::collections::HashMap;

pub fn pick_upstream_url(up: &L7Upstream, rr: &mut HashMap<String, usize>) -> Option<String> {
    if up.is_gateway_origin() {
        return None;
    }
    let idx = *rr.entry(up.id.clone()).or_insert(0);
    rr.insert(up.id.clone(), idx.saturating_add(1));
    up.pick_url(idx)
}
