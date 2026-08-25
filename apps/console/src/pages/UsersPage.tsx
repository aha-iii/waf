/** Console page: Console user accounts and roles. */
import { useCallback, useEffect, useMemo, useState } from "react";
import {
  NerveButton,
  NerveCard,
  NerveForm,
  NerveFormItem,
  NerveInput,
  NerveModal,
  NervePageHeader,
  NerveSelect,
  NerveSwitch,
  NerveTable,
  NerveTableActions,
  NerveTag,
  NERVE_COL_ACTIONS,
  nerveConfirm,
} from "@nerve/ui";
import { apiDelete, apiGet, apiPost, apiPut } from "../api";
import { isAdmin } from "../auth";
import { useToast } from "../hooks/useToast";
import { formatDateTimeZh } from "../lib/datetime";

type ConsoleUser = {
  id: string;
  username: string;
  role: string;
  enabled: boolean;
  display_name?: string | null;
  created_at: string;
  updated_at: string;
};

const ROLE_OPTIONS = [
  { value: "admin", label: "管理员 (admin)" },
  { value: "operator", label: "运维 (operator)" },
  { value: "viewer", label: "只读 (viewer)" },
];

const ROLE_LABEL: Record<string, string> = {
  admin: "管理员",
  operator: "运维",
  viewer: "只读",
};

type FormMode = "create" | "edit" | null;

