-- 系统级配置与 waf_app_* 应用覆盖层分离
-- 全局：waf_protection / waf_pipeline / waf_custom_*（已有）
-- 应用：waf_app_protection / waf_app_pipeline / waf_app_custom_*（本迁移新增 app custom）

-- ========== 系统级 Web 核心防护（单份，侧边栏「WAF 配置」）==========
CREATE TABLE IF NOT EXISTS waf_protection (
    id                      SMALLINT PRIMARY KEY DEFAULT 1 CHECK (id = 1),
    version                 INT NOT NULL DEFAULT 1,
    mode                    TEXT NOT NULL DEFAULT 'block',
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by_id           UUID,
    created_by_id_name      TEXT,
    updated_by_id           UUID,
    updated_by_id_name      TEXT
);

CREATE TABLE IF NOT EXISTS waf_protection_modules (
    waf_protection_modules_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    module_key              TEXT NOT NULL UNIQUE,
    enabled                 BOOLEAN NOT NULL DEFAULT true,
    action                  TEXT NOT NULL DEFAULT 'block',
    sensitivity             TEXT NOT NULL DEFAULT 'medium',
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by_id           UUID,
    created_by_id_name      TEXT,
    updated_by_id           UUID,
    updated_by_id_name      TEXT
);

CREATE TABLE IF NOT EXISTS waf_protection_disabled_rules (
    waf_protection_disabled_rules_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    module_key              TEXT NOT NULL REFERENCES waf_protection_modules(module_key) ON DELETE CASCADE,
    rule_id                 TEXT NOT NULL,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by_id           UUID,
    created_by_id_name      TEXT,
    updated_by_id           UUID,
    updated_by_id_name      TEXT,
    UNIQUE (module_key, rule_id)
);

