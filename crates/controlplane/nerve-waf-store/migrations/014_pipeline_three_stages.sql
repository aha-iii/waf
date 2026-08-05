-- 流水线三段：语义检测 / 核心检测 / 自定义检测（plugin 段并入 custom）
DELETE FROM waf_pipeline_stages WHERE stage_id = 'plugin';
DELETE FROM waf_app_pipeline_stages WHERE stage_id = 'plugin';

UPDATE waf_pipeline_stages SET sort_order = CASE stage_id
    WHEN 'semantic' THEN 0
    WHEN 'crs' THEN 1
    WHEN 'custom' THEN 2
    ELSE sort_order
END;

UPDATE waf_app_pipeline_stages SET sort_order = CASE stage_id
    WHEN 'semantic' THEN 0
    WHEN 'crs' THEN 1
    WHEN 'custom' THEN 2
    ELSE sort_order
END;
