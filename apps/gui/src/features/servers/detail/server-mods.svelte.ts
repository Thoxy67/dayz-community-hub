/**
 * The mods a listed server runs, fetched once per server and shared by every
 * panel that shows it. The list endpoint carries Workshop ids; servers not in
 * the list only name their mods over A2S, which the panel reads instead.
 */
import { SvelteMap } from "svelte/reactivity";
import { getServerDetails } from "$lib/ipc/servers";
import { errorText } from "$lib/ipc/core";
import type { ModDto } from "$lib/ipc/types";

type Entry = { mods: ModDto[] | null; loading: boolean; error: string | null };

const cache = new SvelteMap<string, Entry>();
const flights = new Map<string, Promise<void>>();

export function serverMods(ip: string, queryPort: number): Entry {
  return cache.get(`${ip}:${queryPort}`) ?? { mods: null, loading: false, error: null };
}

export function loadServerMods(ip: string, queryPort: number, force = false): Promise<void> {
  const key = `${ip}:${queryPort}`;
  if (!force && cache.get(key)?.mods) return Promise.resolve();
  const inflight = flights.get(key);
  if (inflight) return inflight;
  cache.set(key, { mods: cache.get(key)?.mods ?? null, loading: true, error: null });
  const p = getServerDetails(ip, queryPort)
    .then((full) => void cache.set(key, { mods: full.mods, loading: false, error: null }))
    .catch((e) => void cache.set(key, { mods: null, loading: false, error: errorText(e) }))
    .finally(() => flights.delete(key));
  flights.set(key, p);
  return p;
}
