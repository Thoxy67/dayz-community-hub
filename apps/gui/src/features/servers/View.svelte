<script lang="ts">
  import { dict } from "$lib/i18n";
  import RefreshCw from "~icons/lucide/refresh-cw";
  import PanelRight from "~icons/lucide/panel-right";
  import SearchX from "~icons/lucide/search-x";
  import CloudOff from "~icons/lucide/cloud-off";
  import Clock from "~icons/lucide/clock";
  import Server from "~icons/lucide/server";
  import { Button, IconButton } from "$lib/components/ui/button";
  import { Spinner } from "$lib/components/ui/spinner";
  import { Topo } from "$lib/components/ui/topo";
  import {
    Empty,
    Figure,
    LIST_GRID,
    LIST_NARROW_HIDDEN,
    LIST_ROW_PX as ROW_PX,
    MasterDetail,
    PageHeader,
    SortHead,
    TableHead,
  } from "$lib/components/app";
  import { cn } from "$lib/cx";
  import { compact, num } from "$lib/format";
  import type { ServerRow as Row } from "$lib/ipc/servers";
  import { app } from "$lib/stores/app.svelte";
  import { keyOf, servers, STALE_MS } from "$lib/stores/servers.svelte";
  import { profile } from "$lib/stores/profile.svelte";
  import { connect } from "$lib/stores/connect.svelte";
  import { indexOf, pad, padActions, padList } from "$lib/gamepad";
  import ServerDetail from "./detail/ServerDetail.svelte";
  import { filters, type SortCol } from "./filters.svelte";
  import { feed } from "./feed.svelte";
  import ServerRow from "./ServerRow.svelte";
  import Toolbar from "./Toolbar.svelte";

  const c = dict("servers");
  const n = dict("nav");
  const p = dict("pad");

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

  // A new request starts at the top.
  $effect(() => {
    const req = filters.request;
    void servers.total;
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

  function onkeydown(e: KeyboardEvent) {
    if (app.view !== "servers" || e.ctrlKey || e.altKey || e.metaKey) return;
    if ((e.target as HTMLElement)?.closest("input, textarea, [contenteditable], [role=dialog]"))
      return;
    switch (e.key) {
      case "ArrowDown":
        return (e.preventDefault(), move(1));
      case "ArrowUp":
        return (e.preventDefault(), move(-1));
      case "PageDown":
        return (e.preventDefault(), move(Math.floor(height / ROW_PX)));
      case "PageUp":
        return (e.preventDefault(), move(-Math.floor(height / ROW_PX)));
      case "Home":
        return (e.preventDefault(), move(-feed.total));
      case "End":
        return (e.preventDefault(), move(feed.total));
      case "Escape":
        selected = null;
        selectedIndex = -1;
        return;
    }
    const s = selected;
    if (!s) return;
    switch (e.key.toLowerCase()) {
      case "enter":
        return void connect.server(s);
      case "f":
        return void profile.toggleFavorite(s.name, s.ip, s.query_port);
      case "i":
        showDetail = !showDetail;
        return;
      case "p":
        return void servers.pingOne(s.ip, s.query_port);
      case "d":
        return connect.openInDirect(s.ip, s.game_port, s.query_port);
    }
  }

  // ── a controller: X refreshes, Y stars the server under the focus ───────
  /** The row a pad is on, else the selected one. */
  function padRow(): Row | null {
    void pad.focused;
    const i = indexOf(document.activeElement);
    return (
      (i !== null && scroller?.contains(document.activeElement) ? feed.rows.get(i) : null) ??
      selected
    );
  }
  padActions("servers", {
    primary: { label: () => $p.refresh.value, run: () => void servers.refresh() },
    secondary: {
      label: () => $p.favorite.value,
      when: () => padRow() !== null,
      run: () => {
        const r = padRow();
        if (r) void profile.toggleFavorite(r.name, r.ip, r.query_port);
      },
    },
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
      <div class="relative grid flex-1 place-items-center">
        <Topo />
        <div class="relative flex flex-col items-center gap-2 text-fg-muted">
          <Spinner class="size-6 text-accent" />
          <span class="label-stencil">{$c.loading.value}</span>
        </div>
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
                    <div
                      class={cn(LIST_GRID, "h-full border-b border-border/50 px-2")}
                      aria-hidden="true"
                    >
                      <span></span>
                      <span class="h-2 w-12 animate-pulse rounded-full bg-raised"></span>
                      <span class="h-2 w-16 animate-pulse rounded-full bg-raised"></span>
                      <span class="h-2.5 w-3/5 animate-pulse rounded-full bg-raised"></span>
                      <span
                        class={cn(
                          "h-2 w-20 animate-pulse rounded-full bg-raised",
                          LIST_NARROW_HIDDEN,
                        )}
                      ></span>
                      <span></span><span></span>
                    </div>
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
      <Figure label={$c.statShown.value} value={num(feed.total)} />
      <Figure label={$c.statPlayers.value} value={compact(fig?.players)} tone="text-ok" />
      <Figure
        label={$c.statFull.value}
        value={num(fig?.full)}
        tone={fig?.full ? "text-err" : "text-fg-muted"}
      />
      <Figure secondary label={$c.statEmpty.value} value={num(fig?.empty)} tone="text-fg-muted" />
      <Figure
        label={$c.statModded.value}
        value={fig && fig.shown ? `${Math.round((fig.modded / fig.shown) * 100)}%` : "—"}
        tone="text-mods"
      />
      <Figure secondary label={$c.statPinged.value} value={num(fig?.pinged)} tone="text-fg-muted" />
      <Figure
        secondary
        label={$c.statBestPing.value}
        value={fig?.best_ping != null ? `${fig.best_ping} ms` : "—"}
        tone="text-ok"
      />
    {/snippet}
    {#snippet actions()}
      <span class="mr-2 hidden font-mono text-3xs text-fg-faint 2xl:inline">{$c.keys.value}</span>
      <IconButton
        icon={PanelRight}
        label={$c.toggleDetails.value}
        kbd="I"
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
  </PageHeader>

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
