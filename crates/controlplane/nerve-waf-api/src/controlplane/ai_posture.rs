//! 防护 posture 巡检 — 各站点配置健康度、FP 率、模块覆盖

use axum::{
    extract::{Query, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use chrono::{Duration, Utc};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::sync::Arc;

use crate::AppState;

pub fn posture_router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/posture/scan", get(get_scan).post(run_scan))
        .route("/api/posture/history", get(scan_history))
}

#[derive(Debug, Deserialize)]
struct ScanQuery {
    #[serde(default)]
    site_id: Option<String>,
    #[serde(default)]
    persist: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct HistoryQuery {
    #[serde(default)]
    site_id: Option<String>,
    #[serde(default = "default_hist_limit")]
    limit: i64,
}

fn default_hist_limit() -> i64 {
    20
}

#[derive(Debug, Clone, serde::Serialize)]
struct PostureFinding {
    severity: String,
    code: String,
    message: String,
    site_id: Option<String>,
    recommendation: String,
}

#[derive(Debug, Clone, serde::Serialize)]
struct PostureCheck {
    site_id: String,
    category: String,
    code: String,
    label: String,
    status: String,
    detail: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    items: Vec<Value>,
}

fn disabled_rule_items(modules: &nerve_waf_sdk::ProtectionModules) -> Vec<Value> {
    let mut out: Vec<Value> = modules
        .modules
        .iter()
        .flat_map(|(mod_key, cfg)| {
            let mod_key = mod_key.clone();
            cfg.disabled_rules.iter().map(move |rid| {
                json!({
                    "kind": "disabled_rule",
                    "module": mod_key,
                    "rule_id": rid,
                })
            })
        })
        .collect();
    out.sort_by(|a, b| {
        let ma = a.get("module").and_then(|v| v.as_str()).unwrap_or("");
        let ra = a.get("rule_id").and_then(|v| v.as_str()).unwrap_or("");
        let mb = b.get("module").and_then(|v| v.as_str()).unwrap_or("");
        let rb = b.get("rule_id").and_then(|v| v.as_str()).unwrap_or("");
        ma.cmp(mb).then(ra.cmp(rb))
    });
    out
}

fn fp_rate_items(confirmed_fp: i64, blocked: i64, top_rules: &[nerve_waf_store::FpRuleCount]) -> Vec<Value> {
    let rate = if blocked > 0 {
        confirmed_fp as f64 / blocked as f64 * 100.0
    } else {
        0.0
    };
    let mut items = vec![
        json!({ "kind": "stat", "label": "确认误报", "value": confirmed_fp.to_string() }),
        json!({ "kind": "stat", "label": "拦截样本", "value": blocked.to_string() }),
        json!({ "kind": "stat", "label": "误报率", "value": format!("{rate:.2}%") }),
    ];
    for r in top_rules {
        items.push(json!({
            "kind": "fp_rule",
            "rule_id": r.rule_id,
            "count": r.count,
        }));
    }
    items
}

async fn get_scan(
    State(state): State<Arc<AppState>>,
    Query(q): Query<ScanQuery>,
) -> Result<Json<Value>, StatusCode> {
    let report = build_posture_report(&state, q.site_id.as_deref(), true).await?;
    Ok(Json(report))
}

async fn run_scan(
    State(state): State<Arc<AppState>>,
    Query(q): Query<ScanQuery>,
) -> Result<Json<Value>, StatusCode> {
    let mut report = build_posture_report(&state, q.site_id.as_deref(), true).await?;
    if q.persist.unwrap_or(true) {
        if let Some(pool) = state.events.postgres_pool() {
            let score = report
                .get("overall_score")
                .and_then(|v| v.as_i64())
                .unwrap_or(0) as i32;
            let findings = report.get("findings").cloned().unwrap_or(json!([]));
            let summary = report.get("summary").cloned().unwrap_or(json!({}));
            if let Ok(scan_id) = nerve_waf_store::insert_posture_scan(
                &pool,
                q.site_id.as_deref(),
                "manual",
                score,
                &findings,
                &summary,
            )
            .await
            {
                if let Some(obj) = report.as_object_mut() {
                    obj.insert("scan_id".into(), json!(scan_id));
                }
            }
        }
    }
    Ok(Json(report))
}

async fn scan_history(
    State(state): State<Arc<AppState>>,
    Query(q): Query<HistoryQuery>,
) -> Result<Json<Value>, StatusCode> {
    let Some(pool) = state.events.postgres_pool() else {
        return Ok(Json(json!({ "ok": true, "scans": [] })));
    };
    let scans = nerve_waf_store::receet_posture_scans(&pool, q.site_id.as_deref(), q.limit)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(json!({ "ok": true, "scans": scans })))
}

async fn build_posture_report(
    state: &AppState,
    site_filter: Option<&str>,
    include_details: bool,
) -> Result<Value, StatusCode> {
    let sites = resolve_site_ids(state, site_filter).await?;
    let since = Utc::now() - Duration::days(7);
    let mut findings: Vec<PostureFinding> = Vec::new();
    let mut checks: Vec<PostureCheck> = Vec::new();
    let mut site_scores: HashMap<String, i32> = HashMap::new();
    let mut total_disabled_rules = 0i64;
    let mut modules_off = 0i64;

    for site_id in &sites {
        let mut score: i32 = 100;
        let modules = state
            .config
            .load_modules_for(site_id)
            .await
            .unwrap_or_default();
        let sem = state.ai_settings.semantic_config_for(site_id).normalized();
        if !sem.enabled {
            findings.push(PostureFinding {
                severity: "medium".into(),
                code: "semantic_disabled".into(),
                message: format!("应用 {site_id} 语义增强未启用"),
                site_id: Some(site_id.clone()),
                recommendation: "在语义设置中启用 NSP，或检查应用级覆盖".into(),
            });
            checks.push(PostureCheck {
                site_id: site_id.clone(),
                category: "semantic".into(),
                code: "semantic_enabled".into(),
                label: "语义增强 (NSP)".into(),
                status: "warn".into(),
                detail: "未启用".into(),
                items: vec![json!({
                    "kind": "hint",
                    "text": "语义增强未开启，核心模块仍依赖规则匹配；建议在「防护设置」中启用 NSP。"
                })],
            });
            score -= 15;
        } else {
            checks.push(PostureCheck {
                site_id: site_id.clone(),
                category: "semantic".into(),
                code: "semantic_enabled".into(),
                label: "语义增强 (NSP)".into(),
                status: "pass".into(),
                detail: "已启用".into(),
                items: vec![],
            });
        }
        let core_keys = ["crs", "sqli", "xss", "rce", "path_traversal"];
        for key in core_keys {
            let cfg = modules.modules.get(key);
            let enabled = cfg.map(|c| c.enabled).unwrap_or(true);
            if !enabled {
                modules_off += 1;
                findings.push(PostureFinding {
                    severity: "high".into(),
                    code: "core_module_off".into(),
                    message: format!("应用 {site_id} 核心模块 {key} 已关闭"),
                    site_id: Some(site_id.clone()),
                    recommendation: "恢复核心防护模块或说明业务豁免原因".into(),
                });
                checks.push(PostureCheck {
                    site_id: site_id.clone(),
                    category: "module".into(),
                    code: format!("core_{key}"),
                    label: format!("核心模块 · {key}"),
                    status: "fail".into(),
                    detail: "已关闭".into(),
                    items: vec![json!({
                        "kind": "module",
                        "module": key,
                        "status": "off",
                    })],
                });
                score -= 20;
            } else {
                checks.push(PostureCheck {
                    site_id: site_id.clone(),
                    category: "module".into(),
                    code: format!("core_{key}"),
                    label: format!("核心模块 · {key}"),
                    status: "pass".into(),
                    detail: "已启用".into(),
                    items: vec![],
                });
            }
        }
        let disabled_items = disabled_rule_items(&modules);
        let disabled: i64 = disabled_items.len() as i64;
        total_disabled_rules += disabled;
        if disabled > 10 {
            findings.push(PostureFinding {
                severity: "medium".into(),
                code: "many_disabled_rules".into(),
                message: format!("应用 {site_id} 已禁用 {disabled} 条规则"),
                site_id: Some(site_id.clone()),
                recommendation: "复核禁用规则列表，避免长期放宽防护".into(),
            });
            checks.push(PostureCheck {
                site_id: site_id.clone(),
                category: "rules".into(),
                code: "disabled_rules".into(),
                label: "规则禁用数量".into(),
                status: "warn".into(),
                detail: format!("已禁用 {disabled} 条（阈值 10）"),
                items: disabled_items,
            });
            score -= 10;
        } else {
            checks.push(PostureCheck {
                site_id: site_id.clone(),
                category: "rules".into(),
                code: "disabled_rules".into(),
                label: "规则禁用数量".into(),
                status: "pass".into(),
                detail: if disabled > 0 {
                    format!("已禁用 {disabled} 条，在阈值内")
                } else {
                    "无禁用规则".into()
                },
                items: disabled_items,
            });
        }
        if let Some(pool) = state.events.postgres_pool() {
            if let Ok(fp) = nerve_waf_store::fp_analytics(&pool, since, Some(site_id)).await {
                let blocked = state
                    .events
                    .stats_since(since)
                    .await
                    .map(|s| s.blocked as i64)
                    .unwrap_or(0);
                let fp_items = fp_rate_items(fp.confirmed_fp, blocked, &fp.top_rules);
                if blocked == 0 {
                    checks.push(PostureCheck {
                        site_id: site_id.clone(),
                        category: "fp".into(),
                        code: "fp_rate_7d".into(),
                        label: "近 7 天误报率".into(),
                        status: "skip".into(),
                        detail: "无拦截样本，跳过评估".into(),
                        items: vec![json!({
                            "kind": "hint",
                            "text": "近 7 天无拦截事件，无法计算误报率。"
                        })],
                    });
                } else if fp.confirmed_fp > 0 {
                    let rate = fp.confirmed_fp as f64 / blocked as f64 * 100.0;
                    if rate > 5.0 {
                        findings.push(PostureFinding {
                            severity: "medium".into(),
                            code: "high_fp_rate".into(),
                            message: format!(
                                "应用 {site_id} 近 7 天误报率约 {rate:.1}%（{}/{}）",
                                fp.confirmed_fp, blocked
                            ),
                            site_id: Some(site_id.clone()),
                            recommendation: "使用误报 Copilot 加白或调语义模式".into(),
                        });
                        checks.push(PostureCheck {
                            site_id: site_id.clone(),
                            category: "fp".into(),
                            code: "fp_rate_7d".into(),
                            label: "近 7 天误报率".into(),
                            status: "warn".into(),
                            detail: format!("{rate:.1}%（{}/{} 拦截）", fp.confirmed_fp, blocked),
                            items: fp_items,
                        });
                        score -= 15;
                    } else {
                        checks.push(PostureCheck {
                            site_id: site_id.clone(),
                            category: "fp".into(),
                            code: "fp_rate_7d".into(),
                            label: "近 7 天误报率".into(),
                            status: "pass".into(),
                            detail: format!("{rate:.1}%（{}/{} 拦截）", fp.confirmed_fp, blocked),
                            items: fp_items,
                        });
                    }
                } else {
                    checks.push(PostureCheck {
                        site_id: site_id.clone(),
                        category: "fp".into(),
                        code: "fp_rate_7d".into(),
                        label: "近 7 天误报率".into(),
                        status: "pass".into(),
                        detail: format!("0%（0/{} 拦截）", blocked),
                        items: fp_items,
                    });
                }
            }
        } else {
            checks.push(PostureCheck {
                site_id: site_id.clone(),
                category: "fp".into(),
                code: "fp_rate_7d".into(),
                label: "近 7 天误报率".into(),
                status: "skip".into(),
                detail: "未配置 PostgreSQL，跳过 FP 统计".into(),
                items: vec![json!({
                    "kind": "hint",
                    "text": "需要 PostgreSQL 持久化方可统计误报确认记录。"
                })],
            });
        }
        site_scores.insert(site_id.clone(), score.clamp(0, 100));
    }

    let overall = if site_scores.is_empty() {
        0
    } else {
        site_scores.values().sum::<i32>() / site_scores.len() as i32
    };

    let mut out = json!({
        "ok": true,
        "scanned_at": Utc::now().to_rfc3339(),
        "window_days": 7,
        "overall_score": overall,
        "site_scores": site_scores,
        "findings": findings,
        "checks": checks,
        "scope": {
            "site_ids": sites,
            "dimensions": [
                "语义增强引擎 (NSP)",
                "核心规则引擎 · crs/sqli/xss/rce/path_traversal",
                "规则禁用与策略漂移",
                "误报趋势（需 PostgreSQL + 拦截样本）",
            ],
        },
        "summary": {
            "sites_scanned": sites.len(),
            "findings_count": findings.len(),
            "checks_count": checks.len(),
            "checks_pass": checks.iter().filter(|c| c.status == "pass").count(),
            "modules_off": modules_off,
            "total_disabled_rules": total_disabled_rules,
        },
        "ai_policy": {
            "hot_path_ai_block": false,
            "model_assist_only": true,
            "changes_require_confirm": true,
            "shadow_before_prod": true,
        },
    });
    if include_details {
        out["engram"] = json!({
            "detect_model": std::env::var("NERVE_ENGRAM_URL")
                .ok()
                .is_some_and(|u| !u.trim().is_empty()),
            "llm_explain": state.ai_settings.triage_llm_config().enabled,
        });
    }
    Ok(out)
}

async fn resolve_site_ids(state: &AppState, filter: Option<&str>) -> Result<Vec<String>, StatusCode> {
    if let Some(s) = filter.filter(|x| !x.trim().is_empty()) {
        return Ok(vec![s.trim().to_string()]);
    }
    let sites = state.platform.list_sites();
    if !sites.is_empty() {
        return Ok(sites.iter().map(|s| s.id.clone()).collect());
    }
    Ok(vec!["default".into()])
}
