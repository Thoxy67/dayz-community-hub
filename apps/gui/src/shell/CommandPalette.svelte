<script lang="ts">
  import type { Component } from "svelte";
  import { Dialog } from "bits-ui";
  import { dict } from "$lib/i18n";
  import Search from "~icons/lucide/search";
  import Star from "~icons/lucide/star";
  import Server from "~icons/lucide/server";
  import History from "~icons/lucide/history";
  import Puzzle from "~icons/lucide/puzzle";
  import RefreshCw from "~icons/lucide/refresh-cw";
  import RotateCcw from "~icons/lucide/rotate-ccw";
  import Download from "~icons/lucide/download";
  import CloudCheck from "~icons/lucide/cloud-check";
  import Keyboard from "~icons/lucide/keyboard";
  import Settings from "~icons/lucide/settings";
  import Play from "~icons/lucide/play";
  import PlugZap from "~icons/lucide/plug-zap";
  import { Kbd } from "$lib/components/ui/kbd";
  import { Spinner } from "$lib/components/ui/spinner";
  import { cn } from "$lib/cx";
  import { serversQuery, type ServerRow } from "$lib/ipc/servers";
  import { runShortcut } from "$lib/shortcuts/actions";
  import { addressOf, fold, match, pieces } from "$lib/shortcuts/fuzzy";
  import { keyText, viewKey } from "$lib/shortcuts/registry";
  import { app } from "$lib/stores/app.svelte";
  import { connect } from "$lib/stores/connect.svelte";
  import { dialogs } from "$lib/stores/dialogs.svelte";
  import { mods } from "$lib/stores/mods.svelte";
  import { profile } from "$lib/stores/profile.svelte";
  import { PLACES } from "./nav";

  /**
   * Ctrl+K (Menu on a controller): one field for everything the window holds.
   *
   * Empty, it offers what was picked last, the actions and the views. Typed
   * into, it matches views, actions (the shortcut registry's, with their
   * keys), settings sections, favourites, history and installed mods here,
   * fuzzily, and asks the backend for servers (the browser's own query,
   * 120 ms after typing pauses, six at most); an address ("1.2.3.4:2302")
   * offers to connect to it. Sections are ordered by their best match;
   * ↑↓ move, Tab jumps a section, Enter does the obvious thing, Esc closes.
   * A controller speaks to it in keys (`data-pad-keys`, lib/gamepad/nav.ts).
   */
  const k = dict("keys");
  const n = dict("nav");
  const st = dict("settings");
  const p = dict("pad");

  type Item = {
    key: string;
    icon: Component<{ class?: string }>;
    tone?: string;
    label: string;
    /** Matched against the query beside the label, never shown. */
    also?: string;
    sub?: string;
    verb: string;
    kbd?: string;
    run: () => void;
    /** For the recent list: a server to rebuild. */
    server?: { ip: string; port: number };
  };
  type Shown = Item & { ranges: [number, number][]; score: number };
  type Group = { id: string; title: string; items: Shown[] };

  const CAP = 5;

  let query = $state("");
  let active = $state(0);
  let found = $state.raw<ServerRow[]>([]);
  let asking = $state(false);
  let list: HTMLDivElement | undefined = $state();
  let field: HTMLInputElement | undefined = $state();

  const q = $derived(fold(query.trim()));
  const address = $derived(addressOf(query));

  // ── recent picks, kept across launches ───────────────────────────────
  type Recent = { key: string; label: string; sub?: string; ip?: string; port?: number };
  const RECENT_KEY = "dzch.palette.recent";
  function loadRecent(): Recent[] {
    try {
      const v = JSON.parse(localStorage.getItem(RECENT_KEY) ?? "[]");
      return Array.isArray(v) ? v.slice(0, 8) : [];
    } catch {
      return [];
    }
  }
  let recent = $state.raw<Recent[]>(loadRecent());
  function remember(item: Item) {
    const r: Recent = {
      key: item.key,
      label: item.label,
      sub: item.sub,
      ip: item.server?.ip,
      port: item.server?.port,
    };
    recent = [r, ...recent.filter((x) => x.key !== r.key)].slice(0, 8);
    try {
      localStorage.setItem(RECENT_KEY, JSON.stringify(recent));
    } catch {
      // Remembered for this session.
    }
  }

  /** What the palette opens typed into (the mock's `?palette=`), once. */
  let seed = "";
  // A fresh palette each time it opens.
  $effect(() => {
    if (dialogs.palette) {
      query = seed;
      seed = "";
      active = 0;
      found = [];
    }
  });

  // Screenshot aid, in mock builds only: `?palette=<query>` opens it typed into.
  if (import.meta.env.VITE_MOCK === "1") {
    const want = new URLSearchParams(location.search).get("palette");
    if (want !== null)
      void (async () => {
        while (!app.initialized) await new Promise((r) => setTimeout(r, 100));
        await new Promise((r) => setTimeout(r, 800));
        seed = want;
        dialogs.palette = true;
      })();
  }

  // Servers: asked once typing pauses, the most populated first.
  $effect(() => {
    const term = query.trim();
    if (!dialogs.palette || term.length < 2 || address) {
      found = [];
      asking = false;
      return;
    }
    let stale = false;
    asking = true;
    const t = setTimeout(async () => {
      try {
        const page = await serversQuery({
          search: term,
          map: null,
          firstPerson: "all",
          password: "all",
          battleye: "all",
          modded: "all",
          official: "all",
          hideEmpty: false,
          hideFull: false,
          maxPing: 0,
          showExcluded: false,
          sort: "players",
          asc: false,
          offset: 0,
          limit: 6,
        });
        if (!stale) found = page.rows;
      } catch {
        if (!stale) found = [];
      } finally {
        if (!stale) asking = false;
      }
    }, 120);
    return () => {
      stale = true;
      clearTimeout(t);
    };
  });

  // ── what can be found ─────────────────────────────────────────────────
  const views = $derived(
    PLACES.map((pl): Item => ({
      key: `view:${pl.id}`,
      icon: pl.icon,
      label: String($n[pl.label]),
      also: pl.id,
      verb: $k.verbOpen.value,
      kbd: viewKey(pl.id),
      run: () => app.go(pl.id),
    })),
  );

  const actions = $derived.by((): Item[] => {
    const last = profile.data?.history?.[0];
    const out: Item[] = [
      {
        key: "act:refresh",
        icon: RefreshCw,
        label: $k.refresh.value,
        verb: $k.verbRun.value,
        kbd: keyText("refresh"),
        run: () => runShortcut("refresh"),
      },
    ];
    if (last)
      out.push({
        key: "act:rejoin",
        icon: RotateCcw,
        label: $k.rejoin.value,
        sub: last.name,
        verb: $k.verbJoin.value,
        kbd: keyText("rejoin"),
        run: () => runShortcut("rejoin"),
      });
    if (mods.stale.length > 0)
      out.push({
        key: "act:update",
        icon: Download,
        tone: "text-warn",
        label: $k.updateMods.value,
        sub: String(mods.stale.length),
        verb: $k.verbRun.value,
        kbd: keyText("updateMods"),
        run: () => runShortcut("updateMods"),
      });
    out.push(
      {
        key: "act:check",
        icon: CloudCheck,
        label: $k.checkMods.value,
        verb: $k.verbRun.value,
        run: () => {
          app.go("mods");
          void mods.checkUpdates(true);
        },
      },
      {
        key: "act:keys",
        icon: Keyboard,
        label: $k.sheet.value,
        verb: $k.verbShow.value,
        kbd: keyText("sheet"),
        // After the palette has closed, or closing it would close the sheet too.
        run: () => setTimeout(() => (dialogs.shortcuts = true), 0),
      },
    );
    return out;
  });

  const SECTIONS = [
    ["account", "sectionAccount"],
    ["steam", "sectionSteam"],
    ["apis", "sectionApis"],
    ["location", "sectionLocation"],
    ["ping", "sectionPing"],
    ["excluded", "sectionExcluded"],
    ["appearance", "sectionAppearance"],
    ["language", "sectionLanguage"],
    ["window", "sectionWindow"],
    ["controller", null],
    ["data", "sectionData"],
  ] as const;
  const settings = $derived(
    SECTIONS.map(([id, label]): Item => ({
      key: `set:${id}`,
      icon: Settings,
      label: label ? String($st[label].value) : $p.section.value,
      also: id === "appearance" ? "theme colours appearance" : id,
      sub: $k.settings.value,
      verb: $k.verbOpen.value,
      run: () => app.go("settings", id),
    })),
  );

  const favorites = $derived(
    (profile.data?.favorites ?? []).map((f): Item => ({
      key: `srv:${f.ip}:${f.port}`,
      icon: Star,
      tone: "fill-warn text-warn",
      label: f.name,
      also: `${f.ip}:${f.port}`,
      sub: `${f.ip}:${f.port}`,
      verb: $k.verbJoin.value,
      server: { ip: f.ip, port: f.port },
      run: () => void connect.address(f.ip, f.port, f.password ?? undefined),
    })),
  );
  const history = $derived(
    (profile.data?.history ?? []).map((h): Item => ({
      key: `srv:${h.ip}:${h.port}`,
      icon: History,
      label: h.name,
      also: `${h.ip}:${h.port}`,
      sub: `${h.ip}:${h.port}`,
      verb: $k.verbJoin.value,
      server: { ip: h.ip, port: h.port },
      run: () => void connect.address(h.ip, h.port),
    })),
  );
  const modItems = $derived(
    mods.installed.map((m): Item => ({
      key: `mod:${m.id}`,
      icon: Puzzle,
      tone: "text-mods",
      label: m.name,
      also: String(m.id),
      sub: `${m.id} · ${m.size_human}`,
      verb: $k.verbShow.value,
      run: () => app.go("mods", `search:${m.name}`),
    })),
  );
  const listed = $derived(
    found.map((r): Item => ({
      key: `srv:${r.ip}:${r.game_port}`,
      icon: Server,
      label: r.name,
      sub: `${r.players}/${r.max_players} · ${r.map} · ${r.ip}:${r.game_port}`,
      verb: $k.verbJoin.value,
      server: { ip: r.ip, port: r.game_port },
      run: () => void connect.server(r),
    })),
  );

  /** The items of a section that match, best first, at most `cap`. */
  function rank(items: Item[], cap = CAP): Shown[] {
    const out: Shown[] = [];
    for (const it of items) {
      const m = match(q, it.label);
      const m2 = m ? null : it.also ? match(q, it.also) : null;
      const best = m ?? (m2 ? { score: m2.score - 50, ranges: [] } : null);
      if (best) out.push({ ...it, ranges: best.ranges, score: best.score });
    }
    return out.sort((a, b) => b.score - a.score).slice(0, cap);
  }
  const plain = (items: Item[]): Shown[] => items.map((i) => ({ ...i, ranges: [], score: 0 }));

  const groups = $derived.by((): Group[] => {
    if (!q) {
      const pool = new Map(
        [...views, ...actions, ...settings, ...favorites, ...history, ...modItems].map((i) => [
          i.key,
          i,
        ]),
      );
      const rec = recent
        .map(
          (r): Item | undefined =>
            pool.get(r.key) ??
            (r.ip && r.port
              ? {
                  key: r.key,
                  icon: Server,
                  label: r.label,
                  sub: r.sub,
                  verb: $k.verbJoin.value,
                  server: { ip: r.ip, port: r.port },
                  run: () => void connect.address(r.ip!, r.port!),
                }
              : undefined),
        )
        .filter((x): x is Item => !!x)
        .slice(0, CAP);
      return [
        { id: "recent", title: $k.recent.value, items: plain(rec) },
        { id: "actions", title: $k.actions.value, items: plain(actions) },
        { id: "views", title: $k.views.value, items: plain(views) },
      ].filter((g) => g.items.length > 0);
    }
    const out: Group[] = [];
    if (address) {
      const a = `${address.host}:${address.port}`;
      out.push({
        id: "address",
        title: $k.address.value,
        items: plain([
          {
            key: `srv:${a}`,
            icon: Play,
            label: $k.connectTo({ address: a }).value,
            verb: $k.verbJoin.value,
            server: { ip: address.host, port: address.port },
            run: () => void connect.address(address.host, address.port),
          },
          {
            key: `direct:${a}`,
            icon: PlugZap,
            label: $k.directTo({ address: a }).value,
            verb: $k.verbOpen.value,
            run: () => connect.openInDirect(address.host, address.port),
          },
        ]),
      });
    }
    const favKeys = new Set(favorites.map((f) => f.key));
    const ranked: Group[] = [
      { id: "favorites", title: $k.favorites.value, items: rank(favorites) },
      {
        id: "history",
        title: $k.history.value,
        items: rank(history.filter((h) => !favKeys.has(h.key))),
      },
      {
        id: "servers",
        title: $k.servers.value,
        // The backend matched these already: kept in its order, most players first.
        items: plain(listed.filter((r) => !favKeys.has(r.key))).map((i) => ({
          ...i,
          ranges: match(q, i.label)?.ranges ?? [],
          score: 500,
        })),
      },
      { id: "mods", title: $k.mods.value, items: rank(modItems) },
      { id: "views", title: $k.views.value, items: rank(views) },
      { id: "actions", title: $k.actions.value, items: rank(actions) },
      { id: "settings", title: $k.settings.value, items: rank(settings) },
    ].filter((g) => g.items.length > 0);
    // The section with the best match first; ties keep the order above.
    ranked.sort((a, b) => (b.items[0]?.score ?? 0) - (a.items[0]?.score ?? 0));
    return [...out, ...ranked];
  });

  const flat = $derived(groups.flatMap((g) => g.items));
  /** Where each section starts in `flat`, for Tab. */
  const starts = $derived(
    groups.reduce<number[]>((acc, _, i) => {
      acc.push(i === 0 ? 0 : acc[i - 1]! + groups[i - 1]!.items.length);
      return acc;
    }, []),
  );
  $effect(() => {
    void q;
    active = 0;
  });

  function choose(item: Item | undefined) {
    if (!item) return;
    remember(item);
    dialogs.palette = false;
    item.run();
  }

  function go(i: number) {
    if (flat.length === 0) return;
    active = Math.max(0, Math.min(flat.length - 1, i));
    queueMicrotask(() =>
      list?.querySelector(`[data-index="${active}"]`)?.scrollIntoView({ block: "nearest" }),
    );
  }
  function section(delta: 1 | -1) {
    if (starts.length === 0) return;
    let g = 0;
    starts.forEach((s, i) => s <= active && (g = i));
    g = (g + delta + starts.length) % starts.length;
    go(starts[g]!);
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "ArrowDown") (e.preventDefault(), go((active + 1) % Math.max(1, flat.length)));
    else if (e.key === "ArrowUp")
      (e.preventDefault(), go((active - 1 + flat.length) % Math.max(1, flat.length)));
    else if (e.key === "PageDown") (e.preventDefault(), go(active + CAP));
    else if (e.key === "PageUp") (e.preventDefault(), go(active - CAP));
    else if (e.key === "Tab") (e.preventDefault(), section(e.shiftKey ? -1 : 1));
    else if (e.key === "Enter") (e.preventDefault(), choose(flat[active]));
  }
