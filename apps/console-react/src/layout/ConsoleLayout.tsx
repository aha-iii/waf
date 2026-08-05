import type { ReactNode } from "react";
import { useMemo } from "react";
import { NerveSider, buildSiderMenuGroups } from "@nerve/ui";
import { NAV, PAGE_META, type PageId } from "../nav";
import { getUsername, getRole, isAdmin, logout } from "../auth";

type Props = {
  page: PageId;
  onNavigate: (id: PageId) => void;
  healthText?: string;
  children: ReactNode;
};

export function ConsoleLayout({ page, onNavigate, healthText, children }: Props) {
  const meta = PAGE_META[page];
  const siderMenu = useMemo(
    () => buildSiderMenuGroups(NAV.filter((item) => item.id !== "users" || isAdmin())),
    [],
  );

  return (
    <div className="console-layout">
      <NerveSider
        title="Nerve WAF"
        menu={siderMenu}
        activeMenuId={page}
        onMenuSelect={(id) => onNavigate(id as PageId)}
      />

      <div className="console-main">
        <header className="console-topbar">
          <div className="topbar-left">
            <div className="topbar-title">{meta?.title ?? page}</div>
            {meta?.subtitle ? <div className="topbar-sub muted small">{meta.subtitle}</div> : null}
          </div>
          <div className="topbar-right">
            {healthText ? <span className="muted small">{healthText}</span> : null}
            <span className="muted small">
              {getUsername() || "console"}
              {getRole() ? ` · ${getRole()}` : ""}
            </span>
            <button type="button" className="ant-btn ant-btn-sm" onClick={() => void logout()}>
              退出
            </button>
          </div>
        </header>
        <main className="console-content">{children}</main>
      </div>
    </div>
  );
}