-- ========== 系统级语义 / 检测流水线（侧边栏「语义设置」）==========
CREATE TABLE IF NOT EXISTS waf_pipeline (
    id                      SMALLINT PRIMARY KEY DEFAULT 1 CHECK (id = 1),
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

CREATE TABLE IF NOT EXISTS waf_pipeline_stages (
    waf_pipeline_stages_id  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    stage_id                TEXT NOT NULL UNIQUE,
    sort_order              INT NOT NULL DEFAULT 0,
    enabled                 BOOLEAN NOT NULL DEFAULT true,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by_id           UUID,
    created_by_id_name      TEXT,
    updated_by_id           UUID,
    updated_by_id_name      TEXT
);

CREATE INDEX IF NOT EXISTS idx_waf_pipeline_stages_order ON waf_pipeline_stages (sort_order);

CREATE TABLE IF NOT EXISTS waf_pipeline_editor (
    id                      SMALLINT PRIMARY KEY DEFAULT 1 CHECK (id = 1),
    editor_mode             TEXT NOT NULL DEFAULT 'list',
    layout                  JSONB NOT NULL DEFAULT '{}',
    graph                   JSONB,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by_id           UUID,
    created_by_id_name      TEXT,
    updated_by_id           UUID,
    updated_by_id_name      TEXT
);

-- ========== 应用级自定义规则（与 waf_custom_* 系统规则库分离）==========
CREATE TABLE IF NOT EXISTS waf_app_custom_rule_sets (
    waf_app_custom_rule_sets_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    site_id                 TEXT NOT NULL,
    set_id                  TEXT NOT NULL,
    name                    TEXT NOT NULL,
    description             TEXT,
    enabled                 BOOLEAN NOT NULL DEFAULT true,
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by_id           UUID,
    created_by_id_name      TEXT,
    updated_by_id           UUID,
    updated_by_id_name      TEXT,
    UNIQUE (site_id, set_id)
);

CREATE INDEX IF NOT EXISTS idx_waf_app_custom_rule_sets_site
    ON waf_app_custom_rule_sets (site_id);

CREATE TABLE IF NOT EXISTS waf_app_custom_rules (
    waf_app_custom_rules_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    site_id                 TEXT NOT NULL,
    rule_id                 TEXT NOT NULL,
    rule_set_id             TEXT NOT NULL,
    name                    TEXT NOT NULL,
    phase                   TEXT NOT NULL DEFAULT 'request',
    priority                INT NOT NULL DEFAULT 100,
    enabled                 BOOLEAN NOT NULL DEFAULT true,
    match_field             TEXT NOT NULL,
    match_op                TEXT NOT NULL,
    match_pattern           TEXT NOT NULL,
    action                  TEXT NOT NULL DEFAULT 'block',
    severity                TEXT NOT NULL DEFAULT 'medium',
    category                TEXT NOT NULL DEFAULT 'custom',
    created_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by_id           UUID,
    created_by_id_name      TEXT,
    updated_by_id           UUID,
    updated_by_id_name      TEXT,
    UNIQUE (site_id, rule_id),
    FOREIGN KEY (site_id, rule_set_id)
        REFERENCES waf_app_custom_rule_sets(site_id, set_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_waf_app_custom_rules_site_set
    ON waf_app_custom_rules (site_id, rule_set_id, enabled, priority);

COMMENT ON TABLE waf_protection IS '系统级 Web 核心防护默认模板（单份，非应用）';
COMMENT ON TABLE waf_pipeline IS '系统级 NSP 语义 / 检测流水线默认（单份，非应用）';
COMMENT ON TABLE waf_custom_rule_sets IS '系统级自定义规则库（侧边栏「自定义规则」，全局共享）';
COMMENT ON TABLE waf_app_protection IS '应用级防护覆盖（仅差异，不含系统 default 行）';
COMMENT ON TABLE waf_app_pipeline IS '应用级流水线覆盖（仅差异）';
COMMENT ON TABLE waf_app_custom_rule_sets IS '应用级自定义规则（仅本应用 Host）';

-- ========== 从 waf_app_* site_id=default 迁到全局表（幂等）==========
INSERT INTO waf_protection (id, version, mode, created_by_id_name, updated_by_id_name)
SELECT 1, version, mode, created_by_id_name, updated_by_id_name
FROM waf_app_protection WHERE site_id = 'default'
ON CONFLICT (id) DO UPDATE SET
  version = EXCLUDED.version,
  mode = EXCLUDED.mode,
  updated_at = now(),
  updated_by_id_name = EXCLUDED.updated_by_id_name;

INSERT INTO waf_protection_modules (module_key, enabled, action, sensitivity, created_by_id_name, updated_by_id_name)
SELECT module_key, enabled, action, sensitivity, created_by_id_name, updated_by_id_name
FROM waf_app_protection_modules WHERE site_id = 'default'
ON CONFLICT (module_key) DO UPDATE SET
  enabled = EXCLUDED.enabled,
  action = EXCLUDED.action,
  sensitivity = EXCLUDED.sensitivity,
  updated_at = now(),
  updated_by_id_name = EXCLUDED.updated_by_id_name;

INSERT INTO waf_protection_disabled_rules (module_key, rule_id, created_by_id_name, updated_by_id_name)
SELECT module_key, rule_id, created_by_id_name, updated_by_id_name
FROM waf_app_protection_disabled_rules WHERE site_id = 'default'
ON CONFLICT (module_key, rule_id) DO NOTHING;

INSERT INTO waf_pipeline (
    id, enabled, semantic_mode, intent_enabled, semantic_first, libinjection, sql_intent,
    fallback_plain_libinjection, decode, scope, whitelist, performance, auto_tune,
    created_by_id_name, updated_by_id_name
)
SELECT 1, enabled, semantic_mode, intent_enabled, semantic_first, libinjection, sql_intent,
       fallback_plain_libinjection, decode, scope, whitelist, performance, auto_tune,
       created_by_id_name, updated_by_id_name
FROM waf_app_pipeline WHERE site_id = 'default'
ON CONFLICT (id) DO UPDATE SET
  enabled = EXCLUDED.enabled,
  semantic_mode = EXCLUDED.semantic_mode,
  intent_enabled = EXCLUDED.intent_enabled,
  semantic_first = EXCLUDED.semantic_first,
  libinjection = EXCLUDED.libinjection,
  sql_intent = EXCLUDED.sql_intent,
  fallback_plain_libinjection = EXCLUDED.fallback_plain_libinjection,
  decode = EXCLUDED.decode,
  scope = EXCLUDED.scope,
  whitelist = EXCLUDED.whitelist,
  performance = EXCLUDED.performance,
  auto_tune = EXCLUDED.auto_tune,
  updated_at = now(),
  updated_by_id_name = EXCLUDED.updated_by_id_name;

INSERT INTO waf_pipeline_stages (stage_id, sort_order, enabled, created_by_id_name, updated_by_id_name)
SELECT stage_id, sort_order, enabled, created_by_id_name, updated_by_id_name
FROM waf_app_pipeline_stages WHERE site_id = 'default'
ON CONFLICT (stage_id) DO UPDATE SET
  sort_order = EXCLUDED.sort_order,
  enabled = EXCLUDED.enabled,
  updated_at = now(),
  updated_by_id_name = EXCLUDED.updated_by_id_name;

INSERT INTO waf_pipeline_editor (id, editor_mode, layout, graph, created_by_id_name, updated_by_id_name)
SELECT 1, editor_mode, layout, graph, created_by_id_name, updated_by_id_name
FROM waf_app_pipeline_editor WHERE site_id = 'default'
ON CONFLICT (id) DO UPDATE SET
  editor_mode = EXCLUDED.editor_mode,
  layout = EXCLUDED.layout,
  graph = EXCLUDED.graph,
  updated_at = now(),
  updated_by_id_name = EXCLUDED.updated_by_id_name;

-- 清理：系统 default 不应再占用 waf_app_* 表
DELETE FROM waf_app_protection WHERE site_id = 'default';

-- 兼容：waf_l7_modules 仅保留 default 镜像
DELETE FROM waf_l7_modules WHERE site_id <> 'default';
