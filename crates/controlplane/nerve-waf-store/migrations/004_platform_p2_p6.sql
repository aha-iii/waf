-- K8s 工作负载、NetworkPolicy、RBAC、审计
CREATE TABLE IF NOT EXISTS waf_workloads (
    waf_workloads_id TEXT PRIMARY KEY,
    cluster_id TEXT NOT NULL DEFAULT 'default',
    namespace TEXT NOT NULL,
    name TEXT NOT NULL,
    kind TEXT NOT NULL DEFAULT 'Pod',
    pod_ip TEXT,
    node_name TEXT,
    labels JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by_id UUID,
    created_by_id_name TEXT,
    updated_by_id UUID,
    updated_by_id_name TEXT,
    UNIQUE (cluster_id, namespace, name, kind)
);

CREATE INDEX IF NOT EXISTS idx_waf_workloads_cluster ON waf_workloads (cluster_id, namespace);

CREATE TABLE IF NOT EXISTS waf_network_policies (
    waf_network_policies_id TEXT PRIMARY KEY,
    cluster_id TEXT NOT NULL DEFAULT 'default',
    name TEXT NOT NULL,
    namespace TEXT NOT NULL,
    spec JSONB NOT NULL,
    compiled_at TIMESTAMPTZ,
    enabled BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by_id UUID,
    created_by_id_name TEXT,
    updated_by_id UUID,
    updated_by_id_name TEXT
);

CREATE TABLE IF NOT EXISTS waf_users (
    waf_users_id TEXT PRIMARY KEY,
    username TEXT NOT NULL UNIQUE,
    role TEXT NOT NULL DEFAULT 'viewer',
    api_key_hash TEXT,
    tenant_id TEXT NOT NULL DEFAULT 'default',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by_id UUID,
    created_by_id_name TEXT,
    updated_by_id UUID,
    updated_by_id_name TEXT
);

CREATE TABLE IF NOT EXISTS waf_audit_platform (
    waf_audit_platform_id BIGSERIAL PRIMARY KEY,
    tenant_id TEXT NOT NULL DEFAULT 'default',
    actor TEXT,
    action TEXT NOT NULL,
    resource TEXT,
    detail JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    created_by_id UUID,
    created_by_id_name TEXT,
    updated_by_id UUID,
    updated_by_id_name TEXT
);
