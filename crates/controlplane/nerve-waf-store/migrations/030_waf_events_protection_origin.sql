-- 攻击日志：防护策略 / 防护段 / 规则集（防护来源列）
ALTER TABLE waf_events ADD COLUMN IF NOT EXISTS strategy_id TEXT;
ALTER TABLE waf_events ADD COLUMN IF NOT EXISTS strategy_name TEXT;
ALTER TABLE waf_events ADD COLUMN IF NOT EXISTS protection_stage TEXT;
ALTER TABLE waf_events ADD COLUMN IF NOT EXISTS rule_set TEXT;
ALTER TABLE waf_events ADD COLUMN IF NOT EXISTS plugin_name TEXT;

CREATE INDEX IF NOT EXISTS idx_waf_events_protection_stage
  ON waf_events (protection_stage, ts DESC);
CREATE INDEX IF NOT EXISTS idx_waf_events_strategy_id
  ON waf_events (strategy_id, ts DESC);
