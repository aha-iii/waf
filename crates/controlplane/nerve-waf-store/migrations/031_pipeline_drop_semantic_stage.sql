-- 语义不再作为流水线检测段；仅保留专家 / 核心 / 自定义
-- 语义增强开关改由 SemanticConfig.enabled（pipeline 表 enabled / JSON）表达

DELETE FROM waf_pipeline_stages WHERE stage_id = 'semantic';
DELETE FROM waf_app_pipeline_stages WHERE stage_id = 'semantic';

UPDATE waf_pipeline_stages SET sort_order = CASE stage_id
    WHEN 'expert' THEN 0
    WHEN 'core' THEN 1
    WHEN 'custom' THEN 2
    WHEN 'crs' THEN 0
    ELSE sort_order
END;

UPDATE waf_app_pipeline_stages SET sort_order = CASE stage_id
    WHEN 'expert' THEN 0
    WHEN 'core' THEN 1
    WHEN 'custom' THEN 2
    WHEN 'crs' THEN 0
    ELSE sort_order
END;