</script>

{#snippet keyHint(keys: string, text: string)}
  <span class="inline-flex items-center gap-1"><Kbd>{keys}</Kbd>{text}</span>
{/snippet}

<Dialog.Root bind:open={dialogs.palette}>
  <Dialog.Portal>
    <Dialog.Overlay
      class="fixed inset-0 z-dialog bg-plot/60 data-[state=open]:animate-fade-in data-[state=closed]:animate-fade-out"
    />
    <Dialog.Content
      class="fixed top-[12vh] left-1/2 z-dialog flex max-h-[72vh] w-[min(42rem,calc(100vw-2rem))] -translate-x-1/2 flex-col overflow-hidden rounded-md border border-border-strong bg-panel shadow-pop data-[state=open]:animate-pop-in data-[state=closed]:animate-pop-out"
      aria-describedby={undefined}
      data-pad-keys
      onOpenAutoFocus={(e) => {
        e.preventDefault();
        field?.focus();
      }}
    >
      <Dialog.Title class="sr-only">{$k.palette.value}</Dialog.Title>
      <div class="flex shrink-0 items-center gap-2 border-b border-border px-3">
        {#if asking}<Spinner class="size-icon shrink-0 text-accent" />{:else}<Search
            class="size-icon shrink-0 text-accent"
          />{/if}
        <input
          bind:this={field}
          bind:value={query}
          {onkeydown}
          type="text"
          spellcheck="false"
          autocomplete="off"
          role="combobox"
          aria-expanded="true"
          aria-controls="palette-list"
          aria-autocomplete="list"
          aria-activedescendant={flat[active] ? `palette-${active}` : undefined}
          placeholder={$k.placeholder.value}
          class="h-11 min-w-0 flex-1 bg-transparent text-sm text-fg outline-none placeholder:text-fg-faint"
        />
        <Kbd>Esc</Kbd>
      </div>

      <div
        bind:this={list}
        id="palette-list"
        role="listbox"
        aria-label={$k.palette.value}
        class="min-h-0 flex-1 overflow-y-auto p-1"
      >
        {#each groups as g, gi (g.id)}
          <div
            class="px-2 pt-2 pb-1 label-stencil text-fg-faint"
            role="presentation"
            id="palette-group-{g.id}"
          >
            {g.title}
          </div>
          {#each g.items as item, ii (item.key)}
            {@const i = starts[gi]! + ii}
            {@const Icon = item.icon}
            <button
              type="button"
              id="palette-{i}"
              role="option"
              aria-selected={i === active}
              data-index={i}
              tabindex="-1"
              data-pad-skip
              class={cn(
                "relative flex h-row w-full items-center gap-2.5 rounded-xs px-2 text-left text-xs",
                i === active ? "bg-raised text-fg" : "text-fg-muted",
              )}
              onmousemove={() => (active = i)}
              onclick={() => choose(item)}
            >
              {#if i === active}<span
                  class="absolute inset-y-1.5 left-0 w-0.5 rounded-full bg-accent"
                ></span>{/if}
              <Icon class={cn("size-icon shrink-0", item.tone ?? "text-fg-faint")} />
              <span class="min-w-0 truncate font-medium"
                >{#each pieces(item.label, item.ranges) as part, pi (pi)}{#if part.hit}<span
                      class="text-accent underline decoration-accent/50 underline-offset-2"
                      >{part.text}</span
                    >{:else}{part.text}{/if}{/each}</span
              >
              {#if item.sub}
                <span class="min-w-0 flex-1 truncate font-mono text-3xs text-fg-faint"
                  >{item.sub}</span
                >
              {:else}
                <span class="flex-1"></span>
              {/if}
              {#if i === active}
                <span class="shrink-0 text-2xs text-accent">{item.verb} ↵</span>
              {/if}
              {#if item.kbd}
                <Kbd>{item.kbd}</Kbd>
              {/if}
            </button>
          {/each}
        {:else}
          <p class="m-0 px-3 pt-5 pb-2 text-center text-xs text-fg-muted">
            {$k.nothing({ query: query.trim() }).value}
          </p>
        {/each}
        {#if !q || flat.length === 0}
          <div class="mx-2 mt-2 mb-1 rounded-sm border border-border/60 px-3 py-2">
            <p class="m-0 mb-1 label-stencil text-fg-faint">{$k.tryTitle.value}</p>
            <ul class="m-0 flex list-disc flex-col gap-0.5 pl-4 text-2xs text-fg-muted">
              <li>{$k.tryServer.value}</li>
              <li>{$k.tryAddress.value}</li>
              <li>{$k.tryOther.value}</li>
            </ul>
          </div>
        {/if}
      </div>

      <footer
        class="flex shrink-0 flex-wrap items-center gap-x-3 gap-y-1 border-t border-border px-3 py-1.5 text-3xs text-fg-faint"
      >
        {@render keyHint("↑↓", $k.footerMove.value)}
        {@render keyHint("Enter", $k.footerRun.value)}
        {@render keyHint("Tab", $k.footerSection.value)}
        {@render keyHint("Esc", $k.footerClose.value)}
        {#if asking}<span class="ml-auto">{$k.searching.value}</span>{/if}
      </footer>
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>
