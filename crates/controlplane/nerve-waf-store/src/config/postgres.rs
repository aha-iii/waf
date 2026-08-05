use super::{ConfigError, RuntimeDenyEntry};
use nerve_waf_sdk::{
    L7Match, L7Upstream, ProtectionModules, RouteMatch, RoutePluginRef, WafConfig, WafRoute,
};
use serde_json::{json, Value};
use sqlx::{PgPool, Row};
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct PostgresConfigStore {
    pool: PgPool,
    site_id: String,
    data_dir: PathBuf,
}

impl PostgresConfigStore {
    pub async fn connect(database_url: &str, data_dir: &Path) -> Result<Self, ConfigError> {
        let pool = crate::migrations::connect_pool(database_url)
            .await
            .map_err(|e| ConfigError::Db(e.to_string()))?;
        let site_id = std::env::var("SITE_ID").unwrap_or_else(|_| "default".into());
        Ok(Self {
            pool,
            site_id,
            data_dir: data_dir.to_path_buf(),
        })
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn import_from_files_if_empty(&self) -> Result<bool, ConfigError> {
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM waf_l3_policies WHERE site_id = $1",
        )
        .bind(&self.site_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| ConfigError::Db(e.to_string()))?;

        let l7_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM waf_l7_routes WHERE site_id = $1",
        )
        .bind(&self.site_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| ConfigError::Db(e.to_string()))?;

        if count > 0 || l7_count > 0 {
            return Ok(false);
        }

