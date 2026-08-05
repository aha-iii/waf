-- 语义运营：按站点持久化统计、样例、误报反馈
CREATE TABLE IF NOT EXISTS waf_semantic_counters (
    site_id     TEXT PRIMARY KEY,
    counters    JSONB NOT NULL DEFAULT '{}',
    updated_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE IF NOT EXISTS waf_semantic_samples (
    id              BIGSERIAL PRIMARY KEY,
    site_id         TEXT NOT NULL,
    ts              TIMESTAMPTZ NOT NULL DEFAULT now(),
    param_path      TEXT NOT NULL DEFAULT '',
    decode_chain    JSONB NOT NULL DEFAULT '[]',
    semantic_score  SMALLINT NOT NULL DEFAULT 0,
    engines         JSONB NOT NULL DEFAULT '[]',
    uri             TEXT NOT NULL DEFAULT '',
    hit             BOOLEAN NOT NULL DEFAULT true
);

CREATE INDEX IF NOT EXISTS idx_waf_semantic_samples_site_ts
    ON waf_semantic_samples (site_id, ts DESC);

CREATE TABLE IF NOT EXISTS waf_semantic_feedback (
    id              BIGSERIAL PRIMARY KEY,
    site_id         TEXT NOT NULL DEFAULT 'default',
    ts              TIMESTAMPTZ NOT NULL DEFAULT now(),
    event_id        TEXT,
    verdict         TEXT NOT NULL,
    note            TEXT,
    param_path      TEXT,
    uri             TEXT,
    applied_action  TEXT
);

CREATE INDEX IF NOT EXISTS idx_waf_semantic_feedback_site_ts
    ON waf_semantic_feedback (site_id, ts DESC);

CREATE INDEX IF NOT EXISTS idx_waf_semantic_feedback_fp
    ON waf_semantic_feedback (site_id, verdict, param_path);
