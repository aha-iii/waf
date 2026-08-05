-- Edge 流量三档计数（按 site + host + 小时桶 rollup）

CREATE TABLE IF NOT EXISTS waf_traffic_hourly (
    site_id     TEXT NOT NULL,
    host        TEXT NOT NULL DEFAULT '',
    bucket_hour TIMESTAMPTZ NOT NULL,
    total       BIGINT NOT NULL DEFAULT 0,
    clean       BIGINT NOT NULL DEFAULT 0,
    observed    BIGINT NOT NULL DEFAULT 0,
    blocked     BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (site_id, host, bucket_hour)
);

CREATE INDEX IF NOT EXISTS idx_waf_traffic_hourly_bucket
    ON waf_traffic_hourly (bucket_hour DESC);

CREATE INDEX IF NOT EXISTS idx_waf_traffic_hourly_site_bucket
    ON waf_traffic_hourly (site_id, bucket_hour DESC);
