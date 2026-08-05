-- 应用级 CRS 版本绑定与历史快照（与系统 rules-versions/crs 只读关联）

CREATE TABLE IF NOT EXISTS waf_app_crs_binding (
    site_id                 TEXT PRIMARY KEY,
    crs_version_id          TEXT NOT NULL,
    crs_version_label       TEXT,
    rule_count              INT NOT NULL DEFAULT 0,
    applied_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    applied_by_id_name      TEXT
);

CREATE INDEX IF NOT EXISTS idx_waf_app_crs_binding_version
    ON waf_app_crs_binding (crs_version_id);

CREATE TABLE IF NOT EXISTS waf_app_crs_history (
    waf_app_crs_history_id  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    site_id                 TEXT NOT NULL,
    crs_version_id          TEXT NOT NULL,
    crs_version_label       TEXT,
    rule_count              INT NOT NULL DEFAULT 0,
    modules_snapshot        JSONB NOT NULL DEFAULT '{}',
    is_active               BOOLEAN NOT NULL DEFAULT false,
    note                    TEXT,
    applied_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    applied_by_id_name      TEXT
);

CREATE INDEX IF NOT EXISTS idx_waf_app_crs_history_site
    ON waf_app_crs_history (site_id, applied_at DESC);

COMMENT ON TABLE waf_app_crs_binding IS '应用当前生效的 CRS 版本（指向 data/rules-versions/crs/{id}）';
COMMENT ON TABLE waf_app_crs_history IS '应用 CRS 版本切换历史（含模块配置与 disabled_rules 快照）';
