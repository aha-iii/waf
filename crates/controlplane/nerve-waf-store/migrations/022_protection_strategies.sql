-- 命名防护策略模板库（「生产-标准」「开发-宽松」等）
CREATE TABLE IF NOT EXISTS waf_protection_strategies (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    semantic_mode TEXT NOT NULL DEFAULT 'standard',
    expert_level TEXT NOT NULL DEFAULT 'medium',
    builtin BOOLEAN NOT NULL DEFAULT FALSE,
    sort_order INT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_waf_protection_strategies_name
    ON waf_protection_strategies (name);

COMMENT ON TABLE waf_protection_strategies IS '防护策略命名模板库；套用时写入全局/应用 semantic+expert';
