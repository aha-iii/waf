-- 024: 应用绑定防护策略（应用级：应用 → 策略 → 模块快照）
CREATE TABLE IF NOT EXISTS waf_app_bound_strategy (
    site_id TEXT PRIMARY KEY,
    strategy_id TEXT NOT NULL DEFAULT 'tpl-standard',
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_by_id_name TEXT NOT NULL DEFAULT ''
);

-- 存量应用默认绑定「标准」
INSERT INTO waf_app_bound_strategy (site_id, strategy_id, updated_by_id_name)
SELECT p.site_id, 'tpl-standard', 'backfill'
FROM waf_app_protection p
WHERE NOT EXISTS (
    SELECT 1 FROM waf_app_bound_strategy b WHERE b.site_id = p.site_id
);

INSERT INTO waf_app_bound_strategy (site_id, strategy_id, updated_by_id_name)
SELECT ap.site_id, 'tpl-standard', 'backfill'
FROM waf_app_pipeline ap
WHERE NOT EXISTS (
    SELECT 1 FROM waf_app_bound_strategy b WHERE b.site_id = ap.site_id
);
