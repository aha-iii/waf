/** Console page: generic placeholder for routes not yet wired. */
import { NervePageHeader } from "@nerve/ui";

type Props = {
  title: string;
  subtitle?: string;
};

export function PlaceholderPage({ title, subtitle = "页面迁移中…" }: Props): JSX.Element {
  return (
    <>
      <NervePageHeader title={title} subtitle={subtitle} />
      <p className="muted">该页面尚未完成 React 迁移，请稍后再试。</p>
    </>
  );
}
