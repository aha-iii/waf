CREATE TABLE IF NOT EXISTS waf_events (
    waf_events_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    ts TIMESTAMPTZ NOT NULL,
    site_id TEXT NOT NULL DEFAULT 'default',
    layer TEXT NOT NULL,
    action TEXT NOT NULL,
    category TEXT NOT NULL,
    rule_id TEXT,
    rule_name TEXT,
    severity TEXT NOT NULL DEFAULT 'medium',
    src_ip TEXT,
    dst_ip TEXT,
    dst_port INT,
    method TEXT,
    host TEXT,
    uri TEXT,
    user_agent TEXT,
    payload_sample TEXT,
    matched_field TEXT,
    route_id TEXT,
    pod TEXT,
    message TEXT,
    trace_id TEXT,
    span_id TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by_id UUID,
    created_by_id_name TEXT,
    updated_by_id UUID,
    updated_by_id_name TEXT
);

CREATE INDEX IF NOT EXISTS idx_waf_events_ts ON waf_events (ts DESC);
CREATE INDEX IF NOT EXISTS idx_waf_events_category ON waf_events (category, ts DESC);
CREATE INDEX IF NOT EXISTS idx_waf_events_src_ip ON waf_events (src_ip, ts DESC);
CREATE INDEX IF NOT EXISTS idx_waf_events_action ON waf_events (action, ts DESC);
