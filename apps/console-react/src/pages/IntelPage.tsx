import { useCallback, useEffect, useState } from "react";
import {
  NerveButton,
  NerveCard,
  NerveForm,
  NerveInput,
  NervePageHeader,
  NervePagination,
  NerveTable,
  NerveTableActions,
  NerveTableToolbar,
  NERVE_COL_ACTIONS,
  defaultPaginationState,
  paginate,
} from "@nerve/ui";
import { apiGet, apiPost, apiPut } from "../api";
import { useToast } from "../hooks/useToast";

type IntelEntry = { ip: string; category: string; severity?: string; note?: string };

export function IntelPage(): JSX.Element {
  const toast = useToast();
  const [entries, setEntries] = useState<IntelEntry[]>([]);
  const [page, setPage] = useState(defaultPaginationState());
  const [lookupIp, setLookupIp] = useState("");
  const [lookupOut, setLookupOut] = useState("—");
  const [loading, setLoading] = useState(true);

  const load = useCallback(async () => {
    const res = await apiGet<{ entries: IntelEntry[] }>("/api/intel/feed");
    setEntries(res.entries ?? []);
  }, []);

  useEffect(() => {
    void load().catch((e) => toast(String(e), false)).finally(() => setLoading(false));
  }, [load, toast]);

  const updateEntry = (index: number, field: keyof IntelEntry, value: string) => {
    setEntries((prev) => prev.map((e, i) => (i === index ? { ...e, [field]: value } : e)));
  };

  const addEntry = () => {
    setEntries((prev) => [...prev, { ip: "0.0.0.0", category: "malware", severity: "high", note: "" }]);
  };

  const removeEntry = (index: number) => {
    setEntries((prev) => prev.filter((_, i) => i !== index));
  };

  const save = async () => {
    try {
      await apiPut("/api/intel/feed", entries);
      toast("威胁情报已保存");
    } catch (e) {
      toast(String(e), false);
    }
  };

  const lookup = async () => {
    try {
      const r = await apiPost("/api/intel/lookup", { ip: lookupIp.trim() });
      setLookupOut(JSON.stringify(r, null, 2));
    } catch (e) {
      toast(String(e), false);
    }
  };

  const rows = paginate(entries, page.page, page.pageSize);
  const rowOffset = (page.page - 1) * page.pageSize;

  return (
    <>
      <NervePageHeader title="威胁情报" />
      <NerveTableToolbar>
        <NerveButton onClick={addEntry}>+ 添加 IP</NerveButton>
        <NerveButton variant="primary" onClick={() => void save()}>保存情报库</NerveButton>
      </NerveTableToolbar>

      <div className="dash-grid-2">
        <NerveCard title="情报条目" tip={`${entries.length} 条`}>
          {loading ? <p className="muted">加载中…</p> : (
            <>
              <NerveTable
                columns={[
                  {
                    key: "ip",
                    title: "IP",
                    render: (r, i) => (
                      <NerveInput
                        sm
                        value={r.ip}
                        onChange={(e) => updateEntry(rowOffset + i, "ip", e.target.value)}
                      />
                    ),
                  },
                  {
                    key: "category",
                    title: "类别",
                    render: (r, i) => (
                      <NerveInput
                        sm
                        value={r.category}
                        onChange={(e) => updateEntry(rowOffset + i, "category", e.target.value)}
                      />
                    ),
                  },
                  {
                    key: "severity",
                    title: "严重度",
                    render: (r, i) => (
                      <NerveInput
                        sm
                        value={r.severity ?? "high"}
                        onChange={(e) => updateEntry(rowOffset + i, "severity", e.target.value)}
                      />
                    ),
                  },
                  {
                    key: "note",
                    title: "备注",
                    render: (r, i) => (
                      <NerveInput
                        value={r.note ?? ""}
                        onChange={(e) => updateEntry(rowOffset + i, "note", e.target.value)}
                      />
                    ),
                  },
                  {
                    key: "actions",
                    title: "",
                    className: NERVE_COL_ACTIONS,
                    render: (_, i) => (
                      <NerveTableActions
                        actions={[
                          {
                            key: "del",
                            label: "删除",
                            danger: true,
                            onClick: () => removeEntry(rowOffset + i),
                          },
                        ]}
                      />
                    ),
                  },
                ]}
                dataSource={rows}
                rowKey={(_, i) => String(rowOffset + i)}
                emptyText="暂无情报，点击「添加 IP」"
              />
              <NervePagination total={entries.length} page={page.page} pageSize={page.pageSize} onChange={setPage} />
            </>
          )}
        </NerveCard>

        <NerveCard title="情报查询">
          <NerveForm>
            <NerveInput wide value={lookupIp} onChange={(e) => setLookupIp(e.target.value)} placeholder="1.2.3.4" />
            <NerveButton onClick={() => void lookup()}>查询</NerveButton>
            <pre className="json-view">{lookupOut}</pre>
          </NerveForm>
        </NerveCard>
      </div>
    </>
  );
}
