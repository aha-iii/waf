/** Local / Keycloak auth helpers for WAF Console sessions. */
import Keycloak from "keycloak-js";

export type KeycloakConfig = {
  enabled: boolean;
  auth_server_url: string;
  realm: string;
  client_id: string;
};

export type AuthConfig = {
  enabled: boolean;
  mode: "local" | "keycloak" | "none";
  local?: { enabled: boolean };
  keycloak?: KeycloakConfig;
};

export type AuthPhase = "loading" | "login" | "ready";

const TOKEN_KEY = "nerve_waf_token";
const USER_KEY = "nerve_waf_user";
const ROLE_KEY = "nerve_waf_role";

let kc: Keycloak | null = null;
let token: string | null = null;
let username: string | null = null;
let role: string | null = null;
let authMode: AuthConfig["mode"] = "none";
let authRequired = false;
let authSkippedReason: string | null = null;
let onUnauthorized: (() => void) | null = null;

function authHeaders(): Record<string, string> {
  return token ? { Authorization: `Bearer ${token}` } : {};
}

function loadStoredSession(): boolean {
  const stored = sessionStorage.getItem(TOKEN_KEY);
  const user = sessionStorage.getItem(USER_KEY);
  const storedRole = sessionStorage.getItem(ROLE_KEY);
  if (!stored) return false;
  token = stored;
  username = user;
  role = storedRole;
  return true;
}

function saveSession(nextToken: string, nextUser: string, nextRole?: string) {
  token = nextToken;
  username = nextUser;
  role = nextRole ?? "admin";
  sessionStorage.setItem(TOKEN_KEY, nextToken);
  sessionStorage.setItem(USER_KEY, nextUser);
  sessionStorage.setItem(ROLE_KEY, role);
}

function clearSession() {
  token = null;
  username = null;
  role = null;
  sessionStorage.removeItem(TOKEN_KEY);
  sessionStorage.removeItem(USER_KEY);
  sessionStorage.removeItem(ROLE_KEY);
}

async function fetchJsonWithTimeout<T>(url: string, ms = 8_000): Promise<T> {
  const ctrl = new AbortController();
  const timer = window.setTimeout(() => ctrl.abort(), ms);
  try {
    const res = await fetch(url, { signal: ctrl.signal });
    if (!res.ok) throw new Error(`HTTP ${res.status}`);
    return (await res.json()) as T;
  } finally {
    window.clearTimeout(timer);
  }
}

