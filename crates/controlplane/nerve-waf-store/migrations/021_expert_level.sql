-- 专家防护强度（低/中/高），与流水线开关分离
ALTER TABLE waf_pipeline
    ADD COLUMN IF NOT EXISTS expert_level TEXT NOT NULL DEFAULT 'medium';

ALTER TABLE waf_app_pipeline
    ADD COLUMN IF NOT EXISTS expert_level TEXT;

COMMENT ON COLUMN waf_pipeline.expert_level IS '专家防护强度: low | medium | high';
COMMENT ON COLUMN waf_app_pipeline.expert_level IS '应用覆盖专家防护强度；NULL=继承全局';
