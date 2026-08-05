-- 028: 历史库数据补齐（非 DDL；由 apply-migrations 命令执行，应用不跑）
-- 原 connect_pool 内嵌 backfill，现迁出与应用解耦

UPDATE waf_custom_rule_sets
SET site_id = 'global'
WHERE site_id IS NULL OR site_id = '' OR site_id = 'default';

INSERT INTO waf_app_pipeline
  (site_id, enabled, semantic_mode, intent_enabled,
   semantic_first, libinjection, sql_intent, fallback_plain_libinjection,
   decode, scope, whitelist, performance, auto_tune,
   created_by_id_name, updated_by_id_name)
SELECT p.site_id, g.enabled, g.semantic_mode, g.intent_enabled,
       g.semantic_first, g.libinjection, g.sql_intent, g.fallback_plain_libinjection,
       g.decode, g.scope, g.whitelist, g.performance, g.auto_tune,
       'backfill', 'backfill'
FROM waf_app_protection p
CROSS JOIN waf_pipeline g
WHERE g.id = 1
  AND NOT EXISTS (SELECT 1 FROM waf_app_pipeline ap WHERE ap.site_id = p.site_id);

INSERT INTO waf_app_pipeline_stages
  (site_id, stage_id, sort_order, enabled, created_by_id_name, updated_by_id_name)
SELECT p.site_id, s.stage_id, s.sort_order, s.enabled, 'backfill', 'backfill'
FROM waf_app_protection p
CROSS JOIN waf_pipeline_stages s
WHERE NOT EXISTS (
  SELECT 1 FROM waf_app_pipeline_stages aps
  WHERE aps.site_id = p.site_id AND aps.stage_id = s.stage_id
);

INSERT INTO waf_app_pipeline_editor
  (site_id, editor_mode, layout, graph, created_by_id_name, updated_by_id_name)
SELECT p.site_id, e.editor_mode, e.layout, e.graph, 'backfill', 'backfill'
FROM waf_app_protection p
CROSS JOIN waf_pipeline_editor e
WHERE e.id = 1
  AND NOT EXISTS (SELECT 1 FROM waf_app_pipeline_editor ape WHERE ape.site_id = p.site_id);

INSERT INTO waf_app_custom_rule_sets
  (site_id, set_id, name, description, enabled, created_by_id_name, updated_by_id_name)
SELECT p.site_id, s.id, s.name, s.description, s.enabled, 'backfill', 'backfill'
FROM waf_app_protection p
CROSS JOIN waf_custom_rule_sets s
WHERE s.site_id = 'global'
  AND NOT EXISTS (SELECT 1 FROM waf_app_custom_rule_sets acs WHERE acs.site_id = p.site_id);

INSERT INTO waf_app_custom_rules
  (site_id, rule_id, rule_set_id, name, phase, priority, enabled, match_field, match_op, match_pattern,
   action, severity, category, rule_type, plugin_name, created_by_id_name, updated_by_id_name)
SELECT p.site_id, r.id, r.rule_set_id, r.name, r.phase, r.priority, r.enabled,
       COALESCE(r.match_field, ''), COALESCE(r.match_op, ''), COALESCE(r.match_pattern, ''),
       r.action, r.severity, r.category, COALESCE(r.rule_type, 'signature'), COALESCE(r.plugin_name, ''),
       'backfill', 'backfill'
FROM waf_app_protection p
CROSS JOIN waf_custom_rules r
INNER JOIN waf_custom_rule_sets s ON s.id = r.rule_set_id AND s.site_id = 'global'
WHERE NOT EXISTS (SELECT 1 FROM waf_app_custom_rules acr WHERE acr.site_id = p.site_id);
