-- 系统自定义规则库 scope 与内置 default 应用分离（global = 侧边栏「自定义规则」）
UPDATE waf_custom_rule_sets
SET site_id = 'global'
WHERE site_id IS NULL OR site_id = '' OR site_id = 'default';

COMMENT ON COLUMN waf_custom_rule_sets.site_id IS '系统规则库 scope，固定 global；应用副本见 waf_app_custom_*';
