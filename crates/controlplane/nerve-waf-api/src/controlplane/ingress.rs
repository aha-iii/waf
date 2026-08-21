//! Control plane — app ingress, Gateway dataplane, Upstreams (Console write path).
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::net::ToSocketAddrs;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::platform::{IngressOrigin, ProtectedSite};
use crate::AppState;
use chrono::Utc;
use nerve_waf_sdk::L7Upstream;

pub fn ingress_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/ingress/platform", get(get_ingress_platform))
        .route(
            "/api/ingress/gateway/upstreams",
            get(list_gateway_upstreams),
        )
        .route(
            "/api/sites/gateway-bindings",
            get(list_gateway_bindings),
        )
        .route(
            "/api/sites/{id}/ingress",
            get(get_site_ingress).put(put_site_ingress),
        )
        .route(
            "/api/sites/{id}/ingress/dns-check",
            post(dns_check_site),
        )
        .route(
            "/api/sites/{id}/ingress/apply-gateway",
            post(apply_site_to_gateway),
        )
        .route("/api/gateway/dataplane", get(get_dataplane))
        .route("/api/gateway/dataplane/sync", post(sync_dataplane))
        .route("/api/upstreams", get(get_waf_upstreams).put(put_waf_upstreams))
}

fn gateway_admin_base() -> String {
    std::env::var("NERVE_GATEWAY_ADMIN_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:9080".into())
        .trim_end_matches('/')
        .to_string()
}

fn gateway_dashboard_base() -> String {
    std::env::var("NERVE_GATEWAY_DASHBOARD_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:5176".into())
        .trim_end_matches('/')
        .to_string()
}

async fn gateway_get(path: &str) -> Result<Value, String> {
    let url = format!("{}{}", gateway_admin_base(), path);
    let client = reqwest::Client::new();
    let res = client
        .get(&url)
        .timeout(std::time::Duration::from_secs(5))
        .send()
        .await
        .map_err(|e| format!("连接 Gateway Admin 失败（{url}）: {e}"))?;
    if !res.status().is_success() {
        return Err(format!("Gateway Admin HTTP {}", res.status()));
    }
    res.json().await.map_err(|e| format!("解析 Gateway 响应失败: {e}"))
}

async fn gateway_put(path: &str, body: &Value) -> Result<(), String> {
    let url = format!("{}{}", gateway_admin_base(), path);
    let client = reqwest::Client::new();
    let res = client
        .put(&url)
        .timeout(std::time::Duration::from_secs(8))
        .json(body)
        .send()
        .await
        .map_err(|e| format!("写入 Gateway Admin 失败（{url}）: {e}"))?;
    if !res.status().is_success() {
        let t = res.text().await.unwrap_or_default();
        return Err(format!("Gateway Admin 写入失败: {t}"));
    }
    Ok(())
}

async fn list_gateway_upstreams() -> Json<Value> {
    match gateway_get("/admin/upstreams").await {
        Ok(v) => {
            let items = v
                .get("upstreams")
                .cloned()
                .unwrap_or_else(|| json!([]));
            Json(json!({
                "ok": true,
                "upstreams": items,
                "admin_url": gateway_admin_base(),
                "dashboard_url": gateway_dashboard_base(),
            }))
        }
        Err(e) => Json(json!({
            "ok": false,
            "upstreams": [],
            "error": e,
            "admin_url": gateway_admin_base(),
            "dashboard_url": gateway_dashboard_base(),
            "hint": "请确认 Nerve-gateway Admin 已启动（默认 :9080），或设置 NERVE_GATEWAY_ADMIN_URL",
        })),
    }
}

fn route_host(route: &Value) -> Option<String> {
    route
        .get("match")
        .and_then(|m| m.get("host"))
        .and_then(|h| h.as_str())
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
}

