/** React hook: Use Hash Page. */
import { useCallback, useEffect, useState } from "react";
import type { PageId } from "../nav";
import { LEGACY_PAGE_ALIASES, NAV, PAGE_META } from "../nav";

function parseHash(): PageId {
  const raw = (location.hash.replace(/^#\/?/, "") || "dashboard").split("?")[0];
  const resolved = LEGACY_PAGE_ALIASES[raw] ?? raw;
  if (resolved && resolved in PAGE_META) return resolved as PageId;
  if (NAV.some((n) => n.id === resolved)) return resolved as PageId;
  return "dashboard";
}

export function useHashPage(): [PageId, (id: PageId) => void] {
  const [page, setPage] = useState<PageId>(() => parseHash());

  useEffect(() => {
    const onHash = () => setPage(parseHash());
    window.addEventListener("hashchange", onHash);
    return () => window.removeEventListener("hashchange", onHash);
  }, []);

  const navigate = useCallback((id: PageId) => {
    const next = `#/${id}`;
    if (location.hash !== next) location.hash = next;
    else setPage(id);
  }, []);

  return [page, navigate];
}
