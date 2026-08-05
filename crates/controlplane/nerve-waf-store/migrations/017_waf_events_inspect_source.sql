-- inspect 入口来源：ebpf / http-bypass / agent-local / console 等
ALTER TABLE waf_events ADD COLUMN IF NOT EXISTS inspect_source TEXT;

CREATE INDEX IF NOT EXISTS idx_waf_events_inspect_source ON waf_events (inspect_source, ts DESC);