fn route_nerve_waf_app_id(route: &Value) -> Option<String> {
    let plugins = route.get("plugins")?.as_array()?;
    for p in plugins {
        let name = p.get("name").and_then(|n| n.as_str()).unwrap_or("");
        if name != "nerve-waf" {
            continue;
        }
        let enabled = p
            .get("enabled")
            .and_then(|e| e.as_bool())
            .unwrap_or(true);
        if !enabled {
            continue;
        }
        let cfg = p.get("config").cloned().unwrap_or(json!({}));
        let app_id = cfg
            .get("app_id")
            .or_else(|| cfg.get("site_id"))
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or("default");
        return Some(app_id.to_string());
    }
    None
}

fn host_matches_site(route_host: &str, site_hosts: &[String]) -> bool {
    let rh = route_host.trim().to_lowercase();
    if rh.is_empty() || rh == "*" {
        return true;
    }
    site_hosts.iter().any(|h| {
        let sh = h.trim().to_lowercase();
        sh.is_empty() || sh == "*" || sh == rh
    })
}

/// Check whether Gateway routes have nerve-waf plugin mounted with each app's app_id.
async fn list_gateway_bindings(State(state): State<Arc<AppState>>) -> Json<Value> {
    let sites = match state.platform.sites.read() {
        Ok(g) => g.clone(),
        Err(_) => {
            return Json(json!({
                "ok": false,
                "gateway_reachable": false,
                "error": "读取应用列表失败",
                "bindings": {},
                "admin_url": gateway_admin_base(),
                "dashboard_url": gateway_dashboard_base(),
            }));
        }
    };

    let rt_list = match gateway_get("/admin/routes").await {
        Ok(v) => v,
        Err(e) => {
            let mut bindings = serde_json::Map::new();
            for s in &sites {
                bindings.insert(
                    s.id.clone(),
                    json!({
                        "mounted": false,
                        "route_count": 0,
                        "routes": [],
                        "hosts_covered": [],
                        "hosts_uncovered": s.hosts,
                    }),
                );
            }
            return Json(json!({
                "ok": false,
                "gateway_reachable": false,
                "error": e,
                "bindings": bindings,
                "admin_url": gateway_admin_base(),
                "dashboard_url": gateway_dashboard_base(),
                "hint": "请确认 Nerve-gateway Admin 已启动（默认 :9080），或设置 NERVE_GATEWAY_ADMIN_URL",
            }));
        }
    };

    let routes = rt_list
        .get("routes")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut bindings = serde_json::Map::new();
    for s in &sites {
        let mut matched: Vec<Value> = Vec::new();
        let mut covered: Vec<String> = Vec::new();
        for r in &routes {
            let enabled = r
                .get("enabled")
                .and_then(|e| e.as_bool())
                .unwrap_or(true);
            if !enabled {
                continue;
            }
            let Some(app_id) = route_nerve_waf_app_id(r) else {
                continue;
            };
            if app_id != s.id {
                continue;
            }
            let host = route_host(r).unwrap_or_else(|| "*".into());
            if !host_matches_site(&host, &s.hosts) && s.hosts.iter().any(|h| h.trim() != "*") {
                // app_id match is enough to treat as mounted; hostnames are coverage hints only.
            }
            if !covered.iter().any(|c| c == &host) {
                covered.push(host.clone());
            }
            matched.push(json!({
                "id": r.get("id").and_then(|x| x.as_str()).unwrap_or(""),
                "name": r.get("name").and_then(|x| x.as_str()).unwrap_or(""),
                "host": host,
                "enabled": true,
            }));
        }
        let uncovered: Vec<String> = s
            .hosts
            .iter()
            .filter(|h| {
                let sh = h.trim().to_lowercase();
                if sh.is_empty() || sh == "*" {
                    return false;
                }
                !covered.iter().any(|c| {
                    let ch = c.trim().to_lowercase();
                    ch == "*" || ch == sh
                })
            })
            .cloned()
            .collect();
        bindings.insert(
            s.id.clone(),
            json!({
                "mounted": !matched.is_empty(),
                "route_count": matched.len(),
                "routes": matched,
                "hosts_covered": covered,
                "hosts_uncovered": uncovered,
            }),
        );
    }

    Json(json!({
        "ok": true,
        "gateway_reachable": true,
        "bindings": bindings,
        "admin_url": gateway_admin_base(),
        "dashboard_url": gateway_dashboard_base(),
    }))
}

