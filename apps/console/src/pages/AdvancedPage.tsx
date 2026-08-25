/** Console page: advanced security placeholders (capability not enabled). */
import { NervePageHeader } from "@nerve/ui";

/** Advanced security: capability not enabled; placeholder only */
export function AdvancedPage(): JSX.Element {
  return (
    <>
      <NervePageHeader title="高级安全" />
      <p className="muted" style={{ padding: "48px 0", textAlign: "center", fontSize: 16 }}>
        敬请期待
      </p>
    </>
  );
}
