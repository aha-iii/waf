/** access feature: Temp Ban Dialog. */
import { useEffect, useState } from "react";
import {
  NerveButton,
  NerveForm,
  NerveFormItem,
  NerveInput,
  NerveModal,
  nerveResult,
} from "@nerve/ui";
import { apiTempDeny } from "../../api";
import { formatDateTimeZh } from "../../lib/datetime";
import { defaultExpiresLocal, localDatetimeToIso } from "../../lib/temp-deny";

export type TempBanDialogProps = {
  open: boolean;
  /** Prefill IP; empty allows typing in the dialog */
  ip?: string;
  source?: string;
  defaultReason?: string;
  onClose: () => void;
  onDone?: () => void;
};

/** Temporary-ban dialog: optional expiry / reason */
export function TempBanDialog({
  open,
  ip: ipProp = "",
  source = "console",
  defaultReason = "",
  onClose,
  onDone,
}: TempBanDialogProps) {
  const ipLocked = !!ipProp.trim();
  const [ip, setIp] = useState(ipProp);
  const [expiresLocal, setExpiresLocal] = useState(() => defaultExpiresLocal(24));
  const [reason, setReason] = useState(defaultReason);
  const [noExpire, setNoExpire] = useState(false);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!open) return;
    setIp(ipProp);
    setExpiresLocal(defaultExpiresLocal(24));
    setReason(defaultReason);
    setNoExpire(false);
  }, [open, ipProp, defaultReason]);

  const submit = async () => {
    const target = ip.trim();
    if (!target) {
      await nerveResult({ type: "error", title: "无法封禁", message: "请输入源 IP" });
      return;
    }
    const expires_at = noExpire ? undefined : localDatetimeToIso(expiresLocal);
    if (!noExpire && expiresLocal.trim() && !expires_at) {
      await nerveResult({ type: "error", title: "到期时间无效", message: "请检查到期时间格式" });
      return;
    }
    setSaving(true);
    try {
      await apiTempDeny(target, {
        source,
        reason: reason.trim() || undefined,
        expires_at,
      });
      await nerveResult({
        type: "success",
        title: "已临时封禁",
        message: expires_at
          ? `${target} 已临时封禁，到期 ${formatDateTimeZh(expires_at)}`
          : `${target} 已临时封禁（无到期，需手动解封）`,
      });
      onDone?.();
      onClose();
    } catch (e) {
      await nerveResult({ type: "error", title: "封禁失败", message: String(e) });
    } finally {
      setSaving(false);
    }
  };

  return (
    <NerveModal
      open={open}
      title={ipLocked ? `临时封禁 ${ipProp}` : "临时封禁"}
      onClose={onClose}
      footer={
        <>
          <NerveButton onClick={onClose} disabled={saving}>
            取消
          </NerveButton>
          <NerveButton variant="danger" disabled={saving} onClick={() => void submit()}>
            {saving ? "封禁中…" : "确认封禁"}
          </NerveButton>
        </>
      }
    >
      <NerveForm>
        <NerveFormItem label="源 IP" required>
          {ipLocked ? (
            <code>{ipProp}</code>
          ) : (
            <NerveInput
              wide
              value={ip}
              placeholder="请输入源 IP"
              onChange={(e) => setIp(e.target.value)}
            />
          )}
        </NerveFormItem>
        <NerveFormItem label="到期时间" required={!noExpire}>
          <NerveInput
            wide
            type="datetime-local"
            disabled={noExpire}
            value={expiresLocal}
            onChange={(e) => setExpiresLocal(e.target.value)}
          />
        </NerveFormItem>
        <label
          className="sem-check-row"
          style={{ display: "flex", gap: 8, alignItems: "center", marginBottom: 12 }}
        >
          <input
            type="checkbox"
            checked={noExpire}
            onChange={(e) => setNoExpire(e.target.checked)}
          />
          <span>不设到期（需在「临时封禁」中手动解封）</span>
        </label>
        <NerveFormItem label="原因（可选）">
          <NerveInput
            wide
            value={reason}
            placeholder="手动封禁 / 攻击日志处置"
            onChange={(e) => setReason(e.target.value)}
          />
        </NerveFormItem>
        <p className="muted small">写入运行时临时封禁，不改插件配置；可在访问控制中解除。</p>
      </NerveForm>
    </NerveModal>
  );
}