fn cname_zone() -> String {
    std::env::var("WAF_CNAME_ZONE").unwrap_or_else(|_| "waf.localhost".into())
}

fn edge_ips() -> Vec<String> {
    std::env::var("WAF_EDGE_IPS")
        .unwrap_or_else(|_| "127.0.0.1".into())
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn egress_cidrs() -> Vec<String> {
    std::env::var("WAF_EDGE_EGRESS_CIDRS")
        .ok()
        .map(|s| {
            s.split(',')
                .map(|x| x.trim().to_string())
                .filter(|x| !x.is_empty())
                .collect()
        })
        .unwrap_or_else(edge_ips)
}

fn now_rfc3339() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("{secs}")
}

async fn get_ingress_platform() -> Json<Value> {
    Json(json!({
        "cname_zone": cname_zone(),
        "edge_ips": edge_ips(),
        "egress_cidrs": egress_cidrs(),
        "acme_enabled": std::env::var("WAF_ACME_ENABLED").ok().as_deref() != Some("0"),
        "hint": "将业务域名 CNAME 到应用的 cname_fqdn；源站建议仅放行 egress_cidrs",
    }))
}

fn ensure_cname_fields(site: &mut ProtectedSite) {
    let zone = cname_zone();
    let slug = site
        .cname_slug
        .clone()
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| site.id.clone());
    let slug = slug
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    site.cname_slug = Some(slug.clone());
    site.cname_fqdn = Some(format!("{slug}.{zone}"));
    if site.ingress_mode == "cname" || site.integration == "cname" {
        site.ingress_mode = "cname".into();
        site.integration = "cname".into();
    }
    if site.origins.is_empty() {
        if let Some(up) = site.upstream.clone().filter(|s| !s.trim().is_empty()) {
            if let Some(origin) = parse_upstream_to_origin(&up) {
                site.origins.push(origin);
            }
        }
    }
}

fn parse_upstream_to_origin(raw: &str) -> Option<IngressOrigin> {
    let raw = raw.trim();
    let (scheme, rest) = if let Some(r) = raw.strip_prefix("https://") {
        ("https", r)
    } else if let Some(r) = raw.strip_prefix("http://") {
        ("http", r)
    } else {
        ("http", raw)
    };
    let host_port = rest.split('/').next().unwrap_or(rest);
    let (address, port) = if let Some((h, p)) = host_port.rsplit_once(':') {
        (h.to_string(), p.parse().unwrap_or(if scheme == "https" { 443 } else { 80 }))
    } else {
        (
            host_port.to_string(),
            if scheme == "https" { 443 } else { 80 },
        )
    };
    if address.is_empty() {
        return None;
    }
    Some(IngressOrigin {
        address,
        port,
        scheme: scheme.into(),
        host_header: None,
        sni: None,
        weight: 100,
        backup: false,
        hc_type: "tcp".into(),
        hc_path: None,
        hc_interval_secs: 10,
    })
}

fn site_ingress_json(site: &ProtectedSite) -> Value {
    json!({
        "site_id": site.id,
        "ingress_mode": site.ingress_mode,
        "cname_slug": site.cname_slug,
        "cname_fqdn": site.cname_fqdn,
        "dns_status": site.dns_status,
        "dns_checked_at": site.dns_checked_at,
        "dns_error": site.dns_error,
        "tls_mode": site.tls_mode,
        "certificate_id": site.certificate_id,
        "websocket_enabled": site.websocket_enabled,
        "xff_trusted_hops": site.xff_trusted_hops,
        "origins": site.origins,
        "edge_pop_ids": site.edge_pop_ids,
        "gateway_upstream_id": site.gateway_upstream_id,
        "hosts": site.hosts,
        "has_upload_cert": site.tls_cert.as_ref().map(|c| !c.is_empty()).unwrap_or(false),
        "gateway_dashboard_url": gateway_dashboard_base(),
    })
}

