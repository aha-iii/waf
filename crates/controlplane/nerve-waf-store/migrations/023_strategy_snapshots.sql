-- 策略四段快照：expert / semantic / core / custom
ALTER TABLE waf_protection_strategies
    ADD COLUMN IF NOT EXISTS expert JSONB NOT NULL DEFAULT '{"level":"medium"}'::jsonb;

ALTER TABLE waf_protection_strategies
    ADD COLUMN IF NOT EXISTS semantic JSONB NOT NULL DEFAULT '{"mode":"standard"}'::jsonb;

ALTER TABLE waf_protection_strategies
    ADD COLUMN IF NOT EXISTS core JSONB NOT NULL DEFAULT '{}'::jsonb;

ALTER TABLE waf_protection_strategies
    ADD COLUMN IF NOT EXISTS custom JSONB NOT NULL DEFAULT '{}'::jsonb;

-- 从旧列回填 expert / semantic.mode
UPDATE waf_protection_strategies
SET expert = jsonb_build_object('level', COALESCE(expert_level, 'medium')),
    semantic = jsonb_build_object('mode', COALESCE(semantic_mode, 'standard'))
WHERE expert = '{"level":"medium"}'::jsonb
   OR semantic = '{"mode":"standard"}'::jsonb;

COMMENT ON COLUMN waf_protection_strategies.expert IS '专家防护快照 JSON，如 {level}';
COMMENT ON COLUMN waf_protection_strategies.semantic IS '语义检测快照 JSON（可为完整 SemanticConfig）';
COMMENT ON COLUMN waf_protection_strategies.core IS '核心防护快照 ProtectionModules JSON';
COMMENT ON COLUMN waf_protection_strategies.custom IS '自定义检测快照 JSON';