function installCryptoPolyfills(): void {
  const cryptoObj = globalThis.crypto;
  if (!cryptoObj || typeof cryptoObj.randomUUID === "function") return;
  if (typeof cryptoObj.getRandomValues !== "function") return;

  cryptoObj.randomUUID = function randomUUID(): `${string}-${string}-${string}-${string}-${string}` {
    const bytes = new Uint8Array(16);
    cryptoObj.getRandomValues(bytes);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    const hex = [...bytes].map((b) => b.toString(16).padStart(2, "0")).join("");
    return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}` as `${string}-${string}-${string}-${string}-${string}`;
  };
}

function canUsePkce(): boolean {
  return typeof window !== "undefined" && window.isSecureContext && !!globalThis.crypto?.subtle;
}

export function getAuthHeaders(): Record<string, string> {
  return authHeaders();
}

export function getUsername(): string | null {
  return username;
}

export function getRole(): string | null {
  return role;
}

export function isAdmin(): boolean {
  return (role ?? "admin") === "admin";
}

export function getAuthMode(): AuthConfig["mode"] {
  return authMode;
}

export function isAuthRequired(): boolean {
  return authRequired;
}

export function hasAuthSession(): boolean {
  return !!token || !!kc?.token;
}

export function setUnauthorizedHandler(fn: (() => void) | null) {
  onUnauthorized = fn;
}

export function notifyUnauthorized() {
  clearSession();
  onUnauthorized?.();
}

export function getAuthSkippedReason(): string | null {
  return authSkippedReason;
}

export async function logout(): Promise<void> {
  if (kc) {
    await kc.logout({ redirectUri: window.location.origin });
    return;
  }
  clearSession();
  window.location.reload();
}

let initAuthPromise: Promise<AuthPhase> | null = null;

export function initAuth(): Promise<AuthPhase> {
  if (initAuthPromise) return initAuthPromise;
  initAuthPromise = doInitAuth().catch((e) => {
    initAuthPromise = null;
    throw e;
  });
  return initAuthPromise;
}

async function fetchAuthStatus(): Promise<{ enabled?: boolean; mode?: string; local?: { enabled?: boolean } } | null> {
  try {
    const res = await fetch("/api/auth/status");
    if (!res.ok) return null;
    return (await res.json()) as { enabled?: boolean; mode?: string; local?: { enabled?: boolean } };
  } catch {
    return null;
  }
}

async function resolveAuthPhase(cfg: AuthConfig | null): Promise<AuthPhase> {
  if (cfg) {
    authMode = cfg.mode ?? "none";
    authRequired = !!cfg.enabled && authMode !== "none";
  } else {
    const st = await fetchAuthStatus();
    authMode = (st?.mode as AuthConfig["mode"]) ?? "none";
    authRequired = !!st?.enabled && authMode !== "none";
    if (st) {
      cfg = {
        enabled: !!st.enabled,
        mode: authMode,
        local: { enabled: st.local?.enabled !== false },
      };
    }
  }

  if (!cfg?.enabled || authMode === "none") {
    authRequired = false;
    return "ready";
  }

  if (authMode === "local" && cfg.local?.enabled !== false) {
    if (loadStoredSession()) return "ready";
    return "login";
  }

  if (authMode === "local") {
    if (loadStoredSession()) return "ready";
    return "login";
  }

  if (authMode === "keycloak" && cfg.keycloak?.enabled) {
    await initKeycloak(cfg.keycloak);
    return "ready";
  }

  if (hasAuthSession()) return "ready";
  return "login";
}

async function doInitAuth(): Promise<AuthPhase> {
  authSkippedReason = null;

  let cfg: AuthConfig | null = null;
  try {
    cfg = await fetchJsonWithTimeout<AuthConfig>("/api/auth/config");
  } catch (e) {
    console.warn("auth config fetch failed", e);
    authSkippedReason = "无法读取 /api/auth/config，尝试 /api/auth/status";
  }

  try {
    return await resolveAuthPhase(cfg);
  } catch (e) {
    console.warn("initAuth failed", e);
    authSkippedReason = "认证初始化失败";
    if (authRequired || (await fetchAuthStatus())?.enabled) {
      return "login";
    }
    return "ready";
  }
}

async function initKeycloak(kcCfg: KeycloakConfig): Promise<void> {
  installCryptoPolyfills();
  const usePkce = canUsePkce();
  const insecure = !window.isSecureContext;

  kc = new Keycloak({
    url: kcCfg.auth_server_url,
    realm: kcCfg.realm,
    clientId: kcCfg.client_id,
  });

  try {
    const ok = await kc.init({
      onLoad: "login-required",
      pkceMethod: usePkce ? "S256" : false,
      checkLoginIframe: false,
    });
    if (!ok) throw new Error("Keycloak 登录失败");
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    if (insecure) {
      authSkippedReason =
        "HTTP 局域网访问无法完成 OIDC 登录，已跳过 Keycloak。建议改用 https:// 或自研登录（NERVE_AUTH_MODE=local）";
      console.warn(authSkippedReason, msg);
      kc = null;
      return;
    }
    throw e instanceof Error ? e : new Error(msg);
  }

  token = kc.token ?? null;
  username =
    (kc.tokenParsed?.preferred_username as string | undefined) ??
    (kc.tokenParsed?.name as string | undefined) ??
    null;

  setInterval(async () => {
    if (!kc) return;
    try {
      await kc.updateToken(30);
      token = kc.token ?? null;
    } catch {
      void kc.login();
    }
  }, 10_000);
}

export async function loginLocal(user: string, pass: string): Promise<void> {
  const res = await fetch("/api/auth/login", {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ username: user, password: pass }),
  });
  const text = await res.text();
  let data: { token?: string; username?: string; role?: string; error?: string } = {};
  try {
    data = text ? JSON.parse(text) : {};
  } catch {
    /* ignore */
  }
  if (!res.ok) {
    throw new Error(data.error || "用户名或密码错误");
  }
  if (!data.token) {
    throw new Error("登录响应无效");
  }
  saveSession(data.token, data.username || user, data.role);
}

/** @deprecated use initAuth */
export async function ensureAuth(): Promise<void> {
  const phase = await initAuth();
  if (phase === "login") {
    throw new Error("需要登录");
  }
}
