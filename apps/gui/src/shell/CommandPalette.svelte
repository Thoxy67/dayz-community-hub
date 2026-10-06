<script lang="ts">
  import type { Component } from "svelte";
  import { Dialog } from "bits-ui";
  import { dict } from "$lib/i18n";
  import Search from "~icons/lucide/search";
  import Star from "~icons/lucide/star";
  import Server from "~icons/lucide/server";
  import Puzzle from "~icons/lucide/puzzle";
  import RefreshCw from "~icons/lucide/refresh-cw";
  import RotateCcw from "~icons/lucide/rotate-ccw";
  import Download from "~icons/lucide/download";
  import CloudCheck from "~icons/lucide/cloud-check";
  import Keyboard from "~icons/lucide/keyboard";
  import { Kbd } from "$lib/components/ui/kbd";
  import { cn } from "$lib/cx";
  import { serversQuery, type ServerRow } from "$lib/ipc/servers";
  import { app } from "$lib/stores/app.svelte";
  import { connect } from "$lib/stores/connect.svelte";
  import { dialogs } from "$lib/stores/dialogs.svelte";
  import { mods } from "$lib/stores/mods.svelte";
  import { profile } from "$lib/stores/profile.svelte";
  import { servers } from "$lib/stores/servers.svelte";
  import { PLACES } from "./nav";

  /**
   * Ctrl+K: one field for everything the window holds. Pages and actions
   * answer at once, favourites and installed mods are matched here, servers
   * are asked of the backend (the same filtered query the browser runs), a
   * few at a time. Enter does the obvious thing: open a page, run an action,
   * join a server, show a mod.
   */
  const s = dict("shell");
  const n = dict("nav");

  type Item = {
    key: string;
    icon: Component<{ class?: string }>;
    label: string;
    sub?: string;
    hint: string;
    kbd?: string;
    run: () => void;
  };
  type Group = { title: string; items: Item[] };

  let query = $state("");
  let active = $state(0);
  let found = $state.raw<ServerRow[]>([]);
  let list: HTMLDivElement | undefined = $state();

  /** Lower case, accents gone: "Énoch" finds "enoch". */
  const fold = (t: string) => t.normalize("NFD").replace(/\p{M}/gu, "").toLowerCase();
  const q = $derived(fold(query.trim()));
  const hit = (...texts: (string | null | undefined)[]) =>
    !q || texts.some((t) => t && fold(t).includes(q));

  // A fresh palette each time it opens.
  $effect(() => {
    if (dialogs.palette) {
      query = "";
      active = 0;
      found = [];
    }
  });

  // Servers: asked once typing pauses, the most populated first.
  $effect(() => {
    const term = query.trim();
    if (!dialogs.palette || term.length < 2) {
      found = [];
      return;
    }
    let stale = false;
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
      }
    }, 120);
    return () => {
      stale = true;
      clearTimeout(t);
    };
  });

  function close() {
    dialogs.palette = false;
  }

  const groups = $derived.by((): Group[] => {
    const out: Group[] = [];
    const go = $s.paletteGo.value;
    const runVerb = $s.paletteRun.value;
    const join = $s.paletteJoin.value;

    const pages = PLACES.map((p, i) => ({ p, i }))
      .filter(({ p }) => hit(String($n[p.label]), p.id))
      .map(({ p, i }): Item => ({
        key: `page:${p.id}`,
        icon: p.icon,
        label: String($n[p.label]),
        hint: go,
        kbd: i < 9 ? `Ctrl ${i + 1}` : undefined,
        run: () => app.go(p.id),
      }));

    const actions: Item[] = [
      {
        key: "act:refresh",
        icon: RefreshCw,
        label: $s.paletteRefreshServers.value,
        hint: runVerb,
        kbd: "Ctrl R",
        run: () => void servers.refresh(),
      },
      ...(profile.data?.history?.[0]
        ? [
            {
              key: "act:rejoin",
              icon: RotateCcw,
              label: $s.paletteRejoin.value,
              sub: profile.data.history[0].name,
              hint: join,
              kbd: "Ctrl L",
              run: () => void connect.rejoin(),
            },
          ]
        : []),
      ...(mods.stale.length > 0
        ? [
            {
              key: "act:update",
              icon: Download,
              label: $s.paletteUpdateMods.value,
              sub: String(mods.stale.length),
              hint: runVerb,
              kbd: "Ctrl U",
              run: () => {
                app.go("mods");
                mods.updateStale();
              },
            },
          ]
        : []),
      {
        key: "act:check",
        icon: CloudCheck,
        label: $s.paletteCheckMods.value,
        hint: runVerb,
        run: () => {
          app.go("mods");
          void mods.checkUpdates(true);
        },
      },
      {
        key: "act:keys",
        icon: Keyboard,
        label: $s.paletteShortcuts.value,
        hint: go,
        kbd: "?",
        run: () => (dialogs.shortcuts = true),
      },
    ].filter((a) => hit(a.label, a.sub));

    const favorites = (profile.data?.favorites ?? [])
      .filter((f) => q && hit(f.name, `${f.ip}:${f.port}`))
      .slice(0, 5)
      .map((f): Item => ({
        key: `fav:${f.ip}:${f.port}`,
        icon: Star,
        label: f.name,
        sub: `${f.ip}:${f.port}`,
        hint: join,
        run: () => void connect.address(f.ip, f.port, f.password ?? undefined),
      }));

    const favKeys = new Set(favorites.map((f) => f.sub));
    const listed = found
      .filter((r) => !favKeys.has(`${r.ip}:${r.query_port}`))
      .map((r): Item => ({
        key: `srv:${r.ip}:${r.query_port}`,
        icon: Server,
        label: r.name,
        sub: `${r.players}/${r.max_players} · ${r.map} · ${r.ip}:${r.game_port}`,
        hint: join,
        run: () => void connect.server(r),
      }));

    const modItems = mods.installed
      .filter((m) => q && hit(m.name, String(m.id)))
      .slice(0, 5)
      .map((m): Item => ({
        key: `mod:${m.id}`,
        icon: Puzzle,
        label: m.name,
        sub: `${m.id} · ${m.size_human}`,
        hint: go,
        run: () => app.go("mods", `search:${m.name}`),
      }));

    // Typing for something specific: what matches it comes first.
    if (q) {
      if (favorites.length) out.push({ title: $s.paletteFavorites.value, items: favorites });
      if (listed.length) out.push({ title: $s.paletteServers.value, items: listed });
      if (modItems.length) out.push({ title: $s.paletteMods.value, items: modItems });
    }
    if (actions.length) out.push({ title: $s.paletteActions.value, items: actions });
    if (pages.length) out.push({ title: $s.palettePages.value, items: pages });
    return out;
  });

  const flat = $derived(groups.flatMap((g) => g.items));
  $effect(() => {
    void q;
    active = 0;
  });

  function choose(item: Item | undefined) {
    if (!item) return;
    close();
    item.run();
  }

  function move(delta: number) {
    if (flat.length === 0) return;
    active = (active + delta + flat.length) % flat.length;
    queueMicrotask(() =>
      list?.querySelector(`[data-index="${active}"]`)?.scrollIntoView({ block: "nearest" }),
    );
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === "ArrowDown") (e.preventDefault(), move(1));
    else if (e.key === "ArrowUp") (e.preventDefault(), move(-1));
    else if (e.key === "Enter") (e.preventDefault(), choose(flat[active]));
  }
