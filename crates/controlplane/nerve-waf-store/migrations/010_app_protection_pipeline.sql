-- 应用级 Web 防护配置（按 site_id 隔离；主键 UUID + 标准审计字段）
CREATE TABLE IF NOT EXISTS waf_app_protection (
    waf_app_protection_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    site_id                 TEXT NOT NULL UNIQUE,
    version                 INT NOT NULL DEFAULT 1,
    mode                    TEXT NOT NULL DEFAULT 'block',
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by_id           UUID,
    created_by_id_name      TEXT,
    updated_by_id           UUID,
    updated_by_id_name      TEXT
);

CREATE INDEX IF NOT EXISTS idx_waf_app_protection_site
    ON waf_app_protection (site_id);

CREATE TABLE IF NOT EXISTS waf_app_protection_modules (
    waf_app_protection_modules_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    site_id                 TEXT NOT NULL REFERENCES waf_app_protection(site_id) ON DELETE CASCADE,
    module_key              TEXT NOT NULL,
    enabled                 BOOLEAN NOT NULL DEFAULT true,
    action                  TEXT NOT NULL DEFAULT 'block',
    sensitivity             TEXT NOT NULL DEFAULT 'medium',
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by_id           UUID,
    created_by_id_name      TEXT,
    updated_by_id           UUID,
    updated_by_id_name      TEXT,
    UNIQUE (site_id, module_key)
);

CREATE INDEX IF NOT EXISTS idx_waf_app_protection_modules_site
    ON waf_app_protection_modules (site_id);

CREATE TABLE IF NOT EXISTS waf_app_protection_disabled_rules (
    waf_app_protection_disabled_rules_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    site_id                 TEXT NOT NULL,
    module_key              TEXT NOT NULL,
    rule_id                 TEXT NOT NULL,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by_id           UUID,
    created_by_id_name      TEXT,
    updated_by_id           UUID,
    updated_by_id_name      TEXT,
    UNIQUE (site_id, module_key, rule_id),
    FOREIGN KEY (site_id, module_key)
        REFERENCES waf_app_protection_modules(site_id, module_key) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_waf_app_protection_disabled_site
    ON waf_app_protection_disabled_rules (site_id, module_key);

-- 应用级检测流水线
CREATE TABLE IF NOT EXISTS waf_app_pipeline (
    waf_app_pipeline_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    site_id                 TEXT NOT NULL UNIQUE,
    enabled                 BOOLEAN NOT NULL DEFAULT true,
    semantic_mode           TEXT NOT NULL DEFAULT 'standard',
    intent_enabled          BOOLEAN NOT NULL DEFAULT true,
    semantic_first          BOOLEAN NOT NULL DEFAULT true,
    libinjection            BOOLEAN NOT NULL DEFAULT true,
    sql_intent              BOOLEAN NOT NULL DEFAULT true,
    fallback_plain_libinjection BOOLEAN NOT NULL DEFAULT true,
    decode                  JSONB NOT NULL DEFAULT '{}',
    scope                   JSONB NOT NULL DEFAULT '{}',
    whitelist               JSONB NOT NULL DEFAULT '{}',
    performance             JSONB NOT NULL DEFAULT '{}',
    auto_tune               JSONB NOT NULL DEFAULT '{}',
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by_id           UUID,
    created_by_id_name      TEXT,
    updated_by_id           UUID,
    updated_by_id_name      TEXT
);

CREATE INDEX IF NOT EXISTS idx_waf_app_pipeline_site
    ON waf_app_pipeline (site_id);

CREATE TABLE IF NOT EXISTS waf_app_pipeline_stages (
    waf_app_pipeline_stages_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    site_id                 TEXT NOT NULL REFERENCES waf_app_pipeline(site_id) ON DELETE CASCADE,
    stage_id                TEXT NOT NULL,
    sort_order              INT NOT NULL DEFAULT 0,
    enabled                 BOOLEAN NOT NULL DEFAULT true,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by_id           UUID,
    created_by_id_name      TEXT,
    updated_by_id           UUID,
    updated_by_id_name      TEXT,
    UNIQUE (site_id, stage_id)
);

CREATE INDEX IF NOT EXISTS idx_waf_app_pipeline_stages_order
    ON waf_app_pipeline_stages (site_id, sort_order);

CREATE TABLE IF NOT EXISTS waf_app_pipeline_editor (
    waf_app_pipeline_editor_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    site_id                 TEXT NOT NULL UNIQUE REFERENCES waf_app_pipeline(site_id) ON DELETE CASCADE,
    editor_mode             TEXT NOT NULL DEFAULT 'flow',
    layout                  JSONB NOT NULL DEFAULT '{}',
    graph                   JSONB,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by_id           UUID,
    created_by_id_name      TEXT,
    updated_by_id           UUID,
    updated_by_id_name      TEXT
);
