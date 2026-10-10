/**
 * The account's Steam friends, kept fresh while they matter: who plays
 * DayZ and where, for the Friends view, the rail's count and the server
 * list's "friends here" marker.
 *
 * Asked on a cadence, never while the window is in the background:
 * every 20 s while the Friends view shows and a friend is in DayZ, every
 * 60 s while it shows otherwise, every 2 min behind other views. Behind
 * other views only through the Web API: without a key the Steam client is
 * asked, which shows DayZ running for a moment, so that path is only taken
 * while the view is open (and every minute). The backend shares one answer
 * between asks closer than 10 s and keeps the friend list itself 10 min
 * (`features/friends`).
 */
import { SvelteMap } from "svelte/reactivity";
import { steamFriends } from "$lib/ipc/friends";
import { errorText } from "$lib/ipc/core";
import type { FriendDto, FriendsDto } from "$lib/ipc/types";
import { toasts } from "$lib/components/ui/toast";
import { words } from "$lib/i18n";
import { app } from "./app.svelte";
import { profile } from "./profile.svelte";
import { connect } from "./connect.svelte";

const SHOWN_DAYZ_MS = 20_000;
const SHOWN_MS = 60_000;
const BEHIND_MS = 120_000;
/** More arrivals than this at once are said in one toast. */
const TOASTS_MAX = 2;

/** The key a friend's server goes by: the query port when the list has it. */
const serverKey = (s: { ip: string; port: number }) => `${s.ip}:${s.port}`;

class Friends {
  data = $state.raw<FriendsDto | null>(null);
  loading = $state(false);
  failed = $state<string | null>(null);
  /** When the last answer arrived (ms), 0 before the first. */
  updatedAt = $state(0);
  /** When the last ask ended, answered or not: the cadence counts from it. */
  #askedAt = $state(0);
  /** A background ask found no working Web API: no more until the view is opened. */
  #behindFailed = false;
  /** When a friend was seen arriving in DayZ, by Steam id (ms): only arrivals this session saw. */
  since = new SvelteMap<string, number>();

  list = $derived(this.data?.friends ?? []);
  inDayz = $derived(this.list.filter((f) => f.in_dayz).length);
  online = $derived(this.list.filter((f) => f.status !== "offline" || f.game).length);
  /** Friends on each server, by "ip:port" (the query port when the server is listed). */
  byServer = $derived.by(() => {
    const m = new Map<string, FriendDto[]>();
    for (const f of this.list) {
      if (!f.in_dayz || !f.server) continue;
      const k = serverKey(f.server);
      const on = m.get(k);
      if (on) on.push(f);
      else m.set(k, [f]);
    }
    return m;
  });

  /** The friends on a server, by its query port (or game port when unlisted). */
  on(ip: string, port: number): FriendDto[] | undefined {
    return this.byServer.get(`${ip}:${port}`);
  }

  /** How long until the next ask, or null for none: see the header. */
  #interval(): number | null {
    if (!app.focused) return null;
    const shown = app.view === "friends";
    const web = !!profile.data?.steam_api_key && !!profile.data?.steam_id;
    if (shown) {
      if (this.data?.source === "steam_client") return SHOWN_MS;
      return this.inDayz > 0 ? SHOWN_DAYZ_MS : SHOWN_MS;
    }
    // Behind other views the Steam client is never asked (it shows DayZ
    // running for a moment): the Web API only, and only once it has worked.
    if (!web || this.#behindFailed) return null;
    return !this.data || this.data.source === "web_api" ? BEHIND_MS : null;
  }

  constructor() {
    $effect.root(() => {
      $effect(() => {
        const every = this.#interval();
        if (every === null) return;
        const due = Math.max(0, this.#askedAt + every - Date.now());
        const t = setTimeout(() => void this.load(), due);
        return () => clearTimeout(t);
      });
    });
  }

  async load() {
    if (this.loading) return;
    this.loading = true;
    const shown = app.view === "friends";
    try {
      const next = await steamFriends(shown);
      if (shown) this.#behindFailed = false;
      else if (!next.source) {
        // Not set up, or Steam refused: what the view showed stays, and
        // the view's own ask says why.
        this.#behindFailed = true;
        return;
      }
      if (this.data?.source && next.source) this.#arrivals(this.data, next);
      this.data = next;
      this.failed = null;
      this.updatedAt = Date.now();
    } catch (e) {
      this.failed = errorText(e);
    } finally {
      this.loading = false;
      this.#askedAt = Date.now();
    }
  }

  /** Friends who started DayZ or moved to another server since the last answer: a toast. */
  #arrivals(before: FriendsDto, after: FriendsDto) {
    const was = new Map(before.friends.map((f) => [f.steam_id, f]));
    const now = Date.now();
    const came: FriendDto[] = [];
    for (const f of after.friends) {
      const p = was.get(f.steam_id);
      if (!f.in_dayz) {
        if (this.since.has(f.steam_id)) this.since.delete(f.steam_id);
        continue;
      }
      if (!p) continue;
      const started = !p.in_dayz;
      const moved =
        !!f.server && (!p.server || serverKey(p.server) !== serverKey(f.server)) && !started;
      if (started) this.since.set(f.steam_id, now);
      if (started || moved) came.push(f);
    }
    if (came.length === 0) return;
    const w = words("friends");
    if (came.length > TOASTS_MAX) {
      toasts.say(String(w.toastMany({ count: came.length })), "neutral", {
        label: String(w.toastShow),
        run: () => app.go("friends"),
      });
      return;
    }
    for (const f of came) {
      const where = f.server?.name ?? (f.server ? `${f.server.ip}:${f.server.port}` : null);
      const text = where
        ? String(w.toastJoined({ name: f.name, server: where }))
        : String(w.toastStarted({ name: f.name }));
      const s = f.server;
      toasts.say(
        text,
        "neutral",
        s
          ? { label: String(w.toastJoin), run: () => void connect.address(s.ip, s.port) }
          : undefined,
      );
    }
  }
}

export const friends = new Friends();
