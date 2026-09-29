/**
 * What the server browser is asking for: the search, the filters, the order.
 * The backend does the filtering and sorting (`servers_query`); this only
 * holds the choices, kept at module level so they survive leaving the view.
 */
import type { ServerQuery, SortCol, Tri } from "$lib/ipc/servers";

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
