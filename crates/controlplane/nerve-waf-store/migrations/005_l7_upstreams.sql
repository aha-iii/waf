-- L7 Upstreams（后端池 / upstream）
CREATE TABLE IF NOT EXISTS waf_l7_upstreams (
    id              TEXT PRIMARY KEY,
    site_id         TEXT NOT NULL DEFAULT 'default',
    name            TEXT NOT NULL,
    enabled         BOOLEAN NOT NULL DEFAULT true,
    scheme          TEXT NOT NULL DEFAULT 'http',
    upstream_type   TEXT NOT NULL DEFAULT 'roundrobin',
    nodes           JSONB NOT NULL DEFAULT '[]',
    description     TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_by      TEXT
);

CREATE INDEX IF NOT EXISTS idx_l7_upstreams_site ON waf_l7_upstreams (site_id, enabled);

ALTER TABLE waf_l7_routes ADD COLUMN IF NOT EXISTS upstream_id TEXT;
