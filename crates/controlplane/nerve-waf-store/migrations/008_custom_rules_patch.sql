-- 兼容旧库：waf_custom_rule_sets 可能缺少 site_id / description（002 早期或未完整执行）
ALTER TABLE waf_custom_rule_sets ADD COLUMN IF NOT EXISTS site_id TEXT NOT NULL DEFAULT 'default';
ALTER TABLE waf_custom_rule_sets ADD COLUMN IF NOT EXISTS description TEXT;

UPDATE waf_custom_rule_sets SET site_id = 'default' WHERE site_id IS NULL OR site_id = '';
