import { useCallback, useEffect, useMemo, useState } from "react";
import {
  NerveAlert,
  NerveButton,
  NerveEmpty,
  NervePageHeader,
  NerveSelect,
  NerveTableToolbar,
  nerveConfirm,
  nerveResult,
  useToast,
} from "@nerve/ui";
import { apiGet, apiPut } from "../api";
import { AppConfigDrawer } from "../features/app-config/AppConfigDrawer";
import {
  siteIngressMode,
  siteSupportsOriginConfig,
} from "../features/ingress/ingress-display";
import { OriginConfigDrawer } from "../features/ingress/OriginConfigDrawer";
import {
  moduleShortLabel,
  moduleTagClasses,
  moduleTagSuffix,
  moduleTagTitle,
  orderedCapabilityKeys,
} from "../lib/module-display";
import type { ProtectionModules } from "../lib/waf-types";
import { loadSites, navigateToAppEvents, type ProtectedSite } from "../sites";

type IngressFilter = "all" | "waf-edge" | "nerve-gateway";

function SiteCapabilityTags({ modules }: { modules: ProtectionModules }) {
  const keys = orderedCapabilityKeys(modules.modules);
  return (
    <span className="cap-tags module-cap-tags">
      {keys.map((key) => {
        const cfg = modules.modules[key];
        if (!cfg) return null;
        return (
          <span
            key={key}
            className={moduleTagClasses(cfg, modules.mode)}
            title={moduleTagTitle(key, cfg, modules.mode)}
          >
            {moduleShortLabel(key)}
            {moduleTagSuffix(cfg, modules.mode)}
          </span>
        );
      })}
    </span>
  );
}

function matchesFilter(site: ProtectedSite, filter: IngressFilter): boolean {
  const mode = siteIngressMode(site);
  if (filter === "waf-edge") return mode === "waf-edge";
  if (filter === "nerve-gateway") return mode === "nerve-gateway";
  return true;
}

