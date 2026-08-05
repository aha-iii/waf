-- Docker 组网库存：每 Agent 节点一份最新快照（JSON）
CREATE TABLE IF NOT EXISTS waf_docker_inventory (
    node_id TEXT PRIMARY KEY,
    hostname TEXT NOT NULL DEFAULT '',
    docker_available BOOLEAN NOT NULL DEFAULT false,
    payload JSONB NOT NULL DEFAULT '{}'::jsonb,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_waf_docker_inventory_updated
    ON waf_docker_inventory (updated_at DESC);
