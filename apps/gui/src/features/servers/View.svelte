<script lang="ts">
  import { dict } from "$lib/i18n";
  import RefreshCw from "~icons/lucide/refresh-cw";
  import PanelRight from "~icons/lucide/panel-right";
  import SearchX from "~icons/lucide/search-x";
  import CloudOff from "~icons/lucide/cloud-off";
  import Clock from "~icons/lucide/clock";
  import Server from "~icons/lucide/server";
  import { Button, IconButton } from "$lib/components/ui/button";
  import { Skeleton } from "$lib/components/ui/skeleton";
  import {
    Empty,
    Figure,
    LIST_GRID,
    LIST_NARROW_HIDDEN,
    LIST_ROW_PX as ROW_PX,
    MasterDetail,
    PageHeader,
    ShortcutsButton,
    SortHead,
    TableHead,
  } from "$lib/components/app";
  import { cn } from "$lib/cx";
  import { compact } from "$lib/format";
  import type { ServerRow as Row } from "$lib/ipc/servers";
  import { app } from "$lib/stores/app.svelte";
  import { dialogs } from "$lib/stores/dialogs.svelte";
  import { keyOf, servers, STALE_MS } from "$lib/stores/servers.svelte";
  import { profile } from "$lib/stores/profile.svelte";
  import { connect } from "$lib/stores/connect.svelte";
  import { session } from "$lib/stores/session.svelte";
  import { indexOf, pad, padActions, padList } from "$lib/gamepad";
  import { idOf, keyText, TYPING } from "$lib/shortcuts/registry";
  import { copyAddress } from "$lib/shortcuts/server";
  import ServerDetail from "./detail/ServerDetail.svelte";
  import { filters, type SortCol } from "./filters.svelte";
  import { feed } from "./feed.svelte";
  import ServerRow from "./ServerRow.svelte";
  import SourcesBar from "./SourcesBar.svelte";
  import Toolbar from "./Toolbar.svelte";
  import ActiveFilters from "./ActiveFilters.svelte";

  const c = dict("servers");
  const n = dict("nav");

  // ── the one long list ───────────────────────────────────────────────────
  // As tall as every matching server; only the rows in view exist, and only
  // the blocks around them are asked of the backend (see feed.svelte.ts).
  let scroller: HTMLDivElement | undefined = $state();
  let scrollTop = $state(0);
  let height = $state(600);
  const OVERSCAN = 8;
  const first = $derived(Math.max(0, Math.floor(scrollTop / ROW_PX) - OVERSCAN));
  const last = $derived(Math.min(feed.total, Math.ceil((scrollTop + height) / ROW_PX) + OVERSCAN));
  const indices = $derived(Array.from({ length: Math.max(0, last - first) }, (_, i) => first + i));

  let raf = 0;
  function onscroll() {
    if (raf) return;
    raf = requestAnimationFrame(() => {
      raf = 0;
      if (scroller) scrollTop = scroller.scrollTop;
    });
  }

  $effect(() => {
    if (!scroller) return;
    const el = scroller;
    // Never more than the window: a guard against a layout that lets the
    // scroller grow with its content, which would mount every row.
    const measure = () => (height = Math.min(el.clientHeight, window.innerHeight));
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    measure();
    return () => ro.disconnect();
  });

  // A new request starts at the top. Only a request that changed: the list
  // growing (Steam's servers arriving for a minute after a refresh) used to
  // throw the player back to the top every time.
  let lastRequest = "";
  $effect(() => {
    const req = filters.request;
    const key = JSON.stringify(req);
    if (key === lastRequest) return;
    lastRequest = key;
    feed.setRequest(req);
    if (scroller) scroller.scrollTop = 0;
    scrollTop = 0;
  });
  // The window asks for its blocks as it moves.
  $effect(() => feed.show(first, last));
  // The backend's data changed: the rows in view are asked for again.
  $effect(() => {
    void servers.generation;
    feed.refresh();
  });

  // ── selection ───────────────────────────────────────────────────────────
  // Raw: rows are replaced whole by the feed, never edited in place.
  let selected = $state.raw<Row | null>(null);
  let selectedIndex = $state(-1);
  let showDetail = $state(true);
  let focusMods = $state(false);
  // The selected row is kept fresh from the feed when it is in view.
  $effect(() => {
    const r = selectedIndex >= 0 ? feed.rows.get(selectedIndex) : undefined;
    if (r && r !== selected && selected && keyOf(r) === keyOf(selected)) selected = r;
  });

  function select(r: Row, index: number, mods = false) {
    selected = r;
    selectedIndex = index;
    focusMods = mods;
    if (mods) showDetail = true;
  }

  // ── after a reload: the scroll, the selection, the detail panel ─────────
  // Kept in the session (lib/stores/session) and put back once the list
  // they refer to has arrived. Until then the saved values are what is kept,
  // so a second reload before they apply loses nothing.
  type Kept = { scroll: number; key: string | null; index: number; detail: boolean };
  let wantScroll = -1;
  let wantKey = $state<string | null>(null);
  let wantIndex = -1;
  const forget = session.keep<Kept>(
    "servers.view",
    () => ({
      scroll: wantScroll >= 0 ? wantScroll : scrollTop,
      key: selected ? keyOf(selected) : wantKey,
      index: selected ? selectedIndex : wantIndex,
      detail: showDetail,
    }),
    (v) => {
      wantScroll = v.scroll;
      wantKey = v.key;
      wantIndex = v.index;
      showDetail = v.detail;
    },
  );
  $effect(() => forget);
  $effect(() => {
    if (wantScroll < 0 || !scroller || feed.loading || feed.total === 0) return;
    const top = Math.min(wantScroll, feed.total * ROW_PX);
    wantScroll = -1;
    scroller.scrollTop = top;
    scrollTop = scroller.scrollTop;
  });
  $effect(() => {
    const key = wantKey;
    if (!key || selected || servers.total === 0) return;
    wantKey = null;
    const at = key.lastIndexOf(":");
    void servers.resolve(key.slice(0, at), Number(key.slice(at + 1))).then((r) => {
      if (r && !selected) {
        selected = r;
        selectedIndex = wantIndex;
      }
    });
  });

  // Screenshot aid, in mock and dev builds only: `?select=N` selects row N
  // once it has arrived, so a capture can show the detail panel.
  const wantSelect =
    import.meta.env.DEV || import.meta.env.VITE_MOCK === "1"
      ? Number(new URLSearchParams(location.search).get("select") ?? NaN)
      : NaN;
  $effect(() => {
    if (Number.isNaN(wantSelect) || selected) return;
    const r = feed.rows.get(wantSelect);
    if (r) select(r, wantSelect);
  });

  function scrollIntoView(i: number) {
    if (!scroller) return;
    const top = i * ROW_PX;
    if (top < scroller.scrollTop) scroller.scrollTop = top;
    else if (top + ROW_PX > scroller.scrollTop + scroller.clientHeight) {
      scroller.scrollTop = top + ROW_PX - scroller.clientHeight;
    }
  }

  function move(delta: number) {
    if (feed.total === 0) return;
    const i = Math.max(
      0,
      Math.min(feed.total - 1, (selectedIndex < 0 ? -1 : selectedIndex) + delta),
    );
    scrollIntoView(i);
    const r = feed.rows.get(i);
    selectedIndex = i;
    if (r) selected = r;
  }

  // ── keys (lib/shortcuts/registry.ts, scope "list"), only while this view
  //    shows and nothing else has them ─────────────────────────────────────
  function onkeydown(e: KeyboardEvent) {
    if (app.view !== "servers" || dialogs.open || connect.request) return;
    if ((e.target as HTMLElement)?.closest(`${TYPING}, [role=dialog]`)) return;
    const id = idOf(e, "list");
    if (!id) return;
    const rows = Math.floor(height / ROW_PX);
    switch (id) {
      case "list.move":
        return (e.preventDefault(), move(e.key === "ArrowDown" ? 1 : -1));
      case "list.page":
        return (e.preventDefault(), move(e.key === "PageDown" ? rows : -rows));
      case "list.edges":
        return (e.preventDefault(), move(e.key === "End" ? feed.total : -feed.total));
      case "list.close":
        selected = null;
        selectedIndex = -1;
        return;
      case "list.detail":
        showDetail = !showDetail;
        return;
    }
    const s = selected;
    if (!s) return;
    // Enter on a focused button presses that button, not Join as well.
    if (id === "list.join" && (e.target as HTMLElement)?.closest("button, a, [role=menu]")) return;
    e.preventDefault();
    act(id, s);
  }

  /** A list shortcut on a server. */
  function act(id: string, s: Row) {
    switch (id) {
      case "list.join":
        return void connect.server(s);
      case "list.fav":
        return void profile.toggleFavorite(s.name, s.ip, s.query_port);
      case "list.ping":
        return void servers.pingOne(s.ip, s.query_port);
      case "list.direct":
        return connect.openInDirect(s.ip, s.game_port, s.query_port);
      case "list.copy":
        return void copyAddress(s.ip, s.game_port);
      case "list.link":
        return void connect.copyLink({
          ip: s.ip,
          gamePort: s.game_port,
          queryPort: s.query_port,
          name: s.name,
        });
    }
  }

  // ── a controller: X stars and Y pings the server under the focus (Y
  //    refreshes the list when no server is), R3 the details ─────────────
  /** The row a pad is on, else the selected one. */
  function padRow(): Row | null {
    void pad.focused;
    const i = indexOf(document.activeElement);
    return (
      (i !== null && scroller?.contains(document.activeElement) ? feed.rows.get(i) : null) ??
      selected
    );
  }
  const onRow = (id: string) => ({
    when: () => padRow() !== null,
    run: () => {
      const r = padRow();
      if (r) act(id, r);
    },
  });
  padActions("servers", {
    "list.fav": onRow("list.fav"),
    "list.ping": onRow("list.ping"),
    "servers.refresh": { run: () => void servers.refresh() },
    "list.detail": { run: () => (showDetail = !showDetail) },
  });

  // ── the list's age ──────────────────────────────────────────────────────
  let now = $state(Date.now());
  $effect(() => {
    const t = setInterval(() => (now = Date.now()), 30_000);
    return () => clearInterval(t);
  });
  const staleMinutes = $derived(
    servers.lastRefreshed > 0 && now - servers.lastRefreshed > STALE_MS
      ? Math.floor((now - servers.lastRefreshed) / 60_000)
      : 0,
  );

  const COLS: { id: Exclude<SortCol, "none">; label: () => string; title?: () => string }[] = [
    { id: "ping", label: () => $c.colPing.value },
    { id: "players", label: () => $c.colPlayers.value },
    { id: "name", label: () => $c.colServer.value },
    { id: "map", label: () => $c.colMap.value },
    { id: "time", label: () => $c.colTime.value, title: () => $c.colTimeTitle.value },
    { id: "mods", label: () => $c.colMods.value },
  ];
  const fig = $derived(feed.stats);