export function UsersPage(): JSX.Element {
  const toast = useToast();
  const [users, setUsers] = useState<ConsoleUser[]>([]);
  const [loading, setLoading] = useState(false);
  const [formMode, setFormMode] = useState<FormMode>(null);
  const [editing, setEditing] = useState<ConsoleUser | null>(null);
  const [username, setUsername] = useState("");
  const [password, setPassword] = useState("");
  const [role, setRole] = useState("viewer");
  const [enabled, setEnabled] = useState(true);
  const [displayName, setDisplayName] = useState("");
  const [saving, setSaving] = useState(false);

  const admin = isAdmin();

  const load = useCallback(async () => {
    const data = await apiGet<{ users: ConsoleUser[] }>("/api/users");
    setUsers(data.users ?? []);
  }, []);

  useEffect(() => {
    if (!admin) return;
    setLoading(true);
    void load()
      .catch((e) => toast(String(e), false))
      .finally(() => setLoading(false));
  }, [admin, load, toast]);

  const openCreate = () => {
    setFormMode("create");
    setEditing(null);
    setUsername("");
    setPassword("");
    setRole("viewer");
    setEnabled(true);
    setDisplayName("");
  };

  const openEdit = (user: ConsoleUser) => {
    setFormMode("edit");
    setEditing(user);
    setUsername(user.username);
    setPassword("");
    setRole(user.role);
    setEnabled(user.enabled);
    setDisplayName(user.display_name ?? "");
  };

  const closeForm = () => {
    setFormMode(null);
    setEditing(null);
  };

  const handleSave = async () => {
    const name = username.trim();
    if (!name) return toast("请输入用户名", false);
    setSaving(true);
    try {
      if (formMode === "create") {
        if (password.length < 4) return toast("密码至少 4 位", false);
        await apiPost("/api/users", {
          username: name,
          password,
          role,
          display_name: displayName.trim() || undefined,
        });
        toast("用户已创建");
      } else if (formMode === "edit" && editing) {
        await apiPut(`/api/users/${encodeURIComponent(editing.id)}`, {
          role,
          enabled,
          password: password.trim() || undefined,
          display_name: displayName.trim() || undefined,
          clear_display_name: !displayName.trim(),
        });
        toast("用户已更新");
      }
      closeForm();
      await load();
    } catch (e) {
      toast(String(e), false);
    } finally {
      setSaving(false);
    }
  };

  const handleDelete = async (user: ConsoleUser) => {
    const ok = await nerveConfirm({
      title: "删除用户",
      message: `确定删除用户「${user.username}」？此操作不可恢复。`,
      okText: "删除",
      danger: true,
    });
    if (!ok) return;
    try {
      await apiDelete(`/api/users/${encodeURIComponent(user.id)}`);
      toast("用户已删除");
      await load();
    } catch (e) {
      toast(String(e), false);
    }
  };

  const columns = useMemo(
    () => [
      { key: "username", title: "用户名", dataIndex: "username" as const },
      {
        key: "role",
        title: "角色",
        render: (row: ConsoleUser) => (
          <NerveTag tone={row.role === "admin" ? "blue" : "default"}>
            {ROLE_LABEL[row.role] ?? row.role}
          </NerveTag>
        ),
      },
      {
        key: "enabled",
        title: "状态",
        render: (row: ConsoleUser) => (
          <NerveTag tone={row.enabled ? "green" : "default"}>{row.enabled ? "启用" : "禁用"}</NerveTag>
        ),
      },
      {
        key: "display_name",
        title: "显示名",
        render: (row: ConsoleUser) => row.display_name || "—",
      },
      {
        key: "updated_at",
        title: "更新时间",
        render: (row: ConsoleUser) => formatDateTimeZh(row.updated_at),
      },
      {
        key: "actions",
        title: "操作",
        width: NERVE_COL_ACTIONS,
        fixed: "right" as const,
        render: (row: ConsoleUser) => (
          <NerveTableActions
            actions={[
              { key: "edit", label: "编辑", onClick: () => openEdit(row) },
              { key: "delete", label: "删除", danger: true, onClick: () => void handleDelete(row) },
            ]}
          />
        ),
      },
    ],
    [],
  );

  if (!admin) {
    return (
      <div>
        <NervePageHeader title="用户管理" subtitle="仅管理员可访问" />
        <NerveCard title="权限不足">
          <p className="muted">当前账号无管理员权限，无法管理 Console 用户。</p>
        </NerveCard>
      </div>
    );
  }

  return (
    <div>
      <NervePageHeader
        title="用户管理"
        subtitle="Console 本地账号与角色（admin / operator / viewer）"
        actions={
          <NerveButton variant="primary" onClick={openCreate}>
            添加用户
          </NerveButton>
        }
      />

      <NerveCard title="用户列表" tip={`共 ${users.length} 个`}>
        {loading ? (
          <p className="muted">加载中…</p>
        ) : (
          <NerveTable<ConsoleUser>
            rowKey="id"
            columns={columns}
            dataSource={users}
            emptyText="暂无用户，请添加"
          />
        )}
      </NerveCard>

      <NerveModal
        open={formMode != null}
        title={formMode === "create" ? "添加用户" : "编辑用户"}
        onClose={closeForm}
        footer={
          <>
            <NerveButton onClick={closeForm} disabled={saving}>
              取消
            </NerveButton>
            <NerveButton variant="primary" onClick={() => void handleSave()} disabled={saving}>
              {saving ? "保存中…" : "保存"}
            </NerveButton>
          </>
        }
      >
        <NerveForm>
          <NerveFormItem label="用户名" required>
            <NerveInput
              wide
              value={username}
              onChange={(e) => setUsername(e.target.value)}
              disabled={formMode === "edit" || saving}
              placeholder="登录用户名"
            />
          </NerveFormItem>
          <NerveFormItem label={formMode === "create" ? "密码" : "新密码"} required={formMode === "create"}>
            <NerveInput
              wide
              type="password"
              value={password}
              onChange={(e) => setPassword(e.target.value)}
              disabled={saving}
              placeholder={formMode === "edit" ? "留空则不修改" : "至少 4 位"}
            />
          </NerveFormItem>
          <NerveFormItem label="角色" required>
            <NerveSelect
              wide
              value={role}
              onChange={(e) => setRole(e.target.value)}
              disabled={saving}
              options={ROLE_OPTIONS}
            />
          </NerveFormItem>
          {formMode === "edit" ? (
            <NerveFormItem label="启用">
              <NerveSwitch
                checked={enabled}
                onChange={(e) => setEnabled(e.target.checked)}
                disabled={saving}
              />
            </NerveFormItem>
          ) : null}
          <NerveFormItem label="显示名">
            <NerveInput
              wide
              value={displayName}
              onChange={(e) => setDisplayName(e.target.value)}
              disabled={saving}
              placeholder="可选"
            />
          </NerveFormItem>
        </NerveForm>
      </NerveModal>
    </div>
  );
}
