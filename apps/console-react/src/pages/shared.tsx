/** Shared helpers and widgets used across Console pages. */
import { NerveTag, type NerveTagTone } from "@nerve/ui";
import { formatTs, formatTsEvent } from "../lib/datetime";
import { categoryLabel as catLabel } from "../lib/labels";

export { catLabel as categoryLabel };
export { formatTs, formatTsEvent };

export function ActionTag({ action }: { action: string }) {
  const tone: NerveTagTone =
    action === "block" ? "red" : action === "log" ? "orange" : "green";
  const label = action === "block" ? "拦截" : action === "log" ? "记录" : action;
  return <NerveTag tone={tone}>{label}</NerveTag>;
}

export function Ellipsis({
  text,
  className,
}: {
  text: string;
  className?: string;
}) {
  const t = text.trim() || "—";
  return (
    <span className={className ?? "ev-ellipsis"} title={t === "—" ? undefined : t}>
      {t}
    </span>
  );
}
