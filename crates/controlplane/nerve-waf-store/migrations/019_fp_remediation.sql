-- 误报治理闭环审计
CREATE TABLE IF NOT EXISTS waf_fp_remediation (
    waf_fp_remediation_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    event_id UUID NOT NULL,
    site_id TEXT NOT NULL,
    src_ip TEXT,
    uri TEXT,
    rule_id TEXT,
    actions JSONB NOT NULL DEFAULT '[]',
    note TEXT,
    operator TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_waf_fp_remediation_site ON waf_fp_remediation (site_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_waf_fp_remediation_event ON waf_fp_remediation (event_id);
