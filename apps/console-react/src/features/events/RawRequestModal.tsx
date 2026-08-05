import { NerveModal } from "@nerve/ui";
import type { WafEvent } from "../../lib/waf-types";

type Props = {
  open: boolean;
  event: WafEvent | null;
  onClose: () => void;
};

function buildUrl(event: WafEvent): string {
  const host = event.host?.trim() || "—";
  const uri = event.uri?.trim() || "";
  return `${host}${uri}`;
}

function parseQuery(uri?: string): string {
  if (!uri) return "—";
  const idx = uri.indexOf("?");
  if (idx === -1) return "—";
  return uri.slice(idx + 1) || "—";
}

function formatHeaders(raw?: string): { name: string; value: string }[] {
  if (!raw) return [];
  return raw
    .split("\n")
    .map((line) => {
      const idx = line.indexOf(":");
      if (idx === -1) return { name: line.trim(), value: "" };
      return {
        name: line.slice(0, idx).trim(),
        value: line.slice(idx + 1).trim(),
      };
    })
    .filter((h) => h.name);
}

export function RawRequestModal({ open, event, onClose }: Props) {
  if (!event) return null;

  const url = buildUrl(event);
  const query = parseQuery(event.uri);
  const headerRows = formatHeaders(event.headers);

  return (
    <NerveModal
      open={open}
      title="原始请求"
      onClose={onClose}
      xl
    >
      <div className="raw-request-modal">
        <table className="data-table raw-request-table">
          <tbody>
            <tr>
              <td className="muted small" style={{ width: 80 }}>
                URL
              </td>
              <td>
                <code>{url}</code>
              </td>
            </tr>
            <tr>
              <td className="muted small">Method</td>
              <td>
                <code>{event.method ?? "—"}</code>
              </td>
            </tr>
            <tr>
              <td className="muted small">Query</td>
              <td>
                <code className="raw-request-query">{query}</code>
              </td>
            </tr>
          </tbody>
        </table>

        <div className="raw-request-section">
          <h4 className="small">Headers</h4>
          {headerRows.length ? (
            <table className="data-table raw-request-headers">
              <tbody>
                {headerRows.map((h, i) => (
                  <tr key={i}>
                    <td className="muted small" style={{ width: 160 }}>
                      {h.name}
                    </td>
                    <td>
                      <code>{h.value}</code>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          ) : (
            <span className="muted">—</span>
          )}
        </div>

        <div className="raw-request-section">
          <h4 className="small">Body</h4>
          {event.body ? (
            <pre className="raw-request-body">{event.body}</pre>
          ) : (
            <span className="muted">—</span>
          )}
        </div>
      </div>
    </NerveModal>
  );
}
