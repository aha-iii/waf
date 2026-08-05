import {
  NerveButton,
  NerveCheckbox,
  NerveFormItem,
  NerveInput,
  NerveTextarea,
} from "@nerve/ui";
import { GATEWAY_DASHBOARD_URL } from "../../nav";
import type { IngressMode, SiteIngress } from "./ingress-types";

type Props = {
  mode: IngressMode;
  ingress: SiteIngress;
  tlsCert: string;
  tlsKey: string;
  onTlsModeChange: (mode: "auto" | "upload") => void;
  onTlsCertChange: (v: string) => void;
  onTlsKeyChange: (v: string) => void;
  onWebsocketChange: (v: boolean) => void;
  onXffChange: (v: number) => void;
};

export function CertificatePanel({
  mode,
  ingress,
  tlsCert,
  tlsKey,
  onTlsModeChange,
  onTlsCertChange,
  onTlsKeyChange,
  onWebsocketChange,
  onXffChange,
}: Props) {
  const dashboardUrl = ingress.gateway_dashboard_url || GATEWAY_DASHBOARD_URL;

  if (mode === "nerve-gateway") {
    return (
      <div className="form-stack">
        <NerveButton
          variant="link"
          onClick={() => window.open(dashboardUrl, "_blank", "noopener,noreferrer")}
        >
          Gateway SSL ↗
        </NerveButton>
        <NerveCheckbox
          label="WebSocket 透传"
          checked={ingress.websocket_enabled}
          onChange={(e) => onWebsocketChange(e.target.checked)}
        />
        <NerveFormItem label="XFF 信任跳数">
          <NerveInput
            type="number"
            wide
            value={String(ingress.xff_trusted_hops)}
            onChange={(e) => onXffChange(Math.max(0, Number(e.target.value) || 1))}
          />
        </NerveFormItem>
      </div>
    );
  }

  const tlsMode = ingress.tls_mode === "upload" ? "upload" : "auto";

  return (
    <div className="form-stack">
      <div className="origin-copy-row">
        <NerveButton
          variant={tlsMode === "auto" ? "primary" : "default"}
          onClick={() => onTlsModeChange("auto")}
        >
          ACME 自动
        </NerveButton>
        <NerveButton
          variant={tlsMode === "upload" ? "primary" : "default"}
          onClick={() => onTlsModeChange("upload")}
        >
          上传证书
        </NerveButton>
      </div>

      {tlsMode === "upload" ? (
        <>
          <NerveFormItem label="证书 PEM">
            <NerveTextarea
              wide
              rows={5}
              value={tlsCert}
              placeholder="-----BEGIN CERTIFICATE-----"
              onChange={(e) => onTlsCertChange(e.target.value)}
            />
          </NerveFormItem>
          <NerveFormItem label="私钥 PEM">
            <NerveTextarea
              wide
              rows={5}
              value={tlsKey}
              placeholder="-----BEGIN PRIVATE KEY-----"
              onChange={(e) => onTlsKeyChange(e.target.value)}
            />
          </NerveFormItem>
        </>
      ) : null}

      <NerveCheckbox
        label="WebSocket"
        checked={ingress.websocket_enabled}
        onChange={(e) => onWebsocketChange(e.target.checked)}
      />
      <NerveFormItem label="XFF 信任跳数">
        <NerveInput
          type="number"
          wide
          value={String(ingress.xff_trusted_hops)}
          onChange={(e) => onXffChange(Math.max(0, Number(e.target.value) || 1))}
        />
      </NerveFormItem>
    </div>
  );
}