export function AssetsPage() {
  const toast = useToast();
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [sites, setSites] = useState<ProtectedSite[]>([]);
  const [modulesBySite, setModulesBySite] = useState<Record<string, ProtectionModules>>({});
  const [configIndex, setConfigIndex] = useState<number | null>(null);
  const [originIndex, setOriginIndex] = useState<number | null>(null);
  const [ingressFilter, setIngressFilter] = useState<IngressFilter>("all");

  const loadModulesSummary = useCallback(async (list: ProtectedSite[]) => {
    const map: Record<string, ProtectionModules> = {};
    await Promise.all(
      list.map(async (s) => {
        try {
          map[s.id] = await apiGet<ProtectionModules>(`/api/modules?site_id=${encodeURIComponent(s.id)}`);
        } catch {
          map[s.id] = { version: 1, mode: "block", modules: {} };
        }
      }),
    );
    setModulesBySite(map);
  }, []);

  const refresh = useCallback(async () => {
    setLoading(true);
    setError(null);
    try {
      const list = await loadSites();
      setSites(list);
      await loadModulesSummary(list);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      toast(String(e), false);
    } finally {
      setLoading(false);
    }
  }, [loadModulesSummary, toast]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  const openConfig = (index: number) => {
    if (index < 0 || index >= sites.length) return;
    setOriginIndex(null);
    setConfigIndex(index);
  };

  const openOrigin = (index: number) => {
    if (index < 0 || index >= sites.length) return;
    const site = sites[index];
    if (!siteSupportsOriginConfig(site)) {
      void nerveResult({
        type: "info",
        title: "无法配置回源",
        message: "请先在「配置应用」填写具体防护域名（不能仅为 *）。",
      });
      return;
    }
    setConfigIndex(null);
    setOriginIndex(index);
  };

  const handleDelete = async (index: number) => {
    const s = sites[index];
    if (s.id === "default") return;
    const ok = await nerveConfirm({
      title: "删除应用",
      message: `删除应用「${s.name.trim() || s.id}」？`,
      okText: "删除",
      danger: true,
    });
    if (!ok) return;
    const next = sites.filter((_, i) => i !== index);
    try {
      await apiPut("/api/sites", next);
      toast("已删除");
      await refresh();
    } catch (e) {
      toast(e instanceof Error ? e.message : String(e), false);
    }
  };

  const handleAdd = () => {
    const id = `app-${Date.now().toString(36)}`;
    const next = [
      ...sites,
      {
        id,
        name: "",
        hosts: [],
        enabled: true,
        ingress_mode: "waf-edge",
        integration: "waf-edge",
      },
    ];
    setSites(next);
    setConfigIndex(next.length - 1);
  };

  const enabledCount = sites.filter((s) => s.enabled).length;
  const visibleSites = useMemo(
    () => sites.map((s, i) => ({ site: s, index: i })).filter(({ site }) => matchesFilter(site, ingressFilter)),
    [sites, ingressFilter],
  );

  return (
    <>
      <NervePageHeader title="防护应用" />
      <NerveTableToolbar>
        <NerveButton onClick={() => void refresh()}>刷新</NerveButton>
        <NerveSelect
          value={ingressFilter}
          onChange={(e) => setIngressFilter(e.target.value as IngressFilter)}
          options={[
            { value: "all", label: "全部" },
            { value: "waf-edge", label: "WAF 边缘回源" },
            { value: "nerve-gateway", label: "Gateway 穿透" },
          ]}
        />
        <NerveButton variant="primary" onClick={handleAdd}>
          + 新增应用
        </NerveButton>
      </NerveTableToolbar>

      {error ? <NerveAlert tone="error">{error}</NerveAlert> : null}
      {loading ? <div className="page-loading">加载中…</div> : null}

      {!loading ? (
        <>
          <div className="asset-summary-bar">
            <span>
              共 <strong>{sites.length}</strong> 个应用
            </span>
            <span>
              <strong>{enabledCount}</strong> 个防护中
            </span>
          </div>

          {sites.length === 0 ? (
            <NerveEmpty description="暂无应用" />
          ) : visibleSites.length === 0 ? (
            <NerveEmpty description="当前筛选下暂无应用" />
          ) : (
            <div className="asset-grid" id="assets-cards">
              {visibleSites.map(({ site: s, index: i }) => {
                const mod = modulesBySite[s.id] ?? { version: 1, mode: "block", modules: {} };
                const note = s.description || s.upstream;
                const originConfigurable = siteSupportsOriginConfig(s);
                return (
                  <div
                    key={s.id}
                    className={`asset-card ${s.enabled ? "protected" : "paused"}`}
                  >
                    <div className="asset-card-accent" />
                    <span className={`asset-status-tag ant-tag${s.enabled ? " ant-tag-success" : ""}`}>
                      {s.enabled ? "防护中" : "已暂停"}
                    </span>
                    <div className="asset-card-body">
                      <div className="asset-card-top">
                        <div className="asset-icon">{s.enabled ? "🛡" : "⏸"}</div>
                        <div>
                          <h3 className="asset-card-title">{s.name.trim() || "未命名应用"}</h3>
                          <div className="muted asset-card-id">{s.id}</div>
                        </div>
                      </div>
                      <div className="asset-card-kv">
                        <div className="asset-field">
                          <span className="lbl">域名</span>
                          <span>{s.hosts.length ? s.hosts.join(", ") : "—"}</span>
                        </div>
                        {note ? (
                          <div className="asset-field">
                            <span className="lbl">备注</span>
                            <code className="small">{note}</code>
                          </div>
                        ) : null}
                        <div className="asset-field">
                          <span className="lbl">模块</span>
                          <SiteCapabilityTags modules={mod} />
                        </div>
                      </div>
                      <div className="asset-card-actions">
                        <div className="asset-card-actions-left">
                          <NerveButton
                            size="sm"
                            variant="primary"
                            onClick={() => openConfig(i)}
                          >
                            配置应用
                          </NerveButton>
                          {originConfigurable ? (
                            <NerveButton size="sm" onClick={() => openOrigin(i)}>
                              回源配置
                            </NerveButton>
                          ) : null}
                          {s.id !== "default" ? (
                            <NerveButton
                              size="sm"
                              variant="danger"
                              onClick={() => void handleDelete(i)}
                            >
                              删除
                            </NerveButton>
                          ) : null}
                        </div>
                        <div className="asset-card-actions-right">
                          <NerveButton
                            size="sm"
                            className="asset-events-btn"
                            title="查看本应用攻击记录"
                            onClick={() => navigateToAppEvents(s.id)}
                          >
                            攻击记录
                          </NerveButton>
                        </div>
                      </div>
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </>
      ) : null}

      {configIndex != null && sites[configIndex] ? (
        <AppConfigDrawer
          open
          site={sites[configIndex]}
          sites={sites}
          siteIndex={configIndex}
          onClose={() => {
            setConfigIndex(null);
            // 关闭抽屉后从服务端重载，丢掉未点「保存并生效」的本地草稿
            void refresh();
          }}
          onSitesChange={setSites}
          onSaved={() => void refresh()}
        />
      ) : null}

      {originIndex != null && sites[originIndex] ? (
        <OriginConfigDrawer
          open
          site={sites[originIndex]}
          onClose={() => setOriginIndex(null)}
          onSaved={() => void refresh()}
          onConfigureApp={() => {
            const i = originIndex;
            setOriginIndex(null);
            setConfigIndex(i);
          }}
        />
      ) : null}
    </>
  );
}
