-- 025: 应用私有策略独立表（与全局策略库物理分表）
-- 旧全局表 site_id 列的迁出见 027_migrate_strategy_site_id.sql（psql / apply-migrations.sh）

CREATE TABLE IF NOT EXISTS waf_app_protection_strategies (
    id TEXT PRIMARY KEY,
    site_id TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    semantic_mode TEXT NOT NULL DEFAULT 'standard',
    expert_level TEXT NOT NULL DEFAULT 'medium',
    sort_order INT NOT NULL DEFAULT 100,
    expert JSONB NOT NULL DEFAULT '{"level":"medium"}'::jsonb,
    semantic JSONB NOT NULL DEFAULT '{"mode":"standard"}'::jsonb,
    core JSONB NOT NULL DEFAULT '{}'::jsonb,
    custom JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_waf_app_protection_strategies_site_name
    ON waf_app_protection_strategies (site_id, name);

CREATE INDEX IF NOT EXISTS idx_waf_app_protection_strategies_site
    ON waf_app_protection_strategies (site_id);

DROP INDEX IF EXISTS idx_waf_protection_strategies_scope_name;

CREATE UNIQUE INDEX IF NOT EXISTS idx_waf_protection_strategies_name
    ON waf_protection_strategies (name);

COMMENT ON TABLE waf_protection_strategies IS
    '全局防护策略库（侧栏）；所有应用可选；不含应用私有策略';

COMMENT ON TABLE waf_app_protection_strategies IS
    '应用私有防护策略；仅所属 site_id 可见/可选；与全局策略表物理隔离';
