export type WafEvent = {
  id: string;
  ts: string;
  site_id?: string;
  layer: string;
  action: string;
  category: string;
  rule_id?: string;
  rule_name?: string;
  severity: string;
  src_ip?: string;
  dst_ip?: string;
  uri?: string;
  method?: string;
  host?: string;
  headers?: string;
  body?: string;
  payload_sample?: string;
  message?: string;
  pod?: string;
  inspect_source?: string;
  strategy_id?: string;
  strategy_name?: string;
  protection_stage?: string;
  rule_set?: string;
  plugin_name?: string;
  triage?: TriageSummary;
};

export type TriageSummary = {
  verdict: string;
  confidence: number;
  risk_score: number;
  summary: string;
  engine: string;
  evidence?: { signal: string; weight: number; detail: string }[];
  recommendations?: string[];
  ai_summary?: string;
  ai_engine?: string;
  ai_model?: string;
};

export type ModuleConfig = {
  enabled: boolean;
  action: string;
  sensitivity: string;
  disabled_rules?: string[];
  requests_per_minute?: number;
  burst?: number;
  block_duration_sec?: number;
  score_threshold?: number;
};

export type ProtectionModules = {
  version: number;
  mode: string;
  modules: Record<string, ModuleConfig>;
};

export type StatsOverview = {
  total_events: number;
  blocked: number;
  logged: number;
  by_category: { category: string; count: number }[];
  top_ips: { src_ip: string; count: number }[];
};

export type TrafficSummary = {
  total: number;
  clean: number;
  observed: number;
  blocked: number;
  block_rate: number;
};

export type TrafficSeriesPoint = {
  bucket: string;
  total: number;
  clean: number;
  observed: number;
  blocked: number;
};

export type TrafficAttackCategory = {
  category: string;
  count: number;
};

export type TrafficHostNode = {
  host: string;
  total: number;
  clean: number;
  observed: number;
  blocked: number;
  attack_categories: TrafficAttackCategory[];
};

export type TrafficSiteTreeNode = {
  site_id: string;
  total: number;
  clean: number;
  observed: number;
  blocked: number;
  hosts: TrafficHostNode[];
};

export type TrafficTreeResponse = {
  since: string;
  summary: TrafficSummary;
  sites: TrafficSiteTreeNode[];
};
