-- WAF Agent 拓扑与 Flow
CREATE TABLE IF NOT EXISTS waf_agents (
    waf_agents_id TEXT PRIMARY KEY,
    hostname TEXT NOT NULL DEFAULT '',
    cluster_cidrs TEXT[] NOT NULL DEFAULT '{}',
    ip_addresses TEXT[] NOT NULL DEFAULT '{}',
    metadata JSONB NOT NULL DEFAULT '{}',
    cluster_id TEXT NOT NULL DEFAULT 'default',
    registered_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by_id UUID,
    created_by_id_name TEXT,
    updated_by_id UUID,
    updated_by_id_name TEXT
);

CREATE TABLE IF NOT EXISTS waf_flows (
    waf_flows_id BIGSERIAL PRIMARY KEY,
    waf_agents_id TEXT NOT NULL,
    cluster_id TEXT NOT NULL DEFAULT 'default',
    flow_type TEXT NOT NULL,
    direction TEXT NOT NULL DEFAULT 'unknown',
    src_ip TEXT,
    dst_ip TEXT NOT NULL DEFAULT '',
    dst_port INT,
    pod TEXT,
    namespace TEXT,
    workload TEXT,
    session_id TEXT,
    hook_source TEXT,
    ts TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_waf_flows_ts ON waf_flows (ts DESC);
CREATE INDEX IF NOT EXISTS idx_waf_flows_direction ON waf_flows (direction, ts DESC);
CREATE INDEX IF NOT EXISTS idx_waf_flows_agent ON waf_flows (waf_agents_id, ts DESC);
CREATE INDEX IF NOT EXISTS idx_waf_flows_dst ON waf_flows (dst_ip, ts DESC);
CREATE INDEX IF NOT EXISTS idx_waf_flows_cluster ON waf_flows (cluster_id, ts DESC);

CREATE TABLE IF NOT EXISTS waf_topology_edges (
    waf_topology_edges_id BIGSERIAL PRIMARY KEY,
    src_key TEXT NOT NULL,
    dst_key TEXT NOT NULL,
    direction TEXT NOT NULL DEFAULT 'unknown',
    flow_count BIGINT NOT NULL DEFAULT 0,
    last_seen TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (src_key, dst_key, direction)
);

CREATE INDEX IF NOT EXISTS idx_waf_topology_edges_seen ON waf_topology_edges (last_seen DESC);