</script>

<Dialog.Root bind:open={dialogs.palette}>
  <Dialog.Portal>
    <Dialog.Overlay class="fixed inset-0 z-dialog bg-plot/60" />
    <Dialog.Content
      class="fixed top-[12vh] left-1/2 z-dialog flex max-h-[70vh] w-[min(40rem,calc(100vw-2rem))] -translate-x-1/2 flex-col overflow-hidden rounded-md border border-border-strong bg-panel shadow-pop"
      aria-describedby={undefined}
    >
      <Dialog.Title class="sr-only">{$s.paletteTitle.value}</Dialog.Title>
      <div class="flex shrink-0 items-center gap-2 border-b border-border px-3">
        <Search class="size-icon shrink-0 text-accent" />
        <!-- svelte-ignore a11y_autofocus -->
        <input
          bind:value={query}
          {onkeydown}
          autofocus
          type="text"
          spellcheck="false"
          autocomplete="off"
          role="combobox"
          aria-expanded="true"
          aria-controls="palette-list"
          aria-activedescendant={flat[active] ? `palette-${active}` : undefined}
          placeholder={$s.palettePlaceholder.value}
          class="h-11 min-w-0 flex-1 bg-transparent text-sm text-fg outline-none placeholder:text-fg-faint"
        />
        <Kbd>Esc</Kbd>
      </div>

      <div
        bind:this={list}
        id="palette-list"
        role="listbox"
        class="min-h-0 flex-1 overflow-y-auto p-1"
      >
        {#each groups as g (g.title)}
          <div class="px-2 pt-2 pb-1 label-stencil text-fg-faint">{g.title}</div>
          {#each g.items as item (item.key)}
            {@const i = flat.indexOf(item)}
            {@const Icon = item.icon}
            <button
              type="button"
              id="palette-{i}"
              role="option"
              aria-selected={i === active}
              data-index={i}
              tabindex="-1"
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
              <Icon
                class={cn(
                  "size-icon shrink-0",
                  item.key.startsWith("fav:") ? "fill-warn text-warn" : "text-fg-faint",
                  item.key.startsWith("mod:") && "text-mods",
                )}
              />
              <span class="min-w-0 truncate font-medium">{item.label}</span>
              {#if item.sub}
                <span class="min-w-0 flex-1 truncate font-mono text-3xs text-fg-faint"
                  >{item.sub}</span
                >
              {:else}
                <span class="flex-1"></span>
              {/if}
              {#if i === active}
                <span class="shrink-0 text-2xs text-accent">{item.hint} ↵</span>
              {:else if item.kbd}
                <Kbd>{item.kbd}</Kbd>
              {/if}
            </button>
          {/each}
        {:else}
          <p class="m-0 px-3 py-6 text-center text-xs text-fg-muted">
            {$s.paletteNothing({ query: query.trim() }).value}
          </p>
        {/each}
      </div>

      <footer class="shrink-0 border-t border-border px-3 py-1.5 font-mono text-3xs text-fg-faint">
        {$s.paletteHelp.value}
      </footer>
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>
