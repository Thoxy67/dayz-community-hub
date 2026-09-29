<script lang="ts">
  import { DropdownMenu as Menu } from "bits-ui";
  import { dict } from "$lib/i18n";
  import Puzzle from "~icons/lucide/puzzle";
  import CloudDownload from "~icons/lucide/cloud-download";
  import RefreshCw from "~icons/lucide/refresh-cw";
  import RotateCw from "~icons/lucide/rotate-cw";
  import Download from "~icons/lucide/download";
  import FolderOpen from "~icons/lucide/folder-open";
  import Ellipsis from "~icons/lucide/ellipsis";
  import Brush from "~icons/lucide/brush-cleaning";
  import Trash from "~icons/lucide/trash-2";
  import KeyRound from "~icons/lucide/key-round";
  import Link from "~icons/lucide/link";
  import Unlink from "~icons/lucide/unlink";
  import HardDrive from "~icons/lucide/hard-drive-download";
  import X from "~icons/lucide/x";
  import SearchX from "~icons/lucide/search-x";
  import {
    PageHeader,
    Figure,
    SortHead,
    TableHead,
    Empty,
    BulkBar,
    MasterDetail,
  } from "$lib/components/app";
  import { Button, IconButton } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import { Segmented } from "$lib/components/ui/segmented";
  import { Checkbox } from "$lib/components/ui/checkbox";
  import { VirtualList } from "$lib/components/ui/virtual-list";
  import { Alert } from "$lib/components/ui/alert";
  import { Spinner } from "$lib/components/ui/spinner";
  import { Tooltip } from "$lib/components/ui/tooltip";
  import { SteamIcon } from "$lib/components/ui/brand";
  import {
    DropdownMenuContent,
    DropdownMenuItem,
    DropdownMenuSeparator,
  } from "$lib/components/ui/dropdown-menu";
  import { cn } from "$lib/cx";
  import { bytes, date, relative } from "$lib/format";
  import type { InstalledModDto } from "$lib/ipc/types";
  import { app } from "$lib/stores/app.svelte";
  import { mods } from "$lib/stores/mods.svelte";
  import { profile } from "$lib/stores/profile.svelte";
  import ModDetail from "./ModDetail.svelte";
  import ModState from "./ModState.svelte";
  import ModMenu from "./ModMenu.svelte";
  import ReviewDialog from "./ReviewDialog.svelte";
  import InstallDialog from "./InstallDialog.svelte";
  import { review } from "./review.svelte";

  /**
   * Installed Workshop mods. The header says how many there are, what they
   * weigh and how many are behind, with the one button that brings them
   * level; each row says in words whether that mod is current, whose folder
   * it is in and whether the game loads it; everything else is in its menu,
   * its details, or the bar that appears once some are ticked.
   */
  const m = dict("mods");

  $effect(() => {
    // Once: with nothing installed, re-reading whenever the (empty) list
    // arrived again read the disk in a loop.
    if (!mods.loaded && !mods.loading) void mods.load();
    void mods.checkUpdates();
  });

  const hasKey = $derived(!!profile.data?.steam_api_key);

  // ── filter and sort ─────────────────────────────────────────────────────
  type Filter = "all" | "updates" | "unmanaged";
  type Col = "name" | "status" | "managed" | "size" | "local";
  let filter = $state<Filter>("all");
  let query = $state("");
  let col = $state<Col>("status");
  let asc = $state(true);

  const unmanaged = $derived(mods.installed.filter((x) => !x.managed).length);

  const status = (x: InstalledModDto) => (x.update_available ? 0 : x.remote_updated ? 2 : 1);
  const rows = $derived.by(() => {
    const q = query.trim().toLowerCase();
    const list = mods.installed.filter(
      (x) =>
        (filter === "all" || (filter === "updates" ? x.update_available : !x.managed)) &&
        (!q || x.name.toLowerCase().includes(q) || String(x.id).includes(q)),
    );
    const dir = asc ? 1 : -1;
    const byName = (a: InstalledModDto, b: InstalledModDto) => a.name.localeCompare(b.name);
    return list.sort((a, b) => {
      switch (col) {
        case "name":
          return dir * byName(a, b);
        case "size":
          return dir * (a.size - b.size);
        case "local":
          return dir * (a.local_updated - b.local_updated);
        case "managed":
          return dir * (Number(b.managed) - Number(a.managed)) || byName(a, b);
        default:
          return dir * (status(a) - status(b)) || byName(a, b);
      }
    });
  });

  function sortBy(c: Col) {
    if (col === c) asc = !asc;
    else {
      col = c;
      asc = c === "name" || c === "status" || c === "managed";
    }
  }

  // ── selection: the ticked ones (bulk) and the one shown in the side pane ─
  let ticked = $state<Set<number>>(new Set());
  let focusId = $state<number | null>(null);
  const focused = $derived(focusId !== null ? (mods.byId.get(focusId) ?? null) : null);
  $effect(() => {
    // Forget ticks for mods that are gone.
    const pruned = new Set([...ticked].filter((id) => mods.byId.has(id)));
    if (pruned.size !== ticked.size) ticked = pruned;
  });
  const tickedMods = $derived(mods.installed.filter((x) => ticked.has(x.id)));
  const tickedSize = $derived(tickedMods.reduce((a, x) => a + x.size, 0));
  const tickedStale = $derived(tickedMods.filter((x) => x.update_available).length);
  const tickedLinked = $derived(tickedMods.filter((x) => x.managed).length);
  const allTicked = $derived(rows.length > 0 && rows.every((x) => ticked.has(x.id)));
  const someTicked = $derived(!allTicked && rows.some((x) => ticked.has(x.id)));

  function tick(id: number, on: boolean) {
    const next = new Set(ticked);
    if (on) next.add(id);
    else next.delete(id);
    ticked = next;
  }
  function tickAll(on: boolean) {
    ticked = on ? new Set(rows.map((x) => x.id)) : new Set();
  }
  async function linkTicked(link: boolean) {
    for (const x of tickedMods) if (x.managed !== link) await mods.toggleManaged(x);
  }

  // ── keyboard (only while this view is showing) ──────────────────────────
  let list = $state<{ scrollToIndex: (i: number) => void } | null>(null);
  function onkeydown(e: KeyboardEvent) {
    if (app.view !== "mods" || e.ctrlKey || e.metaKey || e.altKey) return;
    if ((e.target as HTMLElement)?.closest("input, textarea, [role=dialog], [role=menu]")) return;
    const i = rows.findIndex((x) => x.id === focusId);
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      const next = Math.min(rows.length - 1, Math.max(0, i + (e.key === "ArrowDown" ? 1 : -1)));
      focusId = rows[next]?.id ?? null;
      list?.scrollToIndex(next);
    } else if (e.key === " " && focused) {
      e.preventDefault();
      tick(focused.id, !ticked.has(focused.id));
    } else if ((e.key === "m" || e.key === "M") && focused) {
      void mods.toggleManaged(focused);
    } else if (e.key === "Delete" && focused) {
      void mods.remove(focused);
    } else if ((e.key === "u" || e.key === "U") && focused) {
      review.updateSelected([focused.id]);
    } else if (e.key === "Escape" && focusId !== null) {
      focusId = null;
    }
  }

  let installOpen = $state(false);
  /**
   * tick · mod · state · in game · size · your version · actions. The list
   * is a size container: narrow, "your version" goes first, then "in game"
   * (the details still show both).
   */
  const COLS =
    "grid items-center gap-x-3 grid-cols-[1.25rem_minmax(0,1fr)_10.5rem_6rem_4.5rem_6.5rem_3.75rem] @max-[820px]:grid-cols-[1.25rem_minmax(0,1fr)_10.5rem_6rem_4.5rem_3.75rem] @max-[620px]:grid-cols-[1.25rem_minmax(0,1fr)_10.5rem_4.5rem_3.75rem]";
  const HIDE_VERSION = "@max-[820px]:hidden";
  const HIDE_IN_GAME = "@max-[620px]:hidden";

  const filterOptions = $derived([
    { value: "all" as const, label: `${$m.filterAll.value} · ${mods.installed.length}` },
    { value: "updates" as const, label: `${$m.filterUpdates.value} · ${mods.stale.length}` },
    { value: "unmanaged" as const, label: `${$m.filterUnmanaged.value} · ${unmanaged}` },
  ]);
