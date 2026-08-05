-- 026: 应用私有策略可禁用（禁用后不可选绑，绑定回退到标准）
ALTER TABLE waf_app_protection_strategies
    ADD COLUMN IF NOT EXISTS enabled BOOLEAN NOT NULL DEFAULT TRUE;

COMMENT ON COLUMN waf_app_protection_strategies.enabled IS
    'false=禁用：仍可见但不可选/不可绑定；删除才物理移除';
