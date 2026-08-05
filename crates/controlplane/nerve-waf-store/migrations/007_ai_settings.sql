-- AI 研判：语义管线 (NSP) + LLM 解释配置（Console「AI 研判」菜单持久化）
CREATE TABLE IF NOT EXISTS waf_ai_settings (
    site_id       TEXT PRIMARY KEY DEFAULT 'default',
    semantic      JSONB NOT NULL DEFAULT '{}',
    triage_llm    JSONB NOT NULL DEFAULT '{}',
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_by    TEXT
);
