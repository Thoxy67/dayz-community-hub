/**
 * What the panel knows about one server, gathered once from the stores so the
 * tabs read the same values: the listed row (if the public list has it), the
 * live A2S answer, the mods it runs against what is installed, DayZ Metrics'
 * long view.
 * Nothing here computes over more than this one server.
 */
import { servers, type ServerRow } from "$lib/stores/servers.svelte";
import { serverData } from "$lib/stores/server-data.svelte";
import { mods } from "$lib/stores/mods.svelte";
import { profile } from "$lib/stores/profile.svelte";
import { serverMods } from "./server-mods.svelte";

export type ModState = "missing" | "stale" | "ok";

/** `src` is read reactively: the panel follows its props from one server to the next. */
export function detailModel(src: () => { ip: string; port: number; name: string }) {
  const ip = $derived(src().ip);
  const port = $derived(src().port);
  const name = $derived(src().name);
  const listed = $derived<ServerRow | undefined>(servers.find(ip, port));
  const live = $derived(serverData.a2s(ip, port));
  const a2s = $derived(live.data);
  const queryPort = $derived(listed?.query_port ?? a2s?.query_port ?? port);
  const gamePort = $derived(listed?.game_port ?? a2s?.game_port ?? port);
  const title = $derived(listed?.name || a2s?.server_name || name || `${ip}:${port}`);
  const map = $derived(listed?.map || a2s?.map || "");
  const count = $derived(
    listed
      ? servers.count(listed)
      : a2s
        ? { players: a2s.players, max: a2s.max_players, bots: a2s.bots }
        : null,
  );
  const modsEntry = $derived(listed ? serverMods(listed.ip, listed.query_port) : null);
  // A listed server's mods come from the list; an unlisted one's from its own
  // A2S answer, which carries Workshop ids too.
  const modRows = $derived(
    ((listed ? modsEntry?.mods : a2s?.mods_a2s) ?? []).map((m) => {
      const have = mods.byId.get(m.steam_workshop_id);
      const state: ModState = !have ? "missing" : have.update_available ? "stale" : "ok";
      return {
        id: m.steam_workshop_id,
        name: m.name || have?.name || `Workshop ${m.steam_workshop_id}`,
        state,
      };
    }),
  );
  const modTotals = $derived({
    missing: modRows.filter((m) => m.state === "missing"),
    stale: modRows.filter((m) => m.state === "stale"),
    installed: modRows.filter((m) => m.state !== "missing").length,
  });
  const modsCount = $derived(
    modsEntry?.mods?.length ?? listed?.mods_count ?? a2s?.mods_a2s?.length ?? 0,
  );
  const metricsEntry = $derived(serverData.metrics(ip, gamePort, queryPort));
  const metrics = $derived(metricsEntry.data);
  // "fake" when the site says so outright or its behaviour check does; "suspect"
  // for the softer signs (a suspicious curve, player reports, a name that
  // passes for official).
  const population = $derived.by((): "fake" | "suspect" | "ok" | null => {
    if (!metrics) return null;
    const v = metrics.behavior_verdict?.toLowerCase() ?? "";
    if (metrics.is_fake || v === "fake") return "fake";
    if (v.startsWith("susp") || metrics.flagged || metrics.mimics_official) return "suspect";
    return "ok";
  });
  const country = $derived(metrics?.country ?? null);
  const players = $derived(
    [...(a2s?.players_list ?? [])].sort((a, b) => (b.duration ?? 0) - (a.duration ?? 0)),
  );
  const history = $derived(
    (profile.data?.history ?? []).filter(
      (h) => h.ip === ip && (h.port === queryPort || h.port === gamePort),
    ),
  );

  return {
    get ip() {
      return ip;
    },
    get listed() {
      return listed;
    },
    get live() {
      return live;
    },
    get a2s() {
      return a2s;
    },
    get queryPort() {
      return queryPort;
    },
    get gamePort() {
      return gamePort;
    },
    get title() {
      return title;
    },
    get map() {
      return map;
    },
    get count() {
      return count;
    },
    get modsEntry() {
      return modsEntry;
    },
    get modRows() {
      return modRows;
    },
    get modTotals() {
      return modTotals;
    },
    get modsCount() {
      return modsCount;
    },
    get metricsEntry() {
      return metricsEntry;
    },
    get metrics() {
      return metrics;
    },
    get population() {
      return population;
    },
    get country() {
      return country;
    },
    get players() {
      return players;
    },
    get history() {
      return history;
    },
    get address() {
      return `${ip}:${gamePort}`;
    },
  };
}

export type DetailModel = ReturnType<typeof detailModel>;
