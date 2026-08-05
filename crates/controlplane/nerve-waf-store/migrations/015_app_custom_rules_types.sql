-- 应用级自定义规则：与全局表对齐，支持插件型规则
ALTER TABLE waf_app_custom_rules ADD COLUMN IF NOT EXISTS rule_type TEXT NOT NULL DEFAULT 'signature';
ALTER TABLE waf_app_custom_rules ADD COLUMN IF NOT EXISTS plugin_name TEXT NOT NULL DEFAULT '';

COMMENT ON COLUMN waf_app_custom_rules.rule_type IS 'signature | plugin';
COMMENT ON COLUMN waf_app_custom_rules.plugin_name IS 'rule_type=plugin 时引用的 L7 插件名';
