import { Component, type ErrorInfo, type ReactNode } from "react";
import { NerveAlert, NerveButton } from "@nerve/ui";

type Props = { children: ReactNode; page?: string };
type State = { error: Error | null };

export class ConsoleErrorBoundary extends Component<Props, State> {
  state: State = { error: null };

  static getDerivedStateFromError(error: Error): State {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    console.error("Console page error", error, info.componentStack);
  }

  render() {
    if (this.state.error) {
      return (
        <div style={{ padding: 24, maxWidth: 720 }}>
          <NerveAlert tone="error">页面暂时无法显示，请刷新后重试。</NerveAlert>
          <p className="muted small" style={{ marginTop: 12 }}>
            请刷新页面重试；问题持续存在请联系管理员。
          </p>
          <NerveButton style={{ marginTop: 16 }} onClick={() => window.location.reload()}>
            刷新页面
          </NerveButton>
        </div>
      );
    }
    return this.props.children;
  }
}
