use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use std::collections::HashMap;

use crate::StoreError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowIngestRecord {
    #[serde(rename = "type")]
    pub flow_type: String,
    pub src_ip: Option<String>,
    pub dst_ip: String,
    pub dport: Option<u16>,
    pub pod: Option<String>,
    pub namespace: Option<String>,
    pub workload: Option<String>,
    pub node_id: Option<String>,
    pub direction: Option<String>,
    pub session_id: Option<String>,
    pub hook_source: Option<String>,
    pub time: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowIngestBatch {
    pub node_id: String,
    #[serde(default = "default_cluster")]
    pub cluster_id: String,
    pub flows: Vec<FlowIngestRecord>,
}

fn default_cluster() -> String {
    "default".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkloadRecord {
    /// Optional: overwritten by batch.cluster_id at sync time.
    #[serde(default)]
    pub cluster_id: String,
    pub namespace: String,
    pub name: String,
    pub kind: String,
    pub pod_ip: Option<String>,
    pub node_name: Option<String>,
    #[serde(default)]
    pub labels: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkloadSyncBatch {
    #[serde(default = "default_cluster")]
    pub cluster_id: String,
    pub workloads: Vec<WorkloadRecord>,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentRegisterRequest {
    pub node_id: String,
    pub hostname: Option<String>,
    pub ip_addresses: Option<Vec<String>>,
    pub cluster_cidrs: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopologyNode {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub namespace: Option<String>,
    pub node_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopologyEdge {
    pub source: String,
    pub target: String,
    pub direction: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopologyGraph {
    pub nodes: Vec<TopologyNode>,
    pub edges: Vec<TopologyEdge>,
    pub stats: TopologyStats,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TopologyStats {
    pub total_flows: i64,
    pub north_south: i64,
    pub east_west: i64,
    pub agents: i64,
}

pub struct TopologyStore {
    pool: PgPool,
}

impl TopologyStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn register_agent(&self, req: AgentRegisterRequest) -> Result<(), StoreError> {
        sqlx::query(
            r#"INSERT INTO waf_agents (waf_agents_id, hostname, cluster_cidrs, ip_addresses, last_seen)
               VALUES ($1, $2, $3, $4, now())
               ON CONFLICT (waf_agents_id) DO UPDATE SET
                 hostname = EXCLUDED.hostname,
                 cluster_cidrs = COALESCE(EXCLUDED.cluster_cidrs, waf_agents.cluster_cidrs),
                 ip_addresses = COALESCE(EXCLUDED.ip_addresses, waf_agents.ip_addresses),
                 last_seen = now(),
                 updated_at = now()"#,
        )
        .bind(&req.node_id)
        .bind(req.hostname.unwrap_or_else(|| req.node_id.clone()))
        .bind(req.cluster_cidrs.unwrap_or_default())
        .bind(req.ip_addresses.unwrap_or_default())
        .execute(&self.pool)
        .await
        .map_err(|e| StoreError::Db(e.to_string()))?;
        Ok(())
    }

    pub async fn ingest_flows(&self, batch: FlowIngestBatch) -> Result<usize, StoreError> {
        if batch.flows.is_empty() {
            return Ok(0);
        }
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| StoreError::Db(e.to_string()))?;
        let mut n = 0usize;
        for f in &batch.flows {
            let ts = f.time.unwrap_or_else(Utc::now);
            let direction = f.direction.clone().unwrap_or_else(|| "unknown".into());
            sqlx::query(
                r#"INSERT INTO waf_flows (
                    waf_agents_id, cluster_id, flow_type, direction, src_ip, dst_ip, dst_port,
                    pod, namespace, workload, session_id, hook_source, ts
                ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)"#,
            )
            .bind(&batch.node_id)
            .bind(&batch.cluster_id)
            .bind(&f.flow_type)
            .bind(&direction)
            .bind(&f.src_ip)
            .bind(&f.dst_ip)
            .bind(f.dport.map(|p| p as i32))
            .bind(&f.pod)
            .bind(&f.namespace)
            .bind(&f.workload)
            .bind(&f.session_id)
            .bind(&f.hook_source)
            .bind(ts)
            .execute(&mut *tx)
            .await
            .map_err(|e| StoreError::Db(e.to_string()))?;

            let src_key = flow_endpoint_key(f, true, &batch.node_id);
            let dst_key = flow_endpoint_key(f, false, &batch.node_id);
            sqlx::query(
                r#"INSERT INTO waf_topology_edges (src_key, dst_key, direction, flow_count, last_seen)
                   VALUES ($1, $2, $3, 1, $4)
                   ON CONFLICT (src_key, dst_key, direction) DO UPDATE SET
                     flow_count = waf_topology_edges.flow_count + 1,
                     last_seen = EXCLUDED.last_seen"#,
            )
            .bind(&src_key)
            .bind(&dst_key)
            .bind(&direction)
            .bind(ts)
            .execute(&mut *tx)
            .await
            .map_err(|e| StoreError::Db(e.to_string()))?;
            n += 1;
        }
        sqlx::query("UPDATE waf_agents SET last_seen = now(), updated_at = now() WHERE waf_agents_id = $1")
            .bind(&batch.node_id)
            .execute(&mut *tx)
            .await
            .map_err(|e| StoreError::Db(e.to_string()))?;
        tx.commit()
            .await
            .map_err(|e| StoreError::Db(e.to_string()))?;
        Ok(n)
    }

    pub async fn agent_count(&self) -> Result<i64, StoreError> {
        let row: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM waf_agents")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| StoreError::Db(e.to_string()))?;
        Ok(row.0)
    }

    pub async fn list_agents(&self) -> Result<Vec<serde_json::Value>, StoreError> {
        let rows = sqlx::query(
            r#"SELECT waf_agents_id AS node_id, hostname, cluster_cidrs, last_seen,
                      EXTRACT(EPOCH FROM (now() - last_seen)) AS seconds_ago
               FROM waf_agents ORDER BY last_seen DESC"#,
        )
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StoreError::Db(e.to_string()))?;
        Ok(rows
            .iter()
            .map(|r| {
                serde_json::json!({
                    "node_id": r.get::<String, _>("node_id"),
                    "hostname": r.get::<String, _>("hostname"),
                    "cluster_cidrs": r.get::<Vec<String>, _>("cluster_cidrs"),
                    "last_seen": r.get::<DateTime<Utc>, _>("last_seen"),
                    "seconds_ago": r.get::<f64, _>("seconds_ago"),
                })
            })
            .collect())
    }

    pub async fn graph(&self, minutes: i64, direction: Option<&str>) -> Result<TopologyGraph, StoreError> {
        let since = Utc::now() - chrono::Duration::minutes(minutes.max(1));
        let agents: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM waf_agents")
            .fetch_one(&self.pool)
            .await
            .map_err(|e| StoreError::Db(e.to_string()))?;

        let mut stats = TopologyStats { agents, ..Default::default() };

        let count_rows = sqlx::query(
            r#"SELECT direction, COUNT(*) AS c FROM waf_flows
               WHERE ts >= $1 GROUP BY direction"#,
        )
        .bind(since)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| StoreError::Db(e.to_string()))?;
        for r in &count_rows {
            let dir: String = r.get("direction");
            let c: i64 = r.get("c");
            stats.total_flows += c;
            match dir.as_str() {
                "north_south" => stats.north_south = c,
                "east_west" => stats.east_west = c,
                _ => {}
            }
        }

        let edge_query = if let Some(d) = direction.filter(|x| !x.is_empty() && *x != "all") {
            sqlx::query(
                r#"SELECT src_key, dst_key, direction, SUM(flow_count)::bigint AS cnt
                   FROM waf_topology_edges
                   WHERE last_seen >= $1 AND direction = $2
                   GROUP BY src_key, dst_key, direction
                   ORDER BY cnt DESC LIMIT 200"#,
            )
            .bind(since)
            .bind(d)
            .fetch_all(&self.pool)
            .await
        } else {
            sqlx::query(
                r#"SELECT src_key, dst_key, direction, SUM(flow_count)::bigint AS cnt
                   FROM waf_topology_edges
                   WHERE last_seen >= $1
                   GROUP BY src_key, dst_key, direction
                   ORDER BY cnt DESC LIMIT 200"#,
            )
            .bind(since)
            .fetch_all(&self.pool)
            .await
        }
        .map_err(|e| StoreError::Db(e.to_string()))?;

        let mut nodes_map: HashMap<String, TopologyNode> = HashMap::new();
        let mut edges = Vec::new();

        for r in edge_query {
            let src: String = r.get("src_key");
            let dst: String = r.get("dst_key");
            let dir: String = r.get("direction");
            let cnt: i64 = r.get("cnt");
            ensure_node(&mut nodes_map, &src);
            ensure_node(&mut nodes_map, &dst);
            edges.push(TopologyEdge {
                source: src,
                target: dst,
                direction: dir,
                count: cnt,
            });
        }

        Ok(TopologyGraph {
            nodes: nodes_map.into_values().collect(),
            edges,
            stats,
        })
    }

    pub async fn recent_flows(
        &self,
        limit: i64,
        direction: Option<&str>,
    ) -> Result<Vec<serde_json::Value>, StoreError> {
        let limit = limit.clamp(1, 500);
        let rows = if let Some(d) = direction.filter(|x| !x.is_empty() && *x != "all") {
            sqlx::query(
                r#"SELECT waf_agents_id AS node_id, flow_type, direction, src_ip, dst_ip, dst_port,
                          pod, namespace, workload, session_id, ts
                   FROM waf_flows WHERE direction = $1
                   ORDER BY ts DESC LIMIT $2"#,
            )
            .bind(d)
            .bind(limit)
            .fetch_all(&self.pool)
            .await
        } else {
            sqlx::query(
                r#"SELECT waf_agents_id AS node_id, flow_type, direction, src_ip, dst_ip, dst_port,
                          pod, namespace, workload, session_id, ts
                   FROM waf_flows ORDER BY ts DESC LIMIT $1"#,
            )
            .bind(limit)
            .fetch_all(&self.pool)
            .await
        }
        .map_err(|e| StoreError::Db(e.to_string()))?;

        Ok(rows
            .iter()
            .map(|r| {
                serde_json::json!({
                    "node_id": r.get::<String, _>("node_id"),
                    "type": r.get::<String, _>("flow_type"),
                    "direction": r.get::<String, _>("direction"),
                    "src_ip": r.get::<Option<String>, _>("src_ip"),
                    "dst_ip": r.get::<String, _>("dst_ip"),
                    "dport": r.get::<Option<i32>, _>("dst_port"),
                    "pod": r.get::<Option<String>, _>("pod"),
                    "namespace": r.get::<Option<String>, _>("namespace"),
                    "workload": r.get::<Option<String>, _>("workload"),
                    "session_id": r.get::<Option<String>, _>("session_id"),
                    "ts": r.get::<DateTime<Utc>, _>("ts"),
                })
            })
            .collect())
    }

    pub async fn sync_workloads(&self, batch: WorkloadSyncBatch) -> Result<usize, StoreError> {
        let mut n = 0usize;
        for w in &batch.workloads {
            let cluster = if w.cluster_id.is_empty() {
                batch.cluster_id.as_str()
            } else {
                w.cluster_id.as_str()
            };
            let id = format!("{}/{}/{}", cluster, w.namespace, w.name);
            sqlx::query(
                r#"INSERT INTO waf_workloads (waf_workloads_id, cluster_id, namespace, name, kind, pod_ip, node_name, labels, updated_at)
                   VALUES ($1,$2,$3,$4,$5,$6,$7,$8,now())
                   ON CONFLICT (cluster_id, namespace, name, kind) DO UPDATE SET
                     pod_ip = EXCLUDED.pod_ip,
                     node_name = EXCLUDED.node_name,
                     labels = EXCLUDED.labels,
                     updated_at = now()"#,
            )
            .bind(&id)
            .bind(cluster)
            .bind(&w.namespace)
            .bind(&w.name)
            .bind(&w.kind)
            .bind(&w.pod_ip)
            .bind(&w.node_name)
            .bind(&w.labels)
            .execute(&self.pool)
            .await
            .map_err(|e| StoreError::Db(e.to_string()))?;
            n += 1;
        }
        Ok(n)
    }

    pub async fn sync_docker_inventory(
        &self,
        payload: serde_json::Value,
    ) -> Result<(), StoreError> {
        let node_id = payload
            .get("node_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        if node_id.is_empty() {
            return Err(StoreError::Db("docker inventory missing node_id".into()));
        }
        let hostname = payload
            .get("hostname")
            .and_then(|v| v.as_str())
            .unwrap_or(&node_id)
            .to_string();
        let docker_available = payload
            .get("docker_available")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        sqlx::query(
            r#"INSERT INTO waf_docker_inventory (node_id, hostname, docker_available, payload, updated_at)
               VALUES ($1, $2, $3, $4, now())
               ON CONFLICT (node_id) DO UPDATE SET
                 hostname = EXCLUDED.hostname,
                 docker_available = EXCLUDED.docker_available,
                 payload = EXCLUDED.payload,
                 updated_at = now()"#,
        )
        .bind(&node_id)
        .bind(&hostname)
        .bind(docker_available)
        .bind(&payload)
        .execute(&self.pool)
        .await
        .map_err(|e| StoreError::Db(e.to_string()))?;
        Ok(())
    }

    /// Aggregate Docker inventory across nodes; optional node_id filter.
    pub async fn docker_inventory_aggregate(
        &self,
        node_id: Option<&str>,
        stale_secs: i64,
    ) -> Result<serde_json::Value, StoreError> {
        let stale_secs = stale_secs.clamp(30, 86400);
        let rows = if let Some(nid) = node_id.filter(|x| !x.is_empty()) {
            sqlx::query(
                r#"SELECT d.node_id, d.hostname, d.docker_available, d.payload, d.updated_at,
                          a.last_seen
                   FROM waf_docker_inventory d
                   LEFT JOIN waf_agents a ON a.waf_agents_id = d.node_id
                   WHERE d.node_id = $1"#,
            )
            .bind(nid)
            .fetch_all(&self.pool)
            .await
        } else {
            sqlx::query(
                r#"SELECT d.node_id, d.hostname, d.docker_available, d.payload, d.updated_at,
                          a.last_seen
                   FROM waf_docker_inventory d
                   LEFT JOIN waf_agents a ON a.waf_agents_id = d.node_id
                   ORDER BY d.hostname"#,
            )
            .fetch_all(&self.pool)
            .await
        }
        .map_err(|e| StoreError::Db(e.to_string()))?;

        let now = Utc::now();
        let mut hosts = Vec::new();
        let mut networks: HashMap<String, serde_json::Value> = HashMap::new();
        let mut containers = Vec::new();
        let mut swarm_services: HashMap<String, serde_json::Value> = HashMap::new();
        let mut running = 0i64;

        for r in &rows {
            let nid = r.get::<String, _>("node_id");
            let hostname = r.get::<String, _>("hostname");
            let docker_available = r.get::<bool, _>("docker_available");
            let updated_at = r.get::<DateTime<Utc>, _>("updated_at");
            let last_seen = r.get::<Option<DateTime<Utc>>, _>("last_seen");
            let payload = r.get::<serde_json::Value, _>("payload");
            let online = last_seen
                .map(|t| (now - t).num_seconds() < stale_secs)
                .unwrap_or(false);
            let stale = (now - updated_at).num_seconds() > stale_secs;
            hosts.push(serde_json::json!({
                "node_id": nid,
                "hostname": hostname,
                "online": online,
                "docker_available": docker_available,
                "stale": stale,
                "updated_at": updated_at,
            }));

            if let Some(nets) = payload.get("networks").and_then(|v| v.as_array()) {
                for n in nets {
                    let id = n.get("id").and_then(|v| v.as_str()).unwrap_or("");
                    if id.is_empty() {
                        continue;
                    }
                    networks
                        .entry(id.to_string())
                        .and_modify(|existing| merge_network(existing, n))
                        .or_insert_with(|| n.clone());
                }
            }
            if let Some(ctrs) = payload.get("containers").and_then(|v| v.as_array()) {
                for c in ctrs {
                    let mut cc = c.clone();
                    if let Some(obj) = cc.as_object_mut() {
                        obj.insert("node_id".into(), json_str(&nid));
                        obj.insert("hostname".into(), json_str(&hostname));
                    }
                    if c.get("state").and_then(|v| v.as_str()) == Some("running") {
                        running += 1;
                    }
                    containers.push(cc);
                }
            }
            if let Some(svcs) = payload.get("swarm_services").and_then(|v| v.as_array()) {
                for s in svcs {
                    let id = s.get("id").and_then(|v| v.as_str()).unwrap_or("");
                    if id.is_empty() {
                        continue;
                    }
                    swarm_services.entry(id.to_string()).or_insert_with(|| s.clone());
                }
            }
        }

        let mut networks_out: Vec<_> = networks.into_values().collect();
        networks_out.sort_by(|a, b| {
            let an = a.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let bn = b.get("name").and_then(|v| v.as_str()).unwrap_or("");
            an.cmp(bn)
        });
        let mut services_out: Vec<_> = swarm_services.into_values().collect();
        services_out.sort_by(|a, b| {
            let an = a.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let bn = b.get("name").and_then(|v| v.as_str()).unwrap_or("");
            an.cmp(bn)
        });

        Ok(serde_json::json!({
            "fabric_id": "default",
            "generated_at": now,
            "hosts": hosts,
            "networks": networks_out,
            "containers": containers,
            "swarm_services": services_out,
            "stats": {
                "hosts": hosts.len(),
                "networks": networks_out.len(),
                "containers": containers.len(),
                "running": running,
                "swarm_services": services_out.len(),
            }
        }))
    }

    pub async fn list_workloads(
        &self,
        cluster_id: Option<&str>,
        limit: i64,
    ) -> Result<Vec<serde_json::Value>, StoreError> {
        let limit = limit.clamp(1, 500);
        let rows = if let Some(c) = cluster_id.filter(|x| !x.is_empty()) {
            sqlx::query(
                r#"SELECT cluster_id, namespace, name, kind, pod_ip, node_name, labels, updated_at
                   FROM waf_workloads WHERE cluster_id = $1 ORDER BY updated_at DESC LIMIT $2"#,
            )
            .bind(c)
            .bind(limit)
            .fetch_all(&self.pool)
            .await
        } else {
            sqlx::query(
                r#"SELECT cluster_id, namespace, name, kind, pod_ip, node_name, labels, updated_at
                   FROM waf_workloads ORDER BY updated_at DESC LIMIT $1"#,
            )
            .bind(limit)
            .fetch_all(&self.pool)
            .await
        }
        .map_err(|e| StoreError::Db(e.to_string()))?;
        Ok(rows
            .iter()
            .map(|r| {
                serde_json::json!({
                    "cluster_id": r.get::<String, _>("cluster_id"),
                    "namespace": r.get::<String, _>("namespace"),
                    "name": r.get::<String, _>("name"),
                    "kind": r.get::<String, _>("kind"),
                    "pod_ip": r.get::<Option<String>, _>("pod_ip"),
                    "node_name": r.get::<Option<String>, _>("node_name"),
                    "labels": r.get::<serde_json::Value, _>("labels"),
                    "updated_at": r.get::<DateTime<Utc>, _>("updated_at"),
                })
            })
            .collect())
    }

    pub async fn graph_with_cluster(
        &self,
        minutes: i64,
        direction: Option<&str>,
        cluster_id: Option<&str>,
    ) -> Result<TopologyGraph, StoreError> {
        let mut g = self.graph(minutes, direction).await?;
        let workloads = if let Some(c) = cluster_id.filter(|x| !x.is_empty() && *x != "all") {
            self.list_workloads(Some(c), 200).await?
        } else {
            self.list_workloads(None, 200).await.unwrap_or_default()
        };

        // IP → namespace/workload: lift 'IP · x.x.x.x' to a human-readable name.
        let mut ip_to_wl: HashMap<String, (String, String, Option<String>)> = HashMap::new();
        for w in &workloads {
            let ns = w.get("namespace").and_then(|v| v.as_str()).unwrap_or("");
            let name = w.get("name").and_then(|v| v.as_str()).unwrap_or("");
            if ns.is_empty() || name.is_empty() {
                continue;
            }
            let id = format!("{ns}/{name}");
            let node_name = w
                .get("node_name")
                .and_then(|v| v.as_str())
                .map(str::to_string);
            if let Some(ip) = w.get("pod_ip").and_then(|v| v.as_str()) {
                if !ip.is_empty() {
                    ip_to_wl.insert(ip.to_string(), (id.clone(), name.to_string(), node_name.clone()));
                }
            }
            if !g.nodes.iter().any(|n| n.id == id) {
                g.nodes.push(TopologyNode {
                    id: id.clone(),
                    label: name.to_string(),
                    kind: "workload".into(),
                    namespace: Some(ns.to_string()),
                    node_id: node_name,
                });
            }
        }

        for n in &mut g.nodes {
            if let Some(ip) = n.id.strip_prefix("ip:") {
                if let Some((wl_id, label, node_name)) = ip_to_wl.get(ip) {
                    n.id = wl_id.clone();
                    n.label = label.clone();
                    n.kind = "workload".into();
                    n.namespace = wl_id.split('/').next().map(str::to_string);
                    n.node_id = node_name.clone();
                }
            }
        }
        for e in &mut g.edges {
            for end in [&mut e.source, &mut e.target] {
                if let Some(ip) = end.strip_prefix("ip:") {
                    if let Some((wl_id, _, _)) = ip_to_wl.get(ip) {
                        *end = wl_id.clone();
                    }
                }
            }
        }
        let mut seen = std::collections::HashSet::new();
        g.nodes.retain(|n| seen.insert(n.id.clone()));

        Ok(g)
    }
}

fn is_synthetic_pod(pod: &str) -> bool {
    pod.is_empty()
        || pod.starts_with("cgroup:")
        || pod.starts_with("container:")
        || pod.starts_with("docker-")
}

fn flow_endpoint_key(f: &FlowIngestRecord, is_src: bool, node_id: &str) -> String {
    if is_src {
        if let Some(ns) = &f.namespace {
            if !ns.is_empty() {
                if let Some(wl) = &f.workload {
                    if !is_synthetic_pod(wl) {
                        return format!("{ns}/{wl}");
                    }
                }
                if let Some(pod) = &f.pod {
                    if !is_synthetic_pod(pod) {
                        return format!("{ns}/{pod}");
                    }
                }
            }
        }
        if let Some(pod) = &f.pod {
            if !is_synthetic_pod(pod) {
                // pod:uid short codes are still more readable than raw cgroup.
                return pod.clone();
            }
        }
        if let Some(ip) = &f.src_ip {
            if !ip.is_empty() && ip != "0.0.0.0" && ip != "::" {
                return format!("ip:{ip}");
            }
        }
        return format!("node:{node_id}");
    }
    if !f.dst_ip.is_empty() {
        // Prefer existing business Pod/workload name on destination (avoid forever showing only IP).
        if let Some(ns) = &f.namespace {
            if !ns.is_empty() {
                if let Some(wl) = &f.workload {
                    if !is_synthetic_pod(wl) {
                        return format!("{ns}/{wl}");
                    }
                }
                if let Some(pod) = &f.pod {
                    if !is_synthetic_pod(pod) {
                        return format!("{ns}/{pod}");
                    }
                }
            }
        }
        if f.dst_ip == "0.0.0.0" || f.dst_ip == "::" || f.dst_ip.is_empty() {
            return "ext:unresolved".into();
        }
        if f.dst_ip.starts_with("10.")
            || f.dst_ip.starts_with("172.")
            || f.dst_ip.starts_with("192.168.")
            || f.dst_ip.starts_with("127.")
        {
            return format!("ip:{}", f.dst_ip);
        }
        return format!("ext:{}", f.dst_ip);
    }
    "unknown".into()
}

fn ensure_node(map: &mut HashMap<String, TopologyNode>, id: &str) {
    if map.contains_key(id) {
        return;
    }
    let (kind, label, namespace) = classify_topology_id(id);
    map.insert(
        id.to_string(),
        TopologyNode {
            id: id.to_string(),
            label,
            kind: kind.into(),
            namespace,
            node_id: None,
        },
    );
}

fn json_str(s: &str) -> serde_json::Value {
    serde_json::Value::String(s.to_string())
}

fn merge_network(dst: &mut serde_json::Value, src: &serde_json::Value) {
    let Some(dobj) = dst.as_object_mut() else {
        return;
    };
    for key in ["subnets", "gateways"] {
        let mut set: Vec<String> = dobj
            .get(key)
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|x| x.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        if let Some(arr) = src.get(key).and_then(|v| v.as_array()) {
            for x in arr {
                if let Some(s) = x.as_str() {
                    if !set.iter().any(|e| e == s) {
                        set.push(s.to_string());
                    }
                }
            }
        }
        dobj.insert(
            key.into(),
            serde_json::Value::Array(set.into_iter().map(serde_json::Value::String).collect()),
        );
    }
}

/// Topology node display name: never present raw cgroup hex as a 'service name' to users.
fn classify_topology_id(id: &str) -> (&'static str, String, Option<String>) {
    if let Some(rest) = id.strip_prefix("ext:") {
        let label = match rest {
            "" | "::" | "0.0.0.0" | "unresolved" => "外部 · 未解析".into(),
            "127.0.0.1" | "::1" => "本机回环".into(),
            other => format!("外部 · {other}"),
        };
        return ("external", label, None);
    }
    if let Some(ip) = id.strip_prefix("ip:") {
        return ("ip", format!("IP · {ip}"), None);
    }
    if let Some(node) = id.strip_prefix("node:") {
        return ("node", format!("节点 · {node}"), None);
    }
    if let Some(uid) = id.strip_prefix("pod:") {
        return ("pod", format!("Pod · {uid}"), None);
    }
    if let Some(hex) = id.strip_prefix("cgroup:") {
        let short = if hex.len() > 8 { &hex[..8] } else { hex };
        return (
            "unknown",
            format!("未识别工作负载 · {short}"),
            None,
        );
    }
    if id.contains('/') {
        let ns = id.split('/').next().map(str::to_string);
        // namespace/name → shorten display to name (title still uses id).
        let name = id.rsplit('/').next().unwrap_or(id);
        return ("workload", name.to_string(), ns);
    }
    ("unknown", id.to_string(), None)
}
