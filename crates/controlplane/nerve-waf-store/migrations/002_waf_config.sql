-- L3 网络策略
CREATE TABLE IF NOT EXISTS waf_l3_policies (
    id            TEXT PRIMARY KEY,
    site_id       TEXT NOT NULL DEFAULT 'default',
    name          TEXT NOT NULL,
    priority      INT NOT NULL DEFAULT 100,
    enabled       BOOLEAN NOT NULL DEFAULT true,
    src_ip        TEXT,
    dst_ip        TEXT,
    dst_port      INT,
    pod           TEXT,
    description   TEXT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_by    TEXT
);

CREATE INDEX IF NOT EXISTS idx_l3_policies_site ON waf_l3_policies (site_id, enabled, priority);

CREATE TABLE IF NOT EXISTS waf_l3_policy_plugins (
    id            BIGSERIAL PRIMARY KEY,
    policy_id     TEXT NOT NULL REFERENCES waf_l3_policies(id) ON DELETE CASCADE,
    plugin_name   TEXT NOT NULL,
    enabled       BOOLEAN NOT NULL DEFAULT true,
    sort_order    INT NOT NULL DEFAULT 0,
    config        JSONB NOT NULL DEFAULT '{}',
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (policy_id, plugin_name, sort_order)
);

CREATE INDEX IF NOT EXISTS idx_l3_plugins_policy ON waf_l3_policy_plugins (policy_id);

-- L7 HTTP 路由
CREATE TABLE IF NOT EXISTS waf_l7_routes (
    id            TEXT PRIMARY KEY,
    site_id       TEXT NOT NULL DEFAULT 'default',
    name          TEXT NOT NULL,
    priority      INT NOT NULL DEFAULT 100,
    enabled       BOOLEAN NOT NULL DEFAULT true,
    host          TEXT,
    path_prefix   TEXT,
    methods       TEXT[],
    uri_regex     TEXT,
    description   TEXT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_by    TEXT
);

CREATE INDEX IF NOT EXISTS idx_l7_routes_site ON waf_l7_routes (site_id, enabled, priority);

CREATE TABLE IF NOT EXISTS waf_l7_route_plugins (
    id            BIGSERIAL PRIMARY KEY,
    route_id      TEXT NOT NULL REFERENCES waf_l7_routes(id) ON DELETE CASCADE,
    plugin_name   TEXT NOT NULL,
    enabled       BOOLEAN NOT NULL DEFAULT true,
    sort_order    INT NOT NULL DEFAULT 0,
    config        JSONB NOT NULL DEFAULT '{}',
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (route_id, plugin_name, sort_order)
);

CREATE INDEX IF NOT EXISTS idx_l7_plugins_route ON waf_l7_route_plugins (route_id);

-- L7 全局防护模块
CREATE TABLE IF NOT EXISTS waf_l7_modules (
    site_id       TEXT PRIMARY KEY DEFAULT 'default',
    version       INT NOT NULL DEFAULT 1,
    mode          TEXT NOT NULL DEFAULT 'block',
    modules       JSONB NOT NULL,
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_by    TEXT
);

-- 自定义规则集
CREATE TABLE IF NOT EXISTS waf_custom_rule_sets (
    id            TEXT PRIMARY KEY,
    site_id       TEXT NOT NULL DEFAULT 'default',
    name          TEXT NOT NULL,
    description   TEXT,
    enabled       BOOLEAN NOT NULL DEFAULT true,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS waf_custom_rules (
    id            TEXT PRIMARY KEY,
    rule_set_id   TEXT NOT NULL REFERENCES waf_custom_rule_sets(id) ON DELETE CASCADE,
    name          TEXT NOT NULL,
    phase         TEXT NOT NULL DEFAULT 'request',
    priority      INT NOT NULL DEFAULT 100,
    enabled       BOOLEAN NOT NULL DEFAULT true,
    match_field   TEXT NOT NULL,
    match_op      TEXT NOT NULL,
    match_pattern TEXT NOT NULL,
    action        TEXT NOT NULL DEFAULT 'block',
    severity      TEXT NOT NULL DEFAULT 'medium',
    category      TEXT NOT NULL DEFAULT 'custom',
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_custom_rules_set ON waf_custom_rules (rule_set_id, enabled, priority);

-- 运行时封禁 overlay
CREATE TABLE IF NOT EXISTS waf_runtime_denies (
    ip            INET PRIMARY KEY,
    site_id       TEXT NOT NULL DEFAULT 'default',
    source        TEXT NOT NULL,
    reason        TEXT,
    expires_at    TIMESTAMPTZ,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by    TEXT
);

CREATE INDEX IF NOT EXISTS idx_runtime_denies_expires
    ON waf_runtime_denies (expires_at)
    WHERE expires_at IS NOT NULL;

-- 配置审计
CREATE TABLE IF NOT EXISTS waf_config_revisions (
    id            BIGSERIAL PRIMARY KEY,
    site_id       TEXT NOT NULL DEFAULT 'default',
    kind          TEXT NOT NULL,
    resource_id   TEXT NOT NULL,
    revision      INT NOT NULL,
    operation     TEXT NOT NULL,
    payload       JSONB NOT NULL,
    diff          JSONB,
    operator      TEXT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (kind, resource_id, revision)
);

CREATE INDEX IF NOT EXISTS idx_waf_config_revisions_kind ON waf_config_revisions (kind, resource_id, created_at DESC);