async fn get_site_ingress(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let sites = state.platform.sites.read().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let site = sites
        .iter()
        .find(|s| s.id == id)
        .ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(site_ingress_json(site)))
}

#[derive(Debug, Deserialize)]
struct IngressUpdateBody {
    #[serde(default)]
    ingress_mode: Option<String>,
    #[serde(default)]
    cname_slug: Option<String>,
    #[serde(default)]
    tls_mode: Option<String>,
    #[serde(default)]
    websocket_enabled: Option<bool>,
    #[serde(default)]
    xff_trusted_hops: Option<u32>,
    #[serde(default)]
    origins: Option<Vec<IngressOrigin>>,
    #[serde(default)]
    edge_pop_ids: Option<Vec<String>>,
    #[serde(default)]
    gateway_upstream_id: Option<String>,
    /// true = clear binding and use manually filled origins on this page.
    #[serde(default)]
    clear_gateway_upstream: Option<bool>,
    #[serde(default)]
    tls_cert: Option<String>,
    #[serde(default)]
    tls_key: Option<String>,
    /// Whether to push to Gateway Admin after save (origin is now Gateway-owned; default false).
    #[serde(default)]
    apply_gateway: Option<bool>,
}

async fn put_site_ingress(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<IngressUpdateBody>,
) -> Result<Json<Value>, StatusCode> {
    {
        let mut sites = state
            .platform
            .sites
            .write()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let site = sites
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or(StatusCode::NOT_FOUND)?;
        if let Some(mode) = body.ingress_mode {
            let mode = mode.trim().to_lowercase();
            site.ingress_mode = mode.clone();
            if mode == "cname" {
                site.integration = "cname".into();
            }
        }
        if let Some(slug) = body.cname_slug {
            site.cname_slug = Some(slug);
        }
        if let Some(tls) = body.tls_mode {
            site.tls_mode = tls;
        }
        if let Some(ws) = body.websocket_enabled {
            site.websocket_enabled = ws;
        }
        if let Some(h) = body.xff_trusted_hops {
            site.xff_trusted_hops = h;
        }
        if let Some(origins) = body.origins {
            site.origins = origins;
        }
        if let Some(pops) = body.edge_pop_ids {
            site.edge_pop_ids = pops;
        }
        if body.clear_gateway_upstream == Some(true) {
            site.gateway_upstream_id = None;
        } else if let Some(uid) = body.gateway_upstream_id {
            let uid = uid.trim().to_string();
            site.gateway_upstream_id = if uid.is_empty() { None } else { Some(uid) };
        }
        if let Some(cert) = body.tls_cert {
            if !cert.trim().is_empty() {
                site.tls_cert = Some(cert);
                site.tls_mode = "upload".into();
            }
        }
        if let Some(key) = body.tls_key {
            if !key.trim().is_empty() {
                site.tls_key = Some(key);
            }
        }
        ensure_cname_fields(site);
    }
    state
        .platform
        .save()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let want_apply = body.apply_gateway.unwrap_or(false);
    let mut gateway_apply: Option<Value> = None;
    if want_apply {
        let site_snap = {
            let sites = state
                .platform
                .sites
                .read()
                .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
            sites.iter().find(|s| s.id == id).cloned()
        };
        if let Some(site) = site_snap {
            if site.ingress_mode == "cname" || site.integration == "cname" {
                match push_site_to_gateway(&site).await {
                    Ok(v) => gateway_apply = Some(v),
                    Err(e) => {
                        gateway_apply = Some(json!({ "ok": false, "error": e }));
                    }
                }
            }
        }
    }

    let sites = state.platform.sites.read().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let site = sites.iter().find(|s| s.id == id).ok_or(StatusCode::NOT_FOUND)?;
    crate::dataplane::bump_after_mutation(&state);
    Ok(Json(json!({
        "ok": true,
        "ingress": site_ingress_json(site),
        "gateway_apply": gateway_apply,
    })))
}

