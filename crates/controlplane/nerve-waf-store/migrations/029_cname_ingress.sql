-- CNAME / SaaS 边缘接入：证书与边缘节点元数据（应用 ingress 字段仍在 platform sites JSON）
CREATE TABLE IF NOT EXISTS waf_ingress_certificates (
    id              TEXT PRIMARY KEY,
    site_id         TEXT NOT NULL,
    source          TEXT NOT NULL DEFAULT 'upload',
    hosts           JSONB NOT NULL DEFAULT '[]'::jsonb,
    not_before      TIMESTAMPTZ,
    not_after       TIMESTAMPTZ,
    status          TEXT NOT NULL DEFAULT 'active',
    -- 生产应改密钥服务；此处仅存元数据引用，私钥密文可选
    cert_pem_enc    TEXT,
    key_pem_enc     TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_waf_ingress_certs_site
    ON waf_ingress_certificates (site_id);

CREATE TABLE IF NOT EXISTS waf_edge_nodes (
    id              TEXT PRIMARY KEY,
    name            TEXT NOT NULL,
    pop             TEXT NOT NULL DEFAULT 'default',
    admin_url       TEXT,
    last_seen       TIMESTAMPTZ,
    status          TEXT NOT NULL DEFAULT 'unknown',
    meta            JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);

COMMENT ON TABLE waf_ingress_certificates IS 'CNAME 边缘 TLS 证书元数据（ACME/上传）';
COMMENT ON TABLE waf_edge_nodes IS 'SaaS 边缘 Gateway 节点注册表';
