-- 防护应用（Console GET/PUT /api/sites）
-- DATABASE_URL 配置时由 API 读写；多实例/环境共享，不再依赖各节点 platform.json

CREATE TABLE IF NOT EXISTS waf_protected_sites (
    site_id                 TEXT PRIMARY KEY,
    name                    TEXT NOT NULL,
    hosts                   JSONB NOT NULL DEFAULT '[]'::jsonb,
    enabled                 BOOLEAN NOT NULL DEFAULT true,
    body                    JSONB NOT NULL,
    sort_order              INT NOT NULL DEFAULT 0,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by_id_name      TEXT,
    updated_by_id_name      TEXT
);

CREATE INDEX IF NOT EXISTS idx_waf_protected_sites_enabled
    ON waf_protected_sites (enabled);

COMMENT ON TABLE waf_protected_sites IS
    'WAF 防护应用（hosts、ingress、origins）；与 waf_app_protection.site_id 对齐';
