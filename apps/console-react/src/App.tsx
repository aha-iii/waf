/** Root React app: routing shell and auth gate for WAF Console. */
import { useEffect, useState } from "react";
import { ToastProvider } from "@nerve/ui";
import { apiGet } from "./api";
import { initAuth, setUnauthorizedHandler, type AuthPhase } from "./auth";
import { ConsoleErrorBoundary } from "./components/ConsoleErrorBoundary";
import { LoginPage } from "./pages/LoginPage";
import { useHashPage } from "./hooks/useHashPage";
import { ConsoleLayout } from "./layout/ConsoleLayout";
import type { PageId } from "./nav";
import { PAGE_META } from "./nav";
import { AccessPage } from "./pages/AccessPage";
import { AdvancedPage } from "./pages/AdvancedPage";
import { AiTriageHubPage } from "./pages/AiTriageHubPage";
import { AiPolicyLearningPage } from "./pages/AiPolicyLearningPage";
import { AiSemanticPage } from "./pages/AiSemanticPage";
import { PosturePage } from "./pages/PosturePage";
import { AlertsPage } from "./pages/AlertsPage";
import { AnalyticsPage } from "./pages/AnalyticsPage";
import { AssetsPage } from "./pages/AssetsPage";
import { BotPage } from "./pages/BotPage";
import { CcPage } from "./pages/CcPage";
import { DashboardPage } from "./pages/DashboardPage";
import { EventsPage } from "./pages/EventsPage";
import { GeoPage } from "./pages/GeoPage";
import { InfoLeakPage } from "./pages/InfoLeakPage";
import { IntelPage } from "./pages/IntelPage";
import { GlobalModulesPage } from "./features/modules/GlobalModulesPage";
import { PlaceholderPage } from "./pages/PlaceholderPage";
import { ProtectionLevelsPage } from "./pages/ProtectionLevelsPage";
import { RulesAiPage } from "./pages/RulesAiPage";
import { RulesCrsPage } from "./pages/RulesCrsPage";
import { RulesCustomPage } from "./pages/RulesCustomPage";
import { RulesLuaPage } from "./pages/RulesLuaPage";
import { TrafficDashboardPage } from "./pages/TrafficDashboardPage";
import { UsersPage } from "./pages/UsersPage";
import { UpstreamsPage } from "./pages/UpstreamsPage";

function DashboardPageWrapper() {
  const [, navigate] = useHashPage();
  return <DashboardPage onNavigate={navigate} />;
}

function renderPage(page: PageId) {
  switch (page) {
    case "dashboard":
      return <DashboardPageWrapper />;
    case "assets":
      return <AssetsPage />;
    case "origin-upstreams":
      return <UpstreamsPage />;
    case "events":
      return <EventsPage />;
    case "basic-protection":
      return <GlobalModulesPage pageId="basic-protection" />;
    case "core-protection":
    case "modules":
      return <GlobalModulesPage pageId="core-protection" />;
    case "protection-levels":
      return <ProtectionLevelsPage />;
    case "access":
      return <AccessPage />;
    case "cc":
      return <CcPage />;
    case "geo":
      return <GeoPage />;
    case "bot":
      return <BotPage />;
    case "info-leak":
      return <InfoLeakPage />;
    case "intel":
      return <IntelPage />;
    case "alerts":
      return <AlertsPage />;
    case "analytics":
      return <AnalyticsPage />;
    case "traffic-dashboard":
      return <TrafficDashboardPage />;
    case "users":
      return <UsersPage />;
    case "advanced":
      return <AdvancedPage />;
    case "ai-semantic":
      return <AiSemanticPage />;
    case "ai-triage":
      return <AiTriageHubPage />;
    case "ai-posture":
      return <PosturePage />;
    case "ai-policy-learning":
      return <AiPolicyLearningPage />;
    case "rules-crs":
      return <RulesCrsPage />;
    case "rules-ai":
      return <RulesAiPage />;
    case "rules-custom":
      return <RulesCustomPage />;
    case "rules-plugins-lua":
      return <RulesLuaPage />;
    case "rules-plugins-json":
      return <RulesCustomPage />;
    default: {
      const meta = PAGE_META[page as PageId];
      return (
        <PlaceholderPage title={meta?.title ?? String(page)} subtitle={meta?.subtitle} />
      );
    }
  }
}

export default function App() {
  const [page, navigate] = useHashPage();
  const [authPhase, setAuthPhase] = useState<AuthPhase>("loading");
  const [healthText, setHealthText] = useState<string | undefined>();

  useEffect(() => {
    setUnauthorizedHandler(() => setAuthPhase("login"));
    return () => setUnauthorizedHandler(null);
  }, []);

  useEffect(() => {
    let cancelled = false;
    void initAuth()
      .then((phase) => {
        if (!cancelled) setAuthPhase(phase);
      })
      .catch((e) => {
        console.warn("initAuth failed", e);
        if (!cancelled) setAuthPhase("login");
      });
    return () => {
      cancelled = true;
    };
  }, []);

  useEffect(() => {
    if (authPhase !== "ready") return;
    void apiGet<{ ok: boolean }>("/api/health")
      .then((h) => setHealthText(h.ok ? "正常" : "异常"))
      .catch(() => setHealthText(undefined));
  }, [authPhase]);

  if (authPhase === "loading") {
    return (
      <div
        style={{
          padding: 48,
          minHeight: "100vh",
          background: "#f0f2f5",
          color: "rgba(0,0,0,0.85)",
        }}
      >
        加载中…
      </div>
    );
  }

  if (authPhase === "login") {
    return <LoginPage onSuccess={() => setAuthPhase("ready")} />;
  }

  return (
    <ToastProvider>
      <ConsoleLayout page={page} onNavigate={navigate} healthText={healthText}>
        <ConsoleErrorBoundary page={page}>{renderPage(page)}</ConsoleErrorBoundary>
      </ConsoleLayout>
    </ToastProvider>
  );
}
