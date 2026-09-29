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
  import ExternalLink from "~icons/lucide/external-link";
  import Trash from "~icons/lucide/trash-2";
  import KeyRound from "~icons/lucide/key-round";
  import Link from "~icons/lucide/link";
  import Unlink from "~icons/lucide/unlink";
  import X from "~icons/lucide/x";
  import SearchX from "~icons/lucide/search-x";
  import { PageHeader, Figure, SortHead, TableHead, Empty, BulkBar } from "$lib/components/app";
  import { Button, IconButton } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import { Segmented } from "$lib/components/ui/segmented";
  import { Checkbox } from "$lib/components/ui/checkbox";
  import { Copy } from "$lib/components/ui/copy";
  import { Tag } from "$lib/components/ui/tag";
  import { Split } from "$lib/components/ui/split";
  import { VirtualList } from "$lib/components/ui/virtual-list";
  import { Alert } from "$lib/components/ui/alert";
  import { Spinner } from "$lib/components/ui/spinner";
  import { MiniSwitch } from "$lib/components/ui/switch";
  import { Tooltip } from "$lib/components/ui/tooltip";
  import {
    DropdownMenuContent,
    DropdownMenuItem,
    DropdownMenuSeparator,
  } from "$lib/components/ui/dropdown-menu";
  import { cn } from "$lib/cx";
  import { bytes, date, relative } from "$lib/format";
  import { openUrl } from "$lib/ipc/native";
  import type { InstalledModDto } from "$lib/ipc/types";
  import { app } from "$lib/stores/app.svelte";
  import { mods } from "$lib/stores/mods.svelte";
  import { profile } from "$lib/stores/profile.svelte";
  import ModDetail from "./ModDetail.svelte";
  import ReviewDialog from "./ReviewDialog.svelte";
  import InstallDialog from "./InstallDialog.svelte";
  import { review, workshopUrl } from "./review.svelte";

  /**
   * Installed Workshop mods: how much room they take, which are behind the
   * Workshop, which are linked into the game, and every operation on them,
   * one at a time or in bulk.
   */
  const m = dict("mods");

  $effect(() => {
    if (mods.installed.length === 0 && !mods.loading) void mods.load();
    void mods.checkUpdates();
  });

  const hasKey = $derived(!!profile.data?.steam_api_key);

  // ── filter and sort ─────────────────────────────────────────────────────
  type Filter = "all" | "updates" | "unmanaged";
  type Col = "name" | "id" | "size" | "local" | "remote" | "status";
  let filter = $state<Filter>("all");
  let query = $state("");
  let col = $state<Col>("name");
  let asc = $state(true);

  const unmanaged = $derived(mods.installed.filter((x) => !x.managed).length);
  const largest = $derived(mods.installed.reduce<InstalledModDto | null>((a, x) => (!a || x.size > a.size ? x : a), null));

  const status = (x: InstalledModDto) => (x.update_available ? 0 : !x.managed ? 1 : x.remote_updated ? 2 : 3);
  const rows = $derived.by(() => {
    const q = query.trim().toLowerCase();
    const list = mods.installed.filter(
      (x) =>
        (filter === "all" || (filter === "updates" ? x.update_available : !x.managed)) &&
        (!q || x.name.toLowerCase().includes(q) || String(x.id).includes(q)),
    );
    const dir = asc ? 1 : -1;
    return list.sort((a, b) => {
      switch (col) {
        case "name":
          return dir * a.name.localeCompare(b.name);
        case "id":
          return dir * (a.id - b.id);
        case "size":
          return dir * (a.size - b.size);
        case "local":
          return dir * (a.local_updated - b.local_updated);
        case "remote":
          return dir * ((a.remote_updated ?? 0) - (b.remote_updated ?? 0));
        default:
          return dir * (status(a) - status(b)) || a.name.localeCompare(b.name);
      }
    });
  });

  function sortBy(c: Col) {
    if (col === c) asc = !asc;
    else {
      col = c;
      asc = c === "name" || c === "status";
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
    if ((e.target as HTMLElement)?.closest("input, textarea, [role=dialog]")) return;
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
  // The Workshop id column leaves below 1280 px (the detail pane still shows
  // it): at the window's minimum width the eight columns did not fit and the
  // row actions were cut off.
  const COLS =
    "grid-cols-[1.25rem_minmax(10rem,1fr)_6.5rem_4.5rem_6.5rem_7rem_2.25rem_4.5rem] max-xl:grid-cols-[1.25rem_minmax(8rem,1fr)_4.5rem_6.5rem_7rem_2.25rem_4.5rem]";

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
      <div class="flex min-h-0 flex-1 flex-col">
        {#if mods.loading && mods.installed.length === 0}
          <div class="grid flex-1 place-items-center"><Spinner class="size-6" label={$m.loading.value} /></div>
        {:else if mods.installed.length === 0}
          <Empty icon={Puzzle} title={$m.noMods.value}>
            {$m.noModsHint.value}
            {#snippet action()}
              <Button variant="accent" onclick={() => (installOpen = true)}><Download class="size-icon-sm" />{$m.install.value}</Button>
            {/snippet}
          </Empty>
        {:else}
          <VirtualList bind:this={list} items={rows} rowHeight={40} key={(x) => x.id} aria-label={$m.title.value}>
            {#snippet header()}
              <TableHead grid="grid {COLS} items-center gap-2 !px-pad">
                <Checkbox
                  checked={allTicked}
                  indeterminate={someTicked}
                  onchange={(v) => tickAll(v)}
                  aria-label={allTicked ? $m.deselectAll.value : $m.selectAll.value}
                />
                {@render sortHead("name", $m.colName.value)}
                {@render sortHead("id", $m.colWorkshopId.value, "max-xl:hidden")}
                {@render sortHead("size", $m.colSize.value, "justify-end")}
                {@render sortHead("remote", $m.colRemote.value)}
                {@render sortHead("status", $m.colStatus.value)}
                <span title={$m.colStatusTitle.value}><Link class="size-3" /></span>
                <span class="text-right uppercase">{$m.rowActions.value}</span>
              </TableHead>
            {/snippet}
            {#snippet row(x: InstalledModDto)}
              {@const on = focusId === x.id}
              {@const days =
                x.remote_updated && x.remote_updated > x.local_updated
                  ? Math.floor((x.remote_updated - x.local_updated) / 86400)
                  : 0}
              <!-- svelte-ignore a11y_click_events_have_key_events -->
              <div
                role="row"
                tabindex="-1"
                class={cn(
                  `group grid ${COLS} h-full cursor-pointer items-center gap-2 border-b border-border/40 px-pad text-xs`,
                  on ? "bg-accent/10" : ticked.has(x.id) ? "bg-raised/50" : "hover:bg-raised/40",
                )}
                onclick={() => (focusId = x.id)}
              >
                <Checkbox checked={ticked.has(x.id)} onchange={(v) => tick(x.id, v)} aria-label={x.name} />
                <div class="flex min-w-0 items-center gap-2">
                  {#if x.update_available}<span class="size-1.5 shrink-0 rounded-full bg-warn"></span>{/if}
                  <span class={cn("truncate font-medium", x.managed ? "text-fg" : "text-fg-muted")}>{x.name}</span>
                </div>
                <span class="min-w-0 max-xl:hidden"><Copy text={String(x.id)} title={$m.copyWorkshopId.value} /></span>
                <span class="num text-right font-mono text-2xs text-fg-muted">{x.size_human}</span>
                <span
                  class="flex min-w-0 items-center gap-1 font-mono text-2xs"
                  title={`${$m.colLocal.value}: ${new Date(x.local_updated * 1000).toLocaleString()}`}
                >
                  {#if x.remote_updated}
                    <span class={x.update_available ? "text-warn" : "text-fg-muted"}>{date(x.remote_updated * 1000)}</span>
                  {:else}<span class="text-fg-faint">{date(x.local_updated * 1000)}</span>{/if}
                </span>
                <span>
                  {#if x.update_available}
                    <Tag tone="warn" title={$m.updateClick.value}>{days > 0 ? $m.daysBehind({ days }).value : $m.behindToday.value}</Tag>
                  {:else if x.remote_updated}
                    <Tag tone="ok">{$m.statusOk.value}</Tag>
                  {:else}
                    <Tag>—</Tag>
                  {/if}
                </span>
                <span title={x.managed ? $m.linkedHint.value : $m.unlinkedHint.value}>
                  <MiniSwitch
                    bind:checked={() => x.managed, () => void mods.toggleManaged(x)}
                    aria-label={x.managed ? $m.statusLinked.value : $m.statusUnlinked.value}
                  />
                </span>
                <span class="flex justify-end gap-0.5 opacity-60 group-hover:opacity-100 focus-within:opacity-100">
                  <IconButton
                    size="icon-xs"
                    icon={RefreshCw}
                    label={x.update_available ? $m.updateClick.value : $m.revalidate.value}
                    variant={x.update_available ? "accent" : "ghost"}
                    onclick={(e) => {
                      e.stopPropagation();
                      review.updateSelected([x.id]);
                    }}
                  />
                  <IconButton
                    size="icon-xs"
                    icon={ExternalLink}
                    label={$m.openWorkshop.value}
                    onclick={(e) => {
                      e.stopPropagation();
                      void openUrl(workshopUrl(x.id));
                    }}
                  />
                  <IconButton
                    size="icon-xs"
                    icon={Trash}
                    label={$m.delete.value}
                    class="hover:bg-err/10 hover:text-err"
                    onclick={(e) => {
                      e.stopPropagation();
                      void mods.remove(x);
                    }}
                  />
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
        {/if}

        {#if ticked.size > 0}
          <BulkBar
            label={ticked.size === 1 ? $m.selectedOne({ count: 1 }).value : $m.selected({ count: ticked.size }).value}
            info={`${bytes(tickedSize)} · ${Math.round((ticked.size / Math.max(1, mods.installed.length)) * 100)}%`}
          >
            {#snippet lead()}
              {#if mods.stale.length > 0 && tickedStale < mods.stale.length}
                <Button size="xs" variant="ghost" title={$m.selectAllUpdatesTitle.value} onclick={() => (ticked = new Set(mods.stale.map((x) => x.id)))}>
                  {$m.selectAllUpdates.value}
                </Button>
              {/if}
            {/snippet}
            {#if tickedStale > 0}
              <Button size="xs" variant="accent" title={$m.updateStaleTitle({ count: tickedStale }).value}
                onclick={() => review.updateSelected(tickedMods.filter((x) => x.update_available).map((x) => x.id))}>
                <RefreshCw class="size-3" />{$m.updateStale({ count: tickedStale }).value}
              </Button>
            {/if}
            <Button size="xs" title={$m.revalidateTitle({ count: ticked.size }).value} onclick={() => review.updateSelected([...ticked])}>
              <RotateCw class="size-3" />{$m.revalidateCount({ count: ticked.size }).value}
            </Button>
            {#if tickedLinked < ticked.size}
              <Button size="xs" title={$m.linkTitle({ count: ticked.size - tickedLinked }).value} onclick={() => linkTicked(true)}>
                <Link class="size-3" />{$m.link({ count: ticked.size - tickedLinked }).value}
              </Button>
            {/if}
            {#if tickedLinked > 0}
              <Button size="xs" title={$m.unlinkTitle({ count: tickedLinked }).value} onclick={() => linkTicked(false)}>
                <Unlink class="size-3" />{$m.unlink({ count: tickedLinked }).value}
              </Button>
            {/if}
            <Button size="xs" variant="danger" title={$m.deleteTitle({ count: ticked.size }).value} onclick={() => mods.removeMany([...ticked])}>
              <Trash class="size-3" />{$m.deleteCount({ count: ticked.size }).value}
            </Button>
            <Button size="xs" variant="ghost" title={$m.clearSelectionTitle.value} onclick={() => (ticked = new Set())}>
              <X class="size-3" />{$m.clearSelection.value}
            </Button>
          </BulkBar>
        {/if}
      </div>
{/snippet}

{#snippet detailPane()}
      <div class="flex min-h-0 flex-1 flex-col border-l border-border bg-panel">
        <ModDetail mod={focused} onclose={() => (focusId = null)} />
      </div>
{/snippet}

<div class="flex min-h-0 flex-1 flex-col">
  <PageHeader title={$m.title.value}>
    {#snippet stats()}
      <Figure label={$m.statInstalled.value} value={String(mods.installed.length)} />
      <Figure label={$m.statSize.value} value={bytes(mods.totalSize)} />
      <Figure label={$m.statStale.value} value={String(mods.stale.length)} tone={mods.stale.length ? "text-warn" : "text-ok"} />
      <Figure label={$m.statLinked.value} value={String(mods.installed.length - unmanaged)} tone="text-accent" />
      <Figure label={$m.statUnlinked.value} value={String(unmanaged)} tone={unmanaged ? "text-fg-muted" : "text-fg-faint"} />
      <Figure
        label={$m.statChecked.value}
        value={mods.lastChecked ? relative(Math.floor(mods.lastChecked / 1000)) : $m.neverChecked.value}
      />
      {#if largest}
        <Figure label={$m.statLargest.value} value={bytes(largest.size)} title={largest.name} />
      {/if}
    {/snippet}
    {#snippet actions()}
      {#if hasKey}
        <Button onclick={() => mods.checkUpdates(true)} disabled={mods.checking || mods.installed.length === 0} title={$m.checkUpdatesTitle.value}>
          {#if mods.checking}<Spinner class="size-icon-sm" />{$m.checking.value}{:else}<CloudDownload class="size-icon-sm" />{$m.checkUpdates.value}{/if}
        </Button>
      {/if}
      {#if mods.stale.length > 0}
        <Button variant="accent" onclick={() => review.updateStale()} title={$m.updateCountTitle({ count: mods.stale.length }).value}>
          <RefreshCw class="size-icon-sm" />{$m.updateCount({ count: mods.stale.length }).value}
        </Button>
      {/if}
      <Button onclick={() => (installOpen = true)} title={$m.installTitle.value}>
        <Download class="size-icon-sm" />{$m.install.value}
      </Button>
      <IconButton icon={RotateCw} label={$m.refreshTitle.value} onclick={() => mods.refresh()} disabled={mods.loading} />
      <Menu.Root>
        <Tooltip text={$m.moreActions.value} side="bottom">
          <Menu.Trigger
            class="grid size-control place-items-center rounded-md text-fg-muted hover:bg-raised hover:text-fg"
            aria-label={$m.moreActions.value}><Ellipsis class="size-icon" /></Menu.Trigger
          >
        </Tooltip>
        <DropdownMenuContent>
          <DropdownMenuItem icon={RefreshCw} onselect={() => review.updateAll()} disabled={mods.installed.length === 0}>
            {$m.updateAll.value}
          </DropdownMenuItem>
          <DropdownMenuItem icon={FolderOpen} onselect={() => mods.openWorkshopDir()}>{$m.openFolder.value}</DropdownMenuItem>
          <DropdownMenuSeparator />
          <DropdownMenuItem icon={Brush} tone="danger" onselect={() => mods.cleanup()}>{$m.cleanup.value}</DropdownMenuItem>
        </DropdownMenuContent>
      </Menu.Root>
    {/snippet}
    <div class="flex items-center gap-2">
      <Input type="search" class="w-72" placeholder={$m.searchPlaceholder.value} bind:value={query} />
      <Segmented bind:value={filter} options={filterOptions} aria-label={$m.filterLabel.value} />
      <span class="ml-auto font-mono text-2xs text-fg-faint">{rows.length} / {mods.installed.length}</span>
    </div>
  </PageHeader>


  {#if !hasKey && mods.installed.length > 0}
    <Alert tone="info" icon={KeyRound} title={$m.apiKeyMissingTitle.value} banner>
      {$m.apiKeyMissingBody.value}
      {#snippet actions()}
        <Button size="xs" onclick={() => app.go("settings", "steam")}>{$m.openSettings.value}</Button>
      {/snippet}
    </Alert>
  {/if}

  <!-- The detail pane only when there is a mod to detail: an empty one took
       half the list's height at the window's minimum width. -->
  <div class="flex min-h-0 flex-1 flex-col">
    {#if focused}
      <Split id="mods-detail" initial={340} min={260} max={520} keep={520} main={listPane} aside={detailPane} />
    {:else}
      {@render listPane()}
    {/if}
  </div>
</div>

<ReviewDialog />
<InstallDialog bind:open={installOpen} />