</script>

<svelte:window {onkeydown} />

{#snippet sortHead(c: Col, label: string, klass = "")}
  <SortHead {label} active={col === c} {asc} class={klass} onclick={() => sortBy(c)} />
{/snippet}

{#snippet listPane()}
  <div class="@container flex min-h-0 flex-1 flex-col">
    {#if !mods.loaded && mods.installed.length === 0}
      <div class="grid flex-1 place-items-center">
        <Spinner class="size-6" label={$m.loading.value} />
      </div>
    {:else if mods.installed.length === 0}
      <Empty icon={Puzzle} title={$m.noMods.value}>
        {$m.noModsHint.value}
        {#snippet action()}
          <Button variant="accent" onclick={() => (installOpen = true)}
            ><Download class="size-icon-sm" />{$m.install.value}</Button
          >
        {/snippet}
      </Empty>
    {:else}
      <!-- A sized box with the scroller pinned inside: rows are only
           mounted while on screen. -->
      <div class="relative min-h-0 flex-1">
        <VirtualList
          bind:this={list}
          class="absolute inset-0"
          items={rows}
          rowHeight={48}
          key={(x) => x.id}
          aria-label={$m.title.value}
        >
          {#snippet header()}
            <TableHead grid="{COLS} !px-pad">
              <Checkbox
                checked={allTicked}
                indeterminate={someTicked}
                onchange={(v) => tickAll(v)}
                aria-label={allTicked ? $m.deselectAll.value : $m.selectAll.value}
              />
              {@render sortHead("name", $m.colName.value)}
              {@render sortHead("status", $m.colStatus.value)}
              {@render sortHead("managed", $m.colInGame.value, HIDE_IN_GAME)}
              {@render sortHead("size", $m.colSize.value, "justify-self-end")}
              {@render sortHead("local", $m.colDownloaded.value, HIDE_VERSION)}
              <span></span>
            </TableHead>
          {/snippet}
          {#snippet row(x: InstalledModDto)}
            {@const on = focusId === x.id}
            {@const steam = x.source === "steam"}
            <!-- svelte-ignore a11y_click_events_have_key_events -->
            <div
              role="row"
              tabindex="-1"
              class={cn(
                `group ${COLS} h-full cursor-pointer border-b border-border/40 px-pad text-xs`,
                on ? "bg-accent/10" : ticked.has(x.id) ? "bg-raised/50" : "hover:bg-raised/40",
              )}
              onclick={() => (focusId = x.id)}
            >
              <Checkbox
                checked={ticked.has(x.id)}
                onchange={(v) => tick(x.id, v)}
                aria-label={x.name}
              />
              <div class="flex min-w-0 flex-col gap-0.5">
                <span class="truncate font-medium text-fg">{x.name}</span>
                <span
                  class="flex min-w-0 items-center gap-1 text-2xs text-fg-faint"
                  title={steam ? $m.sourceSteamHint.value : $m.sourceLauncherHint.value}
                >
                  {#if steam}<SteamIcon class="size-2.5 shrink-0" />{:else}<HardDrive
                      class="size-2.5 shrink-0"
                    />{/if}
                  <span class="truncate"
                    >{steam ? $m.whereSteam.value : $m.whereLauncher.value}</span
                  >
                </span>
              </div>
              <ModState mod={x} behind />
              <span
                class={cn(
                  "flex min-w-0 items-center gap-1",
                  HIDE_IN_GAME,
                  x.managed ? "text-fg-muted" : "text-fg-faint",
                )}
                title={x.managed ? $m.inGameLinkedTitle.value : $m.inGameNotLinkedTitle.value}
              >
                {#if x.managed}<Link class="size-3 shrink-0" />{:else}<Unlink
                    class="size-3 shrink-0"
                  />{/if}
                <span class="truncate"
                  >{x.managed ? $m.inGameLinked.value : $m.inGameNotLinked.value}</span
                >
              </span>
              <span class="num text-right font-mono text-2xs text-fg-muted">{x.size_human}</span>
              <span
                class={cn("truncate font-mono text-2xs text-fg-muted", HIDE_VERSION)}
                title={x.remote_updated
                  ? `${$m.workshopCopy.value}: ${date(x.remote_updated * 1000)}`
                  : undefined}>{date(x.local_updated * 1000)}</span
              >
              <span class="flex items-center justify-end gap-0.5">
                {#if x.update_available && !mods.opState(x.id)}
                  <IconButton
                    size="icon-xs"
                    icon={RefreshCw}
                    label={$m.update.value}
                    kbd="U"
                    variant="accent"
                    onclick={(e) => {
                      e.stopPropagation();
                      review.updateSelected([x.id]);
                    }}
                  />
                {/if}
                <ModMenu mod={x} />
              </span>
            </div>
          {/snippet}
          {#snippet empty()}
            <Empty icon={SearchX} title={$m.noMatch.value}>
              {#snippet action()}
                <Button
                  onclick={() => {
                    query = "";
                    filter = "all";
                  }}>{$m.clearFilters.value}</Button
                >
              {/snippet}
            </Empty>
          {/snippet}
        </VirtualList>
      </div>
    {/if}

    {#if ticked.size > 0}
      <BulkBar
        label={ticked.size === 1
          ? $m.selectedOne({ count: 1 }).value
          : $m.selected({ count: ticked.size }).value}
        info={$m.selectedSize({ size: bytes(tickedSize) }).value}
      >
        {#snippet lead()}
          {#if mods.stale.length > 0 && tickedStale < mods.stale.length}
            <Button
              size="xs"
              variant="ghost"
              title={$m.selectAllUpdatesTitle.value}
              onclick={() => (ticked = new Set(mods.stale.map((x) => x.id)))}
            >
              {$m.selectAllUpdates.value}
            </Button>
          {/if}
        {/snippet}
        {#if tickedStale > 0}
          <Button
            size="xs"
            variant="accent"
            title={$m.updateStaleTitle({ count: tickedStale }).value}
            onclick={() =>
              review.updateSelected(tickedMods.filter((x) => x.update_available).map((x) => x.id))}
          >
            <RefreshCw class="size-3" />{$m.updateStale({ count: tickedStale }).value}
          </Button>
        {/if}
        <Button
          size="xs"
          title={$m.revalidateTitle({ count: ticked.size }).value}
          onclick={() => review.updateSelected([...ticked])}
        >
          <RotateCw class="size-3" />{$m.revalidateCount({ count: ticked.size }).value}
        </Button>
        {#if tickedLinked < ticked.size}
          <Button
            size="xs"
            title={$m.linkTitle({ count: ticked.size - tickedLinked }).value}
            onclick={() => linkTicked(true)}
          >
            <Link class="size-3" />{$m.link({ count: ticked.size - tickedLinked }).value}
          </Button>
        {/if}
        {#if tickedLinked > 0}
          <Button
            size="xs"
            title={$m.unlinkTitle({ count: tickedLinked }).value}
            onclick={() => linkTicked(false)}
          >
            <Unlink class="size-3" />{$m.unlink({ count: tickedLinked }).value}
          </Button>
        {/if}
        <Button
          size="xs"
          variant="danger"
          title={$m.deleteTitle({ count: ticked.size }).value}
          onclick={() => mods.removeMany([...ticked])}
        >
          <Trash class="size-3" />{$m.deleteCount({ count: ticked.size }).value}
        </Button>
        <IconButton
          size="icon-xs"
          icon={X}
          label={$m.clearSelectionTitle.value}
          onclick={() => (ticked = new Set())}
        />
      </BulkBar>
    {/if}
  </div>
{/snippet}

{#snippet detailPane()}
  {#if focused}
    <div class="flex min-h-0 flex-1 flex-col border-l border-border">
      <ModDetail mod={focused} onclose={() => (focusId = null)} />
    </div>
  {/if}
{/snippet}

<div class="flex min-h-0 flex-1 flex-col">
  <PageHeader title={$m.title.value}>
    {#snippet stats()}
      {#if mods.installed.length > 0}
        <Figure label={$m.statInstalled.value} value={String(mods.installed.length)} />
        <Figure label={$m.statSize.value} value={bytes(mods.totalSize)} />
        {#if hasKey}
          <Figure
            label={$m.filterUpdates.value}
            value={String(mods.stale.length)}
            tone={mods.stale.length ? "text-warn" : "text-ok"}
            title={mods.stale.length
              ? $m.toDownload({ count: mods.stale.length }).value
              : $m.upToDate.value}
          />
          <Figure
            label={$m.statChecked.value}
            value={mods.checking
              ? $m.checking.value
              : mods.lastChecked
                ? relative(Math.floor(mods.lastChecked / 1000))
                : $m.neverChecked.value}
            secondary
          />
        {/if}
      {/if}
    {/snippet}
    {#snippet actions()}
      {#if mods.stale.length > 0}
        <Button
          variant="accent"
          onclick={() => review.updateStale()}
          title={$m.updateCountTitle({ count: mods.stale.length }).value}
        >
          <RefreshCw class="size-icon-sm" />{$m.updateCount({ count: mods.stale.length }).value}
        </Button>
      {/if}
      {#if hasKey && mods.installed.length > 0}
        <Tooltip text={$m.checkUpdatesTitle.value} side="bottom">
          <Button
            variant={mods.stale.length > 0 ? "ghost" : "default"}
            onclick={() => mods.checkUpdates(true)}
            disabled={mods.checking}
          >
            {#if mods.checking}<Spinner class="size-icon-sm" />{:else}<CloudDownload
                class="size-icon-sm"
              />{/if}{$m.checkUpdates.value}
          </Button>
        </Tooltip>
      {/if}
      <Button onclick={() => (installOpen = true)} title={$m.installTitle.value}>
        <Download class="size-icon-sm" />{$m.install.value}
      </Button>
      <Menu.Root>
        <Tooltip text={$m.moreActions.value} side="bottom">
          <Menu.Trigger
            class="grid size-control place-items-center rounded-md text-fg-muted hover:bg-raised hover:text-fg"
            aria-label={$m.moreActions.value}><Ellipsis class="size-icon" /></Menu.Trigger
          >
        </Tooltip>
        <DropdownMenuContent>
          <DropdownMenuItem icon={RotateCw} onselect={() => mods.refresh()} disabled={mods.loading}>
            {$m.rescan.value}
          </DropdownMenuItem>
          <DropdownMenuItem
            icon={RefreshCw}
            onselect={() => review.updateAll()}
            disabled={mods.installed.length === 0}
          >
            {$m.redownloadAll.value}
          </DropdownMenuItem>
          <DropdownMenuItem icon={FolderOpen} onselect={() => mods.openWorkshopDir()}>
            {$m.openFolderTitle.value}
          </DropdownMenuItem>
          <DropdownMenuSeparator />
          <DropdownMenuItem icon={Brush} tone="danger" onselect={() => mods.cleanup()}>
            {$m.cleanup.value}
          </DropdownMenuItem>
        </DropdownMenuContent>
      </Menu.Root>
    {/snippet}
    {#if mods.installed.length > 0}
      <div class="flex items-center gap-2">
        <Input
          type="search"
          class="w-72 max-w-full"
          placeholder={$m.searchPlaceholder.value}
          bind:value={query}
        />
        <Segmented bind:value={filter} options={filterOptions} aria-label={$m.filterLabel.value} />
        <span class="ml-auto font-mono text-2xs text-fg-faint"
          >{rows.length} / {mods.installed.length}</span
        >
      </div>
    {/if}
  </PageHeader>

  {#if !hasKey && mods.installed.length > 0}
    <Alert tone="info" icon={KeyRound} title={$m.apiKeyMissingTitle.value} banner>
      {$m.apiKeyMissingBody.value}
      {#snippet actions()}
        <Button size="xs" onclick={() => app.go("settings", "steam")}
          >{$m.openSettings.value}</Button
        >
      {/snippet}
    </Alert>
  {/if}

  <MasterDetail
    id="mods-detail"
    selected={focused !== null}
    initial={360}
    min={300}
    max={520}
    breakpoint={900}
    main={listPane}
    detail={detailPane}
  />
</div>

<ReviewDialog />
<InstallDialog bind:open={installOpen} />
