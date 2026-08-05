-- ML 训练标注库：FP/FN/Shadow 样本统一入库，供 Engram 导出与持续学习
CREATE TABLE IF NOT EXISTS waf_ml_labels (
    waf_ml_labels_id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    label_type TEXT NOT NULL,
    source TEXT NOT NULL,
    event_id UUID,
    site_id TEXT,
    src_ip TEXT,
    uri TEXT,
    rule_id TEXT,
    category TEXT,
    payload_sample TEXT,
    metadata JSONB NOT NULL DEFAULT '{}',
    operator TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_waf_ml_labels_type ON waf_ml_labels (label_type, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_waf_ml_labels_site ON waf_ml_labels (site_id, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_waf_ml_labels_event ON waf_ml_labels (event_id);

COMMENT ON TABLE waf_ml_labels IS 'AI 训练标注：false_positive | false_negative | shadow_hit | confirmed_attack';
