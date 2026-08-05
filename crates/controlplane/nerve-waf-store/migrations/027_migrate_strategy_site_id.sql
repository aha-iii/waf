-- 027: 旧版全局策略表若含 site_id，迁入应用私有表后删列
-- 由 scripts/apply-migrations.sh / psql 执行；应用进程不跑本文件

DO $$
BEGIN
  IF EXISTS (
    SELECT 1 FROM information_schema.columns
    WHERE table_schema = 'public'
      AND table_name = 'waf_protection_strategies'
      AND column_name = 'site_id'
  ) THEN
    INSERT INTO waf_app_protection_strategies
      (id, site_id, name, description, semantic_mode, expert_level, sort_order,
       expert, semantic, core, custom, created_at, updated_at)
    SELECT id, site_id, name, description, semantic_mode, expert_level, sort_order,
           expert, semantic, core, custom, created_at, updated_at
    FROM waf_protection_strategies
    WHERE site_id IS NOT NULL AND btrim(site_id) <> ''
    ON CONFLICT (id) DO NOTHING;

    DELETE FROM waf_protection_strategies
    WHERE site_id IS NOT NULL AND btrim(site_id) <> '';

    ALTER TABLE waf_protection_strategies DROP COLUMN IF EXISTS site_id;
  END IF;
END $$;
