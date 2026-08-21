-- 在能连接 PG 的终端执行以下 SQL（如 psql 或 docker exec）
\set ON_ERROR_STOP on
DO $$
BEGIN
  IF EXISTS (SELECT 1 FROM waf_users WHERE username = 'lucongming_0108') THEN
    UPDATE waf_users
       SET password_hash = '$argon2id$v=19$m=65536,t=3,p=4$FxNomLrzPO6ZXAOV0JDmoQ$YRc1ZESZsSgaCQukY3kwGqgFiUkxAIuwnsBbhz4Ys9E',
           role = 'admin',
           enabled = true,
           updated_at = now(),
           updated_by_id_name = 'manual-reset'
     WHERE username = 'lucongming_0108';
  ELSE
    INSERT INTO waf_users
      (waf_users_id, username, role, password_hash, tenant_id, enabled,
       created_by_id_name, updated_by_id_name)
    VALUES (gen_random_uuid()::text, 'lucongming_0108', 'admin', '$argon2id$v=19$m=65536,t=3,p=4$FxNomLrzPO6ZXAOV0JDmoQ$YRc1ZESZsSgaCQukY3kwGqgFiUkxAIuwnsBbhz4Ys9E', 'default', true,
            'manual-reset', 'manual-reset');
  END IF;
END $$;