async fn apply_site_to_gateway(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let site = {
        let mut sites = state
            .platform
            .sites
            .write()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let site = sites
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or(StatusCode::NOT_FOUND)?;
        ensure_cname_fields(site);
        site.clone()
    };
    state
        .platform
        .save()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    match push_site_to_gateway(&site).await {
        Ok(v) => Ok(Json(v)),
        Err(e) => Ok(Json(json!({ "ok": false, "error": e }))),
    }
}

fn origins_to_gateway_upstream(site: &ProtectedSite, upstream_id: &str) -> Value {
    let nodes: Vec<Value> = site
        .origins
        .iter()
        .filter(|o| !o.address.trim().is_empty())
        .map(|o| {
            json!({
                "host": o.address.trim(),
                "port": o.port,
                "weight": o.weight.max(1),
            })
        })
        .collect();
    let scheme = site
        .origins
        .first()
        .map(|o| o.scheme.as_str())
        .unwrap_or("http");
    json!({
        "id": upstream_id,
        "name": format!("{} 回源", site.name),
        "enabled": true,
        "scheme": scheme,
        "type": "roundrobin",
        "nodes": nodes,
        "description": format!("由 WAF 应用 {} CNAME 接入自动同步", site.id),
    })
}

async fn push_site_to_gateway(site: &ProtectedSite) -> Result<Value, String> {
    let up_list = gateway_get("/admin/upstreams").await?;
    let rt_list = gateway_get("/admin/routes").await?;
    let mut upstreams = up_list
        .get("upstreams")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let mut routes = rt_list
        .get("routes")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let bind_existing = site
        .gateway_upstream_id
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty());

    let upstream_id = if let Some(uid) = bind_existing {
        if !upstreams.iter().any(|u| u.get("id").and_then(|x| x.as_str()) == Some(uid)) {
            return Err(format!("Gateway 中不存在 upstream「{uid}」，请刷新后重选"));
        }
        uid.to_string()
    } else {
        let uid = format!("up-{}", site.id);
        let created = origins_to_gateway_upstream(site, &uid);
        if created
            .get("nodes")
            .and_then(|n| n.as_array())
            .map(|a| a.is_empty())
            .unwrap_or(true)
        {
            return Err("请至少配置一条回源地址，或选择 Gateway 已有上游".into());
        }
        if let Some(pos) = upstreams
            .iter()
            .position(|u| u.get("id").and_then(|x| x.as_str()) == Some(uid.as_str()))
        {
            upstreams[pos] = created;
        } else {
            upstreams.push(created);
        }
        uid
    };

    // Remove this app's old auto routes, then rebuild by Host.
    let auto_prefix = format!("waf-{}", site.id);
    routes.retain(|r| {
        r.get("id")
            .and_then(|x| x.as_str())
            .map(|id| !id.starts_with(&auto_prefix))
            .unwrap_or(true)
    });

    for host in &site.hosts {
        let host = host.trim();
        if host.is_empty() || host == "*" {
            continue;
        }
        let rid = format!(
            "{}-{}",
            auto_prefix,
            host.replace('.', "-").replace('*', "star")
        );
        routes.push(json!({
            "id": rid,
            "name": format!("{} / {}", site.name, host),
            "priority": 100,
            "enabled": site.enabled,
            "match": { "host": host, "path_prefix": "/" },
            "upstream_id": upstream_id,
            "plugins": [{
                "name": "nerve-waf",
                "enabled": true,
                "config": { "mode": "edge", "app_id": site.id }
            }],
        }));
    }

    gateway_put("/admin/upstreams", &json!({ "items": upstreams })).await?;
    gateway_put("/admin/routes", &json!({ "items": routes })).await?;

    Ok(json!({
        "ok": true,
        "upstream_id": upstream_id,
        "routes": site.hosts.len(),
        "dashboard_upstream_url": format!("{}/#/upstreams?id={}", gateway_dashboard_base(), upstream_id),
        "message": "已同步到 Gateway 边缘（路由 + 上游）",
    }))
}