</script>

<svelte:window {onkeydown} />

{#snippet placeholder()}
  <div class={cn(LIST_GRID, "h-full border-b border-border/50 px-2")} aria-hidden="true">
    <span></span>
    <Skeleton class="w-12" />
    <Skeleton class="w-16" />
    <Skeleton class="h-2.5 w-3/5" />
    <Skeleton class={cn("w-20", LIST_NARROW_HIDDEN)} />
    <span></span><span></span>
  </div>
{/snippet}

{#snippet listPane()}
  <div class="@container flex h-full min-h-0 flex-1 flex-col">
    <TableHead grid={LIST_GRID}>
      <span></span>
      {#each COLS.filter((col) => col.id === "ping" || col.id === "players" || col.id === "name") as col (col.id)}
        <SortHead
          label={col.label()}
          active={filters.sort === col.id}
          asc={filters.asc}
          onclick={() => filters.toggleSort(col.id)}
        />
      {/each}
      <span class={cn("flex items-center gap-2", LIST_NARROW_HIDDEN)}>
        <SortHead
          label={$c.colMap.value}
          active={filters.sort === "map"}
          asc={filters.asc}
          onclick={() => filters.toggleSort("map")}
        />
        <SortHead
          label={$c.colTime.value}
          title={$c.colTimeTitle.value}
          active={filters.sort === "time"}
          asc={filters.asc}
          onclick={() => filters.toggleSort("time")}
        />
      </span>
      <SortHead
        label={$c.colMods.value}
        active={filters.sort === "mods"}
        asc={filters.asc}
        onclick={() => filters.toggleSort("mods")}
      />
      <span></span>
    </TableHead>

    {#if feed.loading && feed.total === 0}
      <!-- The list's own shape while the first page comes: rows, not a spinner. -->
      <div class="min-h-0 flex-1 overflow-hidden" role="status" aria-label={$c.loading.value}>
        {#each { length: 14 }, i (i)}
          <div style:height="{ROW_PX}px">{@render placeholder()}</div>
        {/each}
      </div>
    {:else if servers.total === 0 && servers.listError}
      <Empty icon={CloudOff} title={$c.listUnavailable.value} class="flex-1">
        <span class="font-mono text-2xs" data-selectable>{servers.listError}</span>
        {#snippet action()}
          <Button variant="accent" disabled={servers.refreshing} onclick={() => servers.refresh()}>
            <RefreshCw class={cn("size-icon-sm", servers.refreshing && "animate-spin")} />{$c
              .refresh.value}
          </Button>
        {/snippet}
      </Empty>
    {:else if feed.total === 0}
      <Empty icon={SearchX} title={$c.noMatch.value} class="flex-1">
        {#snippet action()}
          {#if filters.active}
            <Button variant="accent" onclick={() => filters.clear()}>{$c.clearFilters.value}</Button
            >
          {/if}
        {/snippet}
      </Empty>
    {:else}
      <!-- The scroller is pinned to a box whose size does not depend on its
           content: if it could grow with the list, "the rows in view" would be
           all nine thousand of them. -->
      <div class="relative min-h-0 flex-1">
        <div
          bind:this={scroller}
          {onscroll}
          role="grid"
          aria-label={$c.serverList.value}
          aria-rowcount={feed.total}
          class="absolute inset-0 overflow-y-auto overscroll-contain"
          use:padList={{
            count: () => feed.total,
            reveal: scrollIntoView,
            current: () => selectedIndex,
            page: () => Math.max(1, Math.floor(height / ROW_PX) - 1),
          }}
        >
          <div class="relative" style:height="{feed.total * ROW_PX}px">
            <div class="absolute inset-x-0 top-0" style:transform="translateY({first * ROW_PX}px)">
              {#each indices as i (i)}
                {@const r = feed.rows.get(i)}
                <div style:height="{ROW_PX}px" data-pad-index={i} tabindex="-1">
                  {#if r}
                    <ServerRow
                      server={r}
                      index={i}
                      selected={selected !== null && keyOf(selected) === keyOf(r)}
                      onselect={() => select(r, i)}
                      onjoin={() => connect.server(r)}
                      onmods={() => select(r, i, true)}
                    />
                  {:else}
                    <!-- Not arrived yet: the row's shape, so the list does not jump. -->
                    {@render placeholder()}
                  {/if}
                </div>
              {/each}
            </div>
          </div>
        </div>
      </div>
    {/if}
  </div>
{/snippet}

{#snippet detailPane()}
  {#if selected}
    <ServerDetail
      ip={selected.ip}
      port={selected.query_port}
      name={selected.name}
      {focusMods}
      onclose={() => {
        selected = null;
        selectedIndex = -1;
      }}
    />
  {:else}
    <Empty icon={Server} title={$c.selectServer.value} />
  {/if}
{/snippet}

<div class="flex h-full min-h-0 flex-col">
  <PageHeader title={$n.servers.value}>
    {#snippet stats()}
      <Figure label={$c.statShown.value} amount={feed.total} />
      <Figure
        label={$c.statPlayers.value}
        amount={fig?.players}
        format={compact}
        tone="text-ok"
        title={$c.figSortPlayers.value}
        active={filters.sort === "players" && !filters.asc}
        onclick={() => {
          filters.sort = "players";
          filters.asc = false;
        }}
      />
      <Figure
        label={$c.statFull.value}
        amount={fig?.full}
        tone={fig?.full ? "text-err" : "text-fg-muted"}
        title={filters.hideFull ? $c.figShowFull.value : $c.figHideFull.value}
        active={filters.hideFull}
        onclick={() => (filters.hideFull = !filters.hideFull)}
      />
      <Figure
        secondary
        label={$c.statEmpty.value}
        amount={fig?.empty}
        tone="text-fg-muted"
        title={filters.hideEmpty ? $c.figShowEmpty.value : $c.figHideEmpty.value}
        active={filters.hideEmpty}
        onclick={() => (filters.hideEmpty = !filters.hideEmpty)}
      />
      <Figure
        label={$c.statModded.value}
        value={fig && fig.shown ? `${Math.round((fig.modded / fig.shown) * 100)}%` : "—"}
        tone="text-mods"
        title={filters.modded === "only" ? $c.figAllMods.value : $c.filterModsTitleAll.value}
        active={filters.modded === "only"}
        onclick={() => (filters.modded = filters.modded === "only" ? "all" : "only")}
      />
      <Figure secondary label={$c.statPinged.value} amount={fig?.pinged} tone="text-fg-muted" />
      <Figure
        secondary
        label={$c.statBestPing.value}
        value={fig?.best_ping != null ? `${fig.best_ping} ms` : "—"}
        tone="text-ok"
        title={$c.figSortPing.value}
        active={filters.sort === "ping" && filters.asc}
        onclick={() => {
          filters.sort = "ping";
          filters.asc = true;
        }}
      />
    {/snippet}
    {#snippet actions()}
      <ShortcutsButton />
      <IconButton
        icon={PanelRight}
        label={$c.toggleDetails.value}
        kbd={keyText("list.detail")}
        active={showDetail}
        onclick={() => (showDetail = !showDetail)}
      />
      <Button
        onclick={() => servers.refresh()}
        disabled={servers.refreshing}
        title={$c.refreshTitle.value}
      >
        <RefreshCw class={cn("size-icon-sm", servers.refreshing && "animate-spin")} />{$c.refresh
          .value}
      </Button>
    {/snippet}
    <Toolbar shown={feed.total} />
    <ActiveFilters />
  </PageHeader>
  <SourcesBar />

  {#if staleMinutes > 0 && !servers.loading}
    <div
      class="flex items-center gap-2 border-b border-warn/25 bg-warn/10 px-pad py-1 text-2xs text-warn"
    >
      <Clock class="size-3.5" />{$c.staleData({ minutes: staleMinutes }).value}
      <button
        class="ml-auto rounded-xs px-1.5 font-medium hover:bg-warn/15"
        onclick={() => servers.refresh()}
      >
        {$c.refresh.value}
      </button>
    </div>
  {/if}

  <div class="flex min-h-0 flex-1 flex-col">
    <MasterDetail
      id="servers-detail"
      show={showDetail}
      selected={selected !== null}
      main={listPane}
      detail={detailPane}
    />
  </div>
</div>
