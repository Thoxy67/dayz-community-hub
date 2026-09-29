/**
 * Which tab of the server panel is open. Module-level, so moving from one
 * server to the next keeps you on the same tab: comparing rules or player
 * lists across servers should not mean clicking the tab each time.
 */
export type DetailTab = "overview" | "players" | "mods" | "rules" | "stats";

const TABS: readonly DetailTab[] = ["overview", "players", "mods", "rules", "stats"];

/** `?tab=` picks the first tab in mock and dev builds, for screenshots. */
function initial(): DetailTab {
  if (!(import.meta.env.DEV || import.meta.env.VITE_MOCK === "1")) return "overview";
  const t = new URLSearchParams(location.search).get("tab") as DetailTab | null;
  return t && TABS.includes(t) ? t : "overview";
}

class TabMemory {
  current = $state<DetailTab>(initial());
}

export const detailTab = new TabMemory();