async fn dns_check_site(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, StatusCode> {
    let (hosts, expected_cname) = {
        let mut sites = state
            .platform
            .sites
            .write()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let site = sites
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or(StatusCode::NOT_FOUND)?;
        ensure_cname_fields(site);
        site.dns_status = "checking".into();
        (
            site.hosts.clone(),
            site.cname_fqdn.clone().unwrap_or_default(),
        )
    };

    let mut ok_hosts = Vec::new();
    let mut bad = Vec::new();
    for host in &hosts {
        let host = host.trim().to_lowercase();
        if host.is_empty() || host == "*" {
            continue;
        }
        match check_host_points_to(&host, &expected_cname) {
            Ok(true) => ok_hosts.push(host),
            Ok(false) => bad.push(format!("{host}: CNAME 未指向 {expected_cname}")),
            Err(e) => bad.push(format!("{host}: {e}")),
        }
    }

    {
        let mut sites = state
            .platform
            .sites
            .write()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        let site = sites
            .iter_mut()
            .find(|s| s.id == id)
            .ok_or(StatusCode::NOT_FOUND)?;
        site.dns_checked_at = Some(now_rfc3339());
        if bad.is_empty() && !ok_hosts.is_empty() {
            site.dns_status = "active".into();
            site.dns_error = None;
        } else if bad.is_empty() {
            site.dns_status = "pending".into();
            site.dns_error = Some("无业务 Host 可校验".into());
        } else {
            site.dns_status = "error".into();
            site.dns_error = Some(bad.join("; "));
        }
    }
    state
        .platform
        .save()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let sites = state.platform.sites.read().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let site = sites.iter().find(|s| s.id == id).ok_or(StatusCode::NOT_FOUND)?;
    Ok(Json(json!({
        "ok": site.dns_status == "active",
        "ingress": site_ingress_json(site),
        "checked_ok": ok_hosts,
        "errors": bad,
    })))
}

/// Best-effort resolve: Host resolves to same address as edge IP, or CNAME target string matches (needs system resolver).
fn check_host_points_to(host: &str, expected_cname: &str) -> Result<bool, String> {
    let expected = expected_cname.trim_end_matches('.').to_lowercase();
    if expected.is_empty() {
        return Err("未分配 cname_fqdn".into());
    }
    // Resolve A records for business domain and ingress name; intersection means pointed at edge (CNAME-chain friendly).
    let edge: Vec<_> = edge_ips();
    let host_addrs = resolve_ips(host)?;
    let cname_addrs = resolve_ips(&expected).unwrap_or_default();
    if !host_addrs.is_empty() {
        if host_addrs.iter().any(|a| edge.iter().any(|e| e == a)) {
            return Ok(true);
        }
        if !cname_addrs.is_empty()
            && host_addrs.iter().any(|a| cname_addrs.iter().any(|c| c == a))
        {
            return Ok(true);
        }
    }
    // Local dev: if expected contains localhost, allow 127.0.0.1.
    if expected.contains("localhost")
        && host_addrs.iter().any(|a| a == "127.0.0.1" || a == "::1")
    {
        return Ok(true);
    }
    Ok(false)
}

fn resolve_ips(name: &str) -> Result<Vec<String>, String> {
    let keyed = if name.contains(':') {
        name.to_string()
    } else {
        format!("{name}:443")
    };
    match keyed.to_socket_addrs() {
        Ok(iter) => Ok(iter.map(|a| a.ip().to_string()).collect()),
        Err(e) => Err(format!("DNS 解析失败: {e}")),
    }
}

#[derive(Debug, Serialize)]
struct DataplaneSnapshot {
    version: u64,
    generated_at: String,
    cname_zone: String,
    edge_ips: Vec<String>,
    sites: Vec<Value>,
    routes: Vec<Value>,
    upstreams: Vec<Value>,
    ssl: Vec<Value>,
}

fn build_dataplane(state: &AppState) -> Result<DataplaneSnapshot, StatusCode> {
    let sites = state
        .platform
        .sites
        .read()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let mut out_sites = Vec::new();
    let mut routes = Vec::new();
    let mut upstreams = Vec::new();
    let mut ssl = Vec::new();

    for site in sites.iter().filter(|s| s.enabled) {
        let is_cname = site.ingress_mode == "cname" || site.integration == "cname";
        if !is_cname {
            continue;
        }
        let mut s = site.clone();
        ensure_cname_fields(&mut s);
        let upstream_id = format!("up-{}", s.id);
        let origins = if s.origins.is_empty() {
            vec![IngressOrigin {
                address: "127.0.0.1".into(),
                port: 8080,
                scheme: "http".into(),
                host_header: s.hosts.first().cloned(),
                sni: None,
                weight: 100,
                backup: false,
                hc_type: "none".into(),
                hc_path: None,
                hc_interval_secs: 10,
            }]
        } else {
            s.origins.clone()
        };
        upstreams.push(json!({
            "id": upstream_id,
            "enabled": true,
            "targets": origins.iter().map(|o| json!({
                "url": format!("{}://{}:{}", o.scheme, o.address, o.port),
                "weight": o.weight,
                "backup": o.backup,
                "host_header": o.host_header,
                "sni": o.sni,
                "health_check": {
                    "type": o.hc_type,
                    "path": o.hc_path,
                    "interval_secs": o.hc_interval_secs,
                }
            })).collect::<Vec<_>>(),
        }));
        for host in &s.hosts {
            let host = host.trim();
            if host.is_empty() {
                continue;
            }
            routes.push(json!({
                "id": format!("rt-{}-{}", s.id, host.replace('.', "-")),
                "enabled": true,
                "host": host,
                "path_prefix": "/",
                "upstream_id": upstream_id,
                "site_id": s.id,
                "websocket": s.websocket_enabled,
                "xff_trusted_hops": s.xff_trusted_hops,
                "plugins": [{ "name": "nerve-waf", "config": { "mode": "edge", "app_id": s.id } }],
            }));
        }
        if s.tls_mode == "upload" && s.tls_cert.as_ref().map(|c| !c.is_empty()).unwrap_or(false) {
            ssl.push(json!({
                "id": format!("ssl-{}", s.id),
                "hosts": s.hosts,
                "source": "upload",
                "has_cert": true,
                // Private keys are not logged; dataplane only marks cert present; edge fetches ciphertext via another channel.
                "certificate_ref": s.certificate_id.clone().unwrap_or_else(|| format!("upload:{}", s.id)),
            }));
        } else {
            ssl.push(json!({
                "id": format!("ssl-{}", s.id),
                "hosts": s.hosts,
                "source": "acme",
                "cname_fqdn": s.cname_fqdn,
                "status": "pending_or_issued",
            }));
        }
        out_sites.push(json!({
            "id": s.id,
            "name": s.name,
            "hosts": s.hosts,
            "cname_fqdn": s.cname_fqdn,
            "dns_status": s.dns_status,
            "tls_mode": s.tls_mode,
            "websocket_enabled": s.websocket_enabled,
        }));
    }

    let version = (out_sites.len() as u64)
        .wrapping_mul(1000)
        .wrapping_add(routes.len() as u64);

    Ok(DataplaneSnapshot {
        version,
        generated_at: now_rfc3339(),
        cname_zone: cname_zone(),
        edge_ips: edge_ips(),
        sites: out_sites,
        routes,
        upstreams,
        ssl,
    })
}

async fn get_dataplane(State(state): State<Arc<AppState>>) -> Result<Json<Value>, StatusCode> {
    let snap = build_dataplane(&state)?;
    Ok(Json(serde_json::to_value(snap).unwrap_or(json!({}))))
}

async fn sync_dataplane(State(state): State<Arc<AppState>>) -> Result<Json<Value>, StatusCode> {
    // Ensure every cname app has cname_fqdn filled.
    {
        let mut sites = state
            .platform
            .sites
            .write()
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        for site in sites.iter_mut() {
            if site.ingress_mode == "cname" || site.integration == "cname" {
                ensure_cname_fields(site);
            }
        }
    }
    state
        .platform
        .save()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let snap = build_dataplane(&state)?;
    Ok(Json(json!({ "ok": true, "dataplane": snap })))
}

fn l7_upstream_to_gateway(u: &L7Upstream) -> Value {
    let mut u = u.clone();
    u.normalize_checks();
    let nodes: Vec<Value> = u
        .nodes
        .iter()
        .map(|n| {
            json!({
                "host": n.host.trim(),
                "port": n.port,
                "weight": n.weight.max(1),
            })
        })
        .collect();
    let mut out = json!({
        "id": u.id,
        "name": u.name,
        "enabled": u.enabled,
        "scheme": u.scheme,
        "type": u.upstream_type,
        "nodes": nodes,
        "description": u.description,
    });
    if let Some(kind) = u.origin_kind.as_deref().filter(|k| !k.is_empty()) {
        out["origin_kind"] = json!(kind);
    }
    if let Some(checks) = &u.checks {
        if let Ok(v) = serde_json::to_value(checks) {
            out["checks"] = v;
        }
    }
    out
}

fn validate_upstreams(upstreams: &[L7Upstream]) -> Result<(), String> {
    let mut ids = std::collections::HashSet::new();
    for u in upstreams {
        let id = u.id.trim();
        if id.is_empty() {
            return Err("Upstream ID 不能为空".into());
        }
        if !ids.insert(id.to_string()) {
            return Err(format!("重复 Upstream ID: {id}"));
        }
        if u.name.trim().is_empty() {
            return Err(format!("Upstream「{id}」名称不能为空"));
        }
        if !u.is_gateway_origin() && u.nodes.iter().all(|n| n.host.trim().is_empty()) {
            return Err(format!("Upstream「{id}」至少需要一个节点"));
        }
    }
    Ok(())
}

async fn get_waf_upstreams(State(state): State<Arc<AppState>>) -> Result<Json<Value>, StatusCode> {
    let upstreams = state
        .config
        .load_upstreams()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "ok": true, "upstreams": upstreams })))
}

