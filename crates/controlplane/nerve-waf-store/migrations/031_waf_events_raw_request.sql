-- 031 增加原始请求头/体字段，用于 L7 攻击事件「原始请求」弹框
ALTER TABLE waf_events ADD COLUMN IF NOT EXISTS headers TEXT;
ALTER TABLE waf_events ADD COLUMN IF NOT EXISTS body TEXT;
