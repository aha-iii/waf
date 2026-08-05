-- Console 本地账号：密码哈希、启用状态、显示名
ALTER TABLE waf_users ADD COLUMN IF NOT EXISTS password_hash TEXT;
ALTER TABLE waf_users ADD COLUMN IF NOT EXISTS enabled BOOLEAN NOT NULL DEFAULT true;
ALTER TABLE waf_users ADD COLUMN IF NOT EXISTS display_name TEXT;

CREATE INDEX IF NOT EXISTS idx_waf_users_username ON waf_users (username);
CREATE INDEX IF NOT EXISTS idx_waf_users_role ON waf_users (role);