#[derive(Debug, Deserialize)]
struct UpstreamsPutBody {
    upstreams: Vec<L7Upstream>,
    #[serde(default)]
    sync_gateway: Option<bool>,
}

async fn put_waf_upstreams(
    State(state): State<Arc<AppState>>,
    Json(body): Json<UpstreamsPutBody>,
) -> Result<Json<Value>, StatusCode> {
    let mut upstreams = body.upstreams;
    for u in &mut upstreams {
        u.normalize_checks();
    }
    validate_upstreams(&upstreams).map_err(|e| {
        tracing::warn!("upstreams validate: {e}");
        StatusCode::BAD_REQUEST
    })?;
    state
        .config
        .save_upstreams(&upstreams, "console")
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let mut gateway_sync: Option<Value> = None;
    if body.sync_gateway.unwrap_or(false) {
        let items: Vec<Value> = upstreams.iter().map(l7_upstream_to_gateway).collect();
        match gateway_put("/admin/upstreams", &json!({ "items": items })).await {
            Ok(()) => gateway_sync = Some(json!({ "ok": true })),
            Err(e) => gateway_sync = Some(json!({ "ok": false, "error": e })),
        }
    }

    crate::dataplane::bump_after_mutation(&state);
    Ok(Json(json!({
        "ok": true,
        "upstreams": upstreams,
        "gateway_sync": gateway_sync,
    })))
}
