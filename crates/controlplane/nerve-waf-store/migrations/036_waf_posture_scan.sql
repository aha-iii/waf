-- 防护 posture 巡检快照
CREATE TABLE IF NOT EXISTS waf_posture_scan (
    waf_posture_scan_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    site_id TEXT,
    scan_type TEXT NOT NULL DEFAULT 'manual',
    overall_score INT NOT NULL DEFAULT 0,
    findings JSONB NOT NULL DEFAULT '[]',
    summary JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_waf_posture_scan_site ON waf_posture_scan (site_id, created_at DESC);
