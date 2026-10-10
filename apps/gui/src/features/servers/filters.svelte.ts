/**
 * What the server browser is asking for: the search, the filters, the order.
 * The backend does the filtering and sorting (`servers_query`); this only
 * holds the choices, kept at module level so they survive leaving the view,
 * and in the session (`lib/stores/session`) so they survive a reload.
 */
import type { ServerQuery, SortCol, Tri } from "$lib/ipc/servers";
import { session } from "$lib/stores/session.svelte";

export type { SortCol, Tri };

export const TRI_NEXT: Record<Tri, Tri> = { all: "only", only: "none", none: "all" };

class ServerFilters {
  search = $state("");
  /** The search as it was 200 ms after the last keystroke. */
  query = $state("");
  map = $state<string | null>(null);
  firstPerson = $state<Tri>("all");
  password = $state<Tri>("all");
  battleye = $state<Tri>("all");
  modded = $state<Tri>("all");
  official = $state<Tri>("all");
  hideEmpty = $state(false);
  hideFull = $state(false);
  /** Hide servers slower than this (ms); 0 for any. */
  maxPing = $state(0);
  showExcluded = $state(false);
  sort = $state<SortCol>("none");
  asc = $state(true);

  #timer: ReturnType<typeof setTimeout> | undefined;
  setSearch(v: string) {
    this.search = v;
    clearTimeout(this.#timer);
    this.#timer = setTimeout(() => (this.query = v.trim()), 200);
  }

  get active(): boolean {
    return (
      this.query !== "" ||
      this.map !== null ||
      this.firstPerson !== "all" ||
      this.password !== "all" ||
      this.battleye !== "all" ||
      this.modded !== "all" ||
      this.official !== "all" ||
      this.hideEmpty ||
      this.hideFull ||
      this.maxPing > 0
    );
  }

  clear() {
    this.setSearch("");
    this.query = "";
    this.map = null;
    this.firstPerson = this.password = this.battleye = this.modded = this.official = "all";
    this.hideEmpty = this.hideFull = false;
    this.maxPing = 0;
  }

  toggleSort(col: Exclude<SortCol, "none">) {
    if (this.sort === col) this.asc = !this.asc;
    else {
      this.sort = col;
      // Names read A→Z; figures read biggest (or, for ping, smallest) first.
      this.asc = col === "name" || col === "map" || col === "ping";
    }
  }

  /** The request, without the window. Reading it tracks every choice. */
  get request(): Omit<ServerQuery, "offset" | "limit"> {
    return {
      search: this.query,
      map: this.map,
      firstPerson: this.firstPerson,
      password: this.password,
      battleye: this.battleye,
      modded: this.modded,
      official: this.official,
      hideEmpty: this.hideEmpty,
      hideFull: this.hideFull,
      maxPing: this.maxPing,
      showExcluded: this.showExcluded,
      sort: this.sort,
      asc: this.asc,
    };
  }
}

export const filters = new ServerFilters();

type Saved = Omit<ServerQuery, "offset" | "limit"> & { text: string };

session.keep<Saved>(
  "servers.filters",
  // The search box as typed, beside the request (whose search is debounced).
  () => ({ ...filters.request, text: filters.search }),
  (v) => {
    filters.search = v.text ?? v.search;
    filters.query = v.search;
    filters.map = v.map;
    filters.firstPerson = v.firstPerson;
    filters.password = v.password;
    filters.battleye = v.battleye;
    filters.modded = v.modded;
    filters.official = v.official;
    filters.hideEmpty = v.hideEmpty;
    filters.hideFull = v.hideFull;
    filters.maxPing = v.maxPing;
    filters.showExcluded = v.showExcluded;
    filters.sort = v.sort;
    filters.asc = v.asc;
  },
);
