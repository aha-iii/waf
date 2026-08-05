-- 检测流水线默认顺序：语义 → 核心(CRS) → 自定义规则 → 自定义插件
UPDATE waf_pipeline_stages SET sort_order = CASE stage_id
    WHEN 'semantic' THEN 0
    WHEN 'crs' THEN 1
    WHEN 'custom' THEN 2
    WHEN 'plugin' THEN 3
    ELSE sort_order
END;

UPDATE waf_app_pipeline_stages SET sort_order = CASE stage_id
    WHEN 'semantic' THEN 0
    WHEN 'crs' THEN 1
    WHEN 'custom' THEN 2
    WHEN 'plugin' THEN 3
    ELSE sort_order
END;
