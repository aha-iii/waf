-- 自定义规则：支持未绑定规则集、插件型规则
ALTER TABLE waf_custom_rules ADD COLUMN IF NOT EXISTS rule_type TEXT NOT NULL DEFAULT 'signature';
ALTER TABLE waf_custom_rules ADD COLUMN IF NOT EXISTS plugin_name TEXT NOT NULL DEFAULT '';

ALTER TABLE waf_custom_rules DROP CONSTRAINT IF EXISTS waf_custom_rules_rule_set_id_fkey;
ALTER TABLE waf_custom_rules ALTER COLUMN rule_set_id DROP NOT NULL;
ALTER TABLE waf_custom_rules ADD CONSTRAINT waf_custom_rules_rule_set_id_fkey
    FOREIGN KEY (rule_set_id) REFERENCES waf_custom_rule_sets(id) ON DELETE SET NULL;

ALTER TABLE waf_custom_rules ALTER COLUMN match_field SET DEFAULT '';
ALTER TABLE waf_custom_rules ALTER COLUMN match_op SET DEFAULT '';
ALTER TABLE waf_custom_rules ALTER COLUMN match_pattern SET DEFAULT '';

COMMENT ON COLUMN waf_custom_rules.rule_type IS 'signature | plugin';
COMMENT ON COLUMN waf_custom_rules.plugin_name IS 'rule_type=plugin 时引用的 L7 插件名';
