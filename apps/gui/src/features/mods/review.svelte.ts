/**
 * An update or install waiting for the player's go-ahead: which mods, their
 * dates and sizes. The review dialog draws whatever is pending here.
 */
import type { InstalledModDto } from "$lib/ipc/types";
import { mods } from "$lib/stores/mods.svelte";

export type ReviewKind = "update_stale" | "update_all" | "update_selected" | "install";

export type ReviewMod = {
  id: number;
  name: string;
  size?: number;
  local_updated?: number;
  remote_updated?: number | null;
  installed: boolean;
};

class Review {
  pending = $state<{ kind: ReviewKind; mods: ReviewMod[] } | null>(null);

  #of(m: InstalledModDto): ReviewMod {
    return {
      id: m.id,
      name: m.name,
      size: m.size,
      local_updated: m.local_updated,
      remote_updated: m.remote_updated,
      installed: true,
    };
  }

  updateStale() {
    if (mods.stale.length)
      this.pending = { kind: "update_stale", mods: mods.stale.map((m) => this.#of(m)) };
  }
  updateAll() {
    if (mods.installed.length)
      this.pending = { kind: "update_all", mods: mods.installed.map((m) => this.#of(m)) };
  }
  updateSelected(ids: number[]) {
    const list = mods.installed.filter((m) => ids.includes(m.id));
    if (list.length) this.pending = { kind: "update_selected", mods: list.map((m) => this.#of(m)) };
  }
  install(ids: number[]) {
    if (!ids.length) return;
    this.pending = {
      kind: "install",
      mods: ids.map((id) => {
        const have = mods.byId.get(id);
        return have ? this.#of(have) : { id, name: `Workshop ${id}`, installed: false };
      }),
    };
  }

  confirm() {
    const p = this.pending;
    this.pending = null;
    if (!p) return;
    const ids = p.mods.map((m) => m.id);
    if (p.kind === "update_stale") mods.updateStale();
    else if (p.kind === "update_all") mods.updateAll();
    else if (p.kind === "update_selected") mods.updateMany(ids);
    else mods.install(ids);
  }
}

export const review = new Review();

/** One Workshop id out of a line: a bare number or a steamcommunity URL. */
export function parseWorkshopId(raw: string): number | null {
  const t = raw.trim();
  if (!t) return null;
  const url = /[?&]id=(\d+)/i.exec(t);
  const digits = url ? url[1]! : /^\d+$/.test(t) ? t : null;
  if (!digits) return null;
  const id = Number(digits);
  return id > 0 ? id : null;
}

/** Every distinct id in a block of text, one per line or comma-separated. */
export function parseWorkshopIds(text: string): number[] {
  const seen = new Set<number>();
  for (const part of text.split(/[\s,;]+/)) {
    const id = parseWorkshopId(part);
    if (id !== null) seen.add(id);
  }
  return [...seen];
}

export const workshopUrl = (id: number) =>
  `https://steamcommunity.com/sharedfiles/filedetails/?id=${id}`;
