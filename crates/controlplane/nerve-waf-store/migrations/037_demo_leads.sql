-- 官网预约演示线索
CREATE TABLE IF NOT EXISTS demo_leads (
    id UUID PRIMARY KEY,
    name TEXT NOT NULL,
    email TEXT NOT NULL,
    company TEXT,
    phone TEXT,
    note TEXT,
    product TEXT NOT NULL DEFAULT 'waf',
    source_path TEXT,
    client_ip TEXT,
    user_agent TEXT,
    wecom_notified BOOLEAN NOT NULL DEFAULT false,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_demo_leads_created_at ON demo_leads (created_at DESC);
CREATE INDEX IF NOT EXISTS idx_demo_leads_email ON demo_leads (email);