        let file = super::file::FileConfigStore::new(&self.data_dir);
        if file.config_path().exists() || file.modules_path().exists() {
            let config = file.load_config()?;
            if !config.routes.is_empty() {
                self.save_config(&config, "import").await?;
            }
            if file.modules_path().exists() {
                let modules = file.load_modules()?;
                self.save_modules(&modules, "import").await?;
            }
            let upstreams = file.load_upstreams()?;
            if !upstreams.is_empty() {
                self.save_upstreams(&upstreams, "import").await?;
            }
            return Ok(true);
        }
        Ok(false)
    }

    pub async fn load_config(&self) -> Result<WafConfig, ConfigError> {
        let mut routes = self.load_l3_routes().await?;
        routes.extend(self.load_l7_routes().await?);
        routes.sort_by_key(|r| r.priority);
        Ok(WafConfig {
            version: 1,
            routes,
        })
    }

    async fn load_l3_routes(&self) -> Result<Vec<WafRoute>, ConfigError> {
        let rows = sqlx::query(
            r#"SELECT id, name, priority, enabled, src_ip, dst_ip, dst_port, pod
               FROM waf_l3_policies WHERE site_id = $1 ORDER BY priority"#,
        )
        .bind(&self.site_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| ConfigError::Db(e.to_string()))?;

        let mut routes = Vec::new();
        for row in rows {
            let id: String = row.get("id");
            let plugins = self.load_l3_plugins(&id).await?;
            routes.push(WafRoute {
                id,
                name: row.get("name"),
                priority: row.get::<i32, _>("priority"),
                enabled: row.get("enabled"),
                layer: "l3".into(),
                match_: RouteMatch {
                    src_ip: row.get("src_ip"),
                    dst_ip: row.get("dst_ip"),
                    dst_port: row.get::<Option<i32>, _>("dst_port").map(|p| p as u16),
                    pod: row.get("pod"),
                    l7: None,
                },
                plugins,
                upstream_id: None,
            });
        }
        Ok(routes)
    }

    async fn load_l3_plugins(&self, policy_id: &str) -> Result<Vec<RoutePluginRef>, ConfigError> {
        let rows = sqlx::query(
            r#"SELECT plugin_name, enabled, config
               FROM waf_l3_policy_plugins
               WHERE policy_id = $1 ORDER BY sort_order, id"#,
        )
        .bind(policy_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| ConfigError::Db(e.to_string()))?;

        Ok(rows
            .into_iter()
            .map(|row| RoutePluginRef {
                name: row.get("plugin_name"),
                enabled: row.get("enabled"),
                config: row.get::<Value, _>("config"),
            })
            .collect())
    }

    async fn load_l7_routes(&self) -> Result<Vec<WafRoute>, ConfigError> {
        let rows = sqlx::query(
            r#"SELECT id, name, priority, enabled, host, path_prefix, methods, uri_regex, upstream_id
               FROM waf_l7_routes WHERE site_id = $1 ORDER BY priority"#,
        )
        .bind(&self.site_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| ConfigError::Db(e.to_string()))?;

        let mut routes = Vec::new();
        for row in rows {
            let id: String = row.get("id");
            let plugins = self.load_l7_plugins(&id).await?;
            let methods: Option<Vec<String>> = row.get("methods");
            routes.push(WafRoute {
                id,
                name: row.get("name"),
                priority: row.get::<i32, _>("priority"),
                enabled: row.get("enabled"),
                layer: "l7".into(),
                match_: RouteMatch {
                    l7: Some(L7Match {
                        host: row.get("host"),
                        path_prefix: row.get("path_prefix"),
                        methods,
                        uri_regex: row.get("uri_regex"),
                    }),
                    ..Default::default()
                },
                plugins,
                upstream_id: row.get("upstream_id"),
            });
        }
        Ok(routes)
    }

    async fn load_l7_plugins(&self, route_id: &str) -> Result<Vec<RoutePluginRef>, ConfigError> {
        let rows = sqlx::query(
            r#"SELECT plugin_name, enabled, config
               FROM waf_l7_route_plugins
               WHERE route_id = $1 ORDER BY sort_order, id"#,
        )
        .bind(route_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| ConfigError::Db(e.to_string()))?;

        Ok(rows
            .into_iter()
            .map(|row| RoutePluginRef {
                name: row.get("plugin_name"),
                enabled: row.get("enabled"),
                config: row.get::<Value, _>("config"),
            })
            .collect())
    }

    pub async fn save_config(&self, config: &WafConfig, operator: &str) -> Result<(), ConfigError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| ConfigError::Db(e.to_string()))?;

        let mut l3_ids = Vec::new();
        let mut l7_ids = Vec::new();

        for route in &config.routes {
            let layer = route_layer(route);
            if layer == "l7" {
                l7_ids.push(route.id.clone());
                self.upsert_l7_route(&mut tx, route, operator).await?;
            } else {
                l3_ids.push(route.id.clone());
                self.upsert_l3_policy(&mut tx, route, operator).await?;
            }
        }

        sqlx::query(
            "DELETE FROM waf_l3_policies WHERE site_id = $1 AND NOT (id = ANY($2))",
        )
        .bind(&self.site_id)
        .bind(&l3_ids)
        .execute(tx.as_mut())
        .await
        .map_err(|e| ConfigError::Db(e.to_string()))?;

        sqlx::query(
            "DELETE FROM waf_l7_routes WHERE site_id = $1 AND NOT (id = ANY($2))",
        )
        .bind(&self.site_id)
        .bind(&l7_ids)
        .execute(tx.as_mut())
        .await
        .map_err(|e| ConfigError::Db(e.to_string()))?;

        self.write_revision(
            &mut tx,
            "routes",
            "all",
            "update",
            &json!(config),
            operator,
        )
        .await?;

        tx.commit()
            .await
            .map_err(|e| ConfigError::Db(e.to_string()))?;
        Ok(())
    }

    async fn upsert_l3_policy(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        route: &WafRoute,
        operator: &str,
    ) -> Result<(), ConfigError> {
        let m = route.route_match();
        sqlx::query(
            r#"INSERT INTO waf_l3_policies (
                id, site_id, name, priority, enabled,
                src_ip, dst_ip, dst_port, pod, updated_at, updated_by
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,now(),$10)
            ON CONFLICT (id) DO UPDATE SET
                site_id = EXCLUDED.site_id,
                name = EXCLUDED.name,
                priority = EXCLUDED.priority,
                enabled = EXCLUDED.enabled,
                src_ip = EXCLUDED.src_ip,
                dst_ip = EXCLUDED.dst_ip,
                dst_port = EXCLUDED.dst_port,
                pod = EXCLUDED.pod,
                updated_at = now(),
                updated_by = EXCLUDED.updated_by"#,
        )
        .bind(&route.id)
        .bind(&self.site_id)
        .bind(&route.name)
        .bind(route.priority)
        .bind(route.enabled)
        .bind(&m.src_ip)
        .bind(&m.dst_ip)
        .bind(m.dst_port.map(|p| p as i32))
        .bind(&m.pod)
        .bind(operator)
        .execute(tx.as_mut())
        .await
        .map_err(|e| ConfigError::Db(e.to_string()))?;

        sqlx::query("DELETE FROM waf_l3_policy_plugins WHERE policy_id = $1")
            .bind(&route.id)
            .execute(tx.as_mut())
            .await
            .map_err(|e| ConfigError::Db(e.to_string()))?;

        for (idx, plugin) in route.plugins.iter().enumerate() {
            sqlx::query(
                r#"INSERT INTO waf_l3_policy_plugins
                   (policy_id, plugin_name, enabled, sort_order, config, updated_at)
                   VALUES ($1,$2,$3,$4,$5,now())"#,
            )
            .bind(&route.id)
            .bind(&plugin.name)
            .bind(plugin.enabled)
            .bind(idx as i32)
            .bind(&plugin.config)
            .execute(tx.as_mut())
            .await
            .map_err(|e| ConfigError::Db(e.to_string()))?;
        }
        Ok(())
    }

    async fn upsert_l7_route(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        route: &WafRoute,
        operator: &str,
    ) -> Result<(), ConfigError> {
        let l7 = route.match_.l7.as_ref();
        sqlx::query(
            r#"INSERT INTO waf_l7_routes (
                id, site_id, name, priority, enabled,
                host, path_prefix, methods, uri_regex, upstream_id, updated_at, updated_by
            ) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,now(),$11)
            ON CONFLICT (id) DO UPDATE SET
                site_id = EXCLUDED.site_id,
                name = EXCLUDED.name,
                priority = EXCLUDED.priority,
                enabled = EXCLUDED.enabled,
                host = EXCLUDED.host,
                path_prefix = EXCLUDED.path_prefix,
                methods = EXCLUDED.methods,
                uri_regex = EXCLUDED.uri_regex,
                upstream_id = EXCLUDED.upstream_id,
                updated_at = now(),
                updated_by = EXCLUDED.updated_by"#,
        )
        .bind(&route.id)
        .bind(&self.site_id)
        .bind(&route.name)
        .bind(route.priority)
        .bind(route.enabled)
        .bind(&l7.and_then(|x| x.host.clone()))
        .bind(&l7.and_then(|x| x.path_prefix.clone()))
        .bind(&l7.and_then(|x| x.methods.clone()))
        .bind(&l7.and_then(|x| x.uri_regex.clone()))
        .bind(&route.upstream_id)
        .bind(operator)
        .execute(tx.as_mut())
        .await
        .map_err(|e| ConfigError::Db(e.to_string()))?;

        sqlx::query("DELETE FROM waf_l7_route_plugins WHERE route_id = $1")
            .bind(&route.id)
            .execute(tx.as_mut())
            .await
            .map_err(|e| ConfigError::Db(e.to_string()))?;

        for (idx, plugin) in route.plugins.iter().enumerate() {
            sqlx::query(
                r#"INSERT INTO waf_l7_route_plugins
                   (route_id, plugin_name, enabled, sort_order, config, updated_at)
                   VALUES ($1,$2,$3,$4,$5,now())"#,
            )
            .bind(&route.id)
            .bind(&plugin.name)
            .bind(plugin.enabled)
            .bind(idx as i32)
            .bind(&plugin.config)
            .execute(tx.as_mut())
            .await
            .map_err(|e| ConfigError::Db(e.to_string()))?;
        }
        Ok(())
    }

    pub async fn load_modules(&self) -> Result<ProtectionModules, ConfigError> {
        self.load_modules_for(&self.site_id).await
    }

    pub async fn load_modules_for(&self, site_id: &str) -> Result<ProtectionModules, ConfigError> {
        crate::app_config::pg::load_effective_protection(&self.pool, site_id).await
    }

    pub async fn load_global_modules(&self) -> Result<ProtectionModules, ConfigError> {
        crate::app_config::pg::load_default_protection(&self.pool).await
    }

    pub async fn save_global_modules(
        &self,
        modules: &ProtectionModules,
        operator: &str,
    ) -> Result<(), ConfigError> {
        let ctx = crate::config::AuditContext::from_legacy_operator(operator);
        crate::global_config::pg::save_global_protection(&self.pool, modules, &ctx).await
    }

    pub async fn load_global_pipeline(
        &self,
    ) -> Result<nerve_waf_semantic::SemanticConfig, ConfigError> {
        Ok(
            crate::global_config::pg::load_global_pipeline(&self.pool)
                .await?
                .unwrap_or_default(),
        )
    }

    pub async fn save_global_pipeline(
        &self,
        cfg: &nerve_waf_semantic::SemanticConfig,
        operator: &str,
    ) -> Result<(), ConfigError> {
        let ctx = crate::config::AuditContext::from_legacy_operator(operator);
        crate::global_config::pg::save_global_pipeline(&self.pool, cfg, None, &ctx).await
    }

    pub async fn save_modules(
        &self,
        modules: &ProtectionModules,
        operator: &str,
    ) -> Result<(), ConfigError> {
        self.save_modules_for(&self.site_id, modules, operator).await
    }

    pub async fn save_modules_for(
        &self,
        site_id: &str,
        modules: &ProtectionModules,
        operator: &str,
    ) -> Result<(), ConfigError> {
        let ctx = crate::config::AuditContext::from_legacy_operator(operator);
        crate::app_config::pg::ensure_app_provisioned(&self.pool, site_id, &ctx).await?;
        crate::app_config::pg::save_app_protection(&self.pool, site_id, modules, &ctx).await?;

        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| ConfigError::Db(e.to_string()))?;

        self.write_revision(
            &mut tx,
            "app_protection",
            site_id,
            "update",
            &serde_json::to_value(modules)?,
            operator,
        )
        .await?;

        tx.commit()
            .await
            .map_err(|e| ConfigError::Db(e.to_string()))?;
        Ok(())
    }

    pub async fn ensure_app_provisioned(&self, site_id: &str) -> Result<(), ConfigError> {
        crate::app_config::pg::ensure_app_provisioned(
            &self.pool,
            site_id,
            &crate::config::AuditContext::console(),
        )
        .await
    }

    pub async fn active_runtime_deny_ips(&self) -> Result<Vec<String>, ConfigError> {
        let rows = sqlx::query(
            r#"SELECT host(ip) AS ip FROM waf_runtime_denies
               WHERE site_id = $1 AND (expires_at IS NULL OR expires_at > now())
               ORDER BY created_at"#,
        )
        .bind(&self.site_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| ConfigError::Db(e.to_string()))?;

        Ok(rows.into_iter().map(|r| r.get::<String, _>("ip")).collect())
    }

    pub async fn list_runtime_denies(&self) -> Result<Vec<RuntimeDenyEntry>, ConfigError> {
        let rows = sqlx::query(
            r#"SELECT host(ip) AS ip, source, reason, expires_at, created_at
               FROM waf_runtime_denies
               WHERE site_id = $1 AND (expires_at IS NULL OR expires_at > now())
               ORDER BY created_at DESC"#,
        )
        .bind(&self.site_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| ConfigError::Db(e.to_string()))?;

        Ok(rows
            .into_iter()
            .map(|row| RuntimeDenyEntry {
                ip: row.get("ip"),
                source: row.get("source"),
                reason: row.get("reason"),
                expires_at: row.get("expires_at"),
                created_at: row.get("created_at"),
            })
            .collect())
    }

    pub async fn add_runtime_deny(
        &self,
        ip: &str,
        source: &str,
        reason: Option<&str>,
        operator: Option<&str>,
        expires_at: Option<chrono::DateTime<chrono::Utc>>,
    ) -> Result<(), ConfigError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| ConfigError::Db(e.to_string()))?;

        sqlx::query(
            r#"INSERT INTO waf_runtime_denies (ip, site_id, source, reason, expires_at, created_by)
               VALUES ($1::inet, $2, $3, $4, $5, $6)
               ON CONFLICT (ip) DO UPDATE SET
                 site_id = EXCLUDED.site_id,
                 source = EXCLUDED.source,
                 reason = EXCLUDED.reason,
                 expires_at = EXCLUDED.expires_at,
                 created_at = now(),
                 created_by = EXCLUDED.created_by"#,
        )
        .bind(ip)
        .bind(&self.site_id)
        .bind(source)
        .bind(reason)
        .bind(expires_at)
        .bind(operator)
        .execute(tx.as_mut())
        .await
        .map_err(|e| ConfigError::Db(e.to_string()))?;

        self.write_revision(
            &mut tx,
            "runtime_deny",
            ip,
            "upsert",
            &json!({ "ip": ip, "source": source, "reason": reason, "expires_at": expires_at }),
            operator.unwrap_or("system"),
        )
        .await?;

        tx.commit()
            .await
            .map_err(|e| ConfigError::Db(e.to_string()))?;
        Ok(())
    }

    pub async fn remove_runtime_deny(&self, ip: &str) -> Result<(), ConfigError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| ConfigError::Db(e.to_string()))?;

        sqlx::query("DELETE FROM waf_runtime_denies WHERE ip = $1::inet AND site_id = $2")
            .bind(ip)
            .bind(&self.site_id)
            .execute(tx.as_mut())
            .await
            .map_err(|e| ConfigError::Db(e.to_string()))?;

        self.write_revision(
            &mut tx,
            "runtime_deny",
            ip,
            "delete",
            &json!({ "ip": ip }),
            "system",
        )
        .await?;

        tx.commit()
            .await
            .map_err(|e| ConfigError::Db(e.to_string()))?;
        Ok(())
    }

    pub async fn delete_route(&self, id: &str) -> Result<(), ConfigError> {
        sqlx::query("DELETE FROM waf_l3_policies WHERE id = $1 AND site_id = $2")
            .bind(id)
            .bind(&self.site_id)
            .execute(&self.pool)
            .await
            .map_err(|e| ConfigError::Db(e.to_string()))?;

        sqlx::query("DELETE FROM waf_l7_routes WHERE id = $1 AND site_id = $2")
            .bind(id)
            .bind(&self.site_id)
            .execute(&self.pool)
            .await
            .map_err(|e| ConfigError::Db(e.to_string()))?;
        Ok(())
    }

    pub async fn load_upstreams(&self) -> Result<Vec<L7Upstream>, ConfigError> {
        let rows = sqlx::query(
            r#"SELECT id, name, enabled, scheme, upstream_type, nodes, description
               FROM waf_l7_upstreams WHERE site_id = $1 ORDER BY name"#,
        )
        .bind(&self.site_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| ConfigError::Db(e.to_string()))?;

        Ok(rows
            .into_iter()
            .map(|row| L7Upstream {
                id: row.get("id"),
                name: row.get("name"),
                enabled: row.get("enabled"),
                scheme: row.get("scheme"),
                upstream_type: row.get("upstream_type"),
                nodes: row
                    .get::<Value, _>("nodes")
                    .as_array()
                    .map(|a| {
                        a.iter()
                            .filter_map(|v| serde_json::from_value(v.clone()).ok())
                            .collect()
                    })
                    .unwrap_or_default(),
                description: row.get("description"),
                host_header: None,
                sni: None,
                transport: None,
                origin_kind: None,
                checks: None,
                health_check: None,
            })
            .collect())
    }

    pub async fn save_upstreams(
        &self,
        upstreams: &[L7Upstream],
        operator: &str,
    ) -> Result<(), ConfigError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| ConfigError::Db(e.to_string()))?;

        let ids: Vec<String> = upstreams.iter().map(|u| u.id.clone()).collect();
        for u in upstreams {
            let nodes = serde_json::to_value(&u.nodes).unwrap_or(json!([]));
            sqlx::query(
                r#"INSERT INTO waf_l7_upstreams
                   (id, site_id, name, enabled, scheme, upstream_type, nodes, description, updated_at, updated_by)
                   VALUES ($1,$2,$3,$4,$5,$6,$7,$8,now(),$9)
                   ON CONFLICT (id) DO UPDATE SET
                     site_id = EXCLUDED.site_id,
                     name = EXCLUDED.name,
                     enabled = EXCLUDED.enabled,
                     scheme = EXCLUDED.scheme,
                     upstream_type = EXCLUDED.upstream_type,
                     nodes = EXCLUDED.nodes,
                     description = EXCLUDED.description,
                     updated_at = now(),
                     updated_by = EXCLUDED.updated_by"#,
            )
            .bind(&u.id)
            .bind(&self.site_id)
            .bind(&u.name)
            .bind(u.enabled)
            .bind(&u.scheme)
            .bind(&u.upstream_type)
            .bind(nodes)
            .bind(&u.description)
            .bind(operator)
            .execute(tx.as_mut())
            .await
            .map_err(|e| ConfigError::Db(e.to_string()))?;
        }

        sqlx::query("DELETE FROM waf_l7_upstreams WHERE site_id = $1 AND NOT (id = ANY($2))")
            .bind(&self.site_id)
            .bind(&ids)
            .execute(tx.as_mut())
            .await
            .map_err(|e| ConfigError::Db(e.to_string()))?;

        tx.commit()
            .await
            .map_err(|e| ConfigError::Db(e.to_string()))?;
        Ok(())
    }

    pub async fn load_gateway_dataplane(
        &self,
    ) -> Result<(Vec<WafRoute>, Vec<L7Upstream>), ConfigError> {
        let routes = self
            .load_l7_routes()
            .await?
            .into_iter()
            .filter(|r| r.enabled)
            .collect();
        let upstreams = self
            .load_upstreams()
            .await?
            .into_iter()
            .filter(|u| u.enabled)
            .collect();
        Ok((routes, upstreams))
    }

    async fn write_revision(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        kind: &str,
        resource_id: &str,
        operation: &str,
        payload: &Value,
        operator: &str,
    ) -> Result<(), ConfigError> {
        let revision: i32 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(revision), 0) + 1 FROM waf_config_revisions WHERE kind = $1 AND resource_id = $2",
        )
        .bind(kind)
        .bind(resource_id)
        .fetch_one(tx.as_mut())
        .await
        .map_err(|e| ConfigError::Db(e.to_string()))?;

        sqlx::query(
            r#"INSERT INTO waf_config_revisions
               (site_id, kind, resource_id, revision, operation, payload, operator)
               VALUES ($1,$2,$3,$4,$5,$6,$7)"#,
        )
        .bind(&self.site_id)
        .bind(kind)
        .bind(resource_id)
        .bind(revision)
        .bind(operation)
        .bind(payload)
        .bind(operator)
        .execute(tx.as_mut())
        .await
        .map_err(|e| ConfigError::Db(e.to_string()))?;
        Ok(())
    }

    pub async fn load_app_crs_binding(&self, site_id: &str) -> Result<Option<crate::AppCrsBinding>, ConfigError> {
        crate::app_crs::pg::load_binding(&self.pool, site_id).await
    }

    pub async fn load_app_crs_history(&self, site_id: &str) -> Result<Vec<crate::AppCrsHistoryEntry>, ConfigError> {
        crate::app_crs::pg::load_history(&self.pool, site_id).await
    }

    pub async fn load_app_crs_history_entry(
        &self,
        site_id: &str,
        history_id: &str,
    ) -> Result<Option<crate::AppCrsHistoryEntry>, ConfigError> {
        crate::app_crs::pg::load_history_entry(&self.pool, site_id, history_id).await
    }

    pub async fn apply_app_crs_version(
        &self,
        site_id: &str,
        crs_version_id: &str,
        crs_version_label: Option<&str>,
        rule_count: u32,
        modules_snapshot: std::collections::HashMap<String, nerve_waf_sdk::ModuleConfig>,
        operator: &str,
    ) -> Result<crate::AppCrsBinding, ConfigError> {
        crate::app_crs::pg::apply_version(
            &self.pool,
            site_id,
            crs_version_id,
            crs_version_label,
            rule_count,
            modules_snapshot,
            operator,
        )
        .await
    }
}

fn route_layer(route: &WafRoute) -> &'static str {
    if route.layer == "l7" {
        "l7"
    } else if route.layer == "l3" {
        "l3"
    } else if route
        .match_
        .l7
        .as_ref()
        .is_some_and(|l7| {
            l7.host.is_some()
                || l7.path_prefix.is_some()
                || l7.methods.as_ref().is_some_and(|m| !m.is_empty())
                || l7.uri_regex.is_some()
        }) {
        "l7"
    } else {
        "l3"
    }
}





















