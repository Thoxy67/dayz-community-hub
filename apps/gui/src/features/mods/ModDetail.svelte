<script lang="ts">
  import { dict } from "$lib/i18n";
  import Puzzle from "~icons/lucide/puzzle";
  import FolderOpen from "~icons/lucide/folder-open";
  import ExternalLink from "~icons/lucide/external-link";
  import RefreshCw from "~icons/lucide/refresh-cw";
  import Trash from "~icons/lucide/trash-2";
  import { Button } from "$lib/components/ui/button";
  import { Copy } from "$lib/components/ui/copy";
  import { Meter } from "$lib/components/ui/meter";
  import { MiniSwitch } from "$lib/components/ui/switch";
  import { Tag } from "$lib/components/ui/tag";
  import { Empty, DetailList, DetailRow } from "$lib/components/app";
  import { bytes, date, relative } from "$lib/format";
  import { openUrl } from "$lib/ipc/native";
  import type { InstalledModDto } from "$lib/ipc/types";
  import { mods } from "$lib/stores/mods.svelte";
  import { review, workshopUrl } from "./review.svelte";

  /** Everything known about one installed mod, and what can be done to it. */
  let { mod }: { mod: InstalledModDto | null } = $props();
  const m = dict("mods");

  const behind = $derived(
    mod?.remote_updated && mod.remote_updated > mod.local_updated
      ? Math.floor((mod.remote_updated - mod.local_updated) / 86400)
      : 0,
  );
  const share = $derived(mod && mods.totalSize > 0 ? mod.size / mods.totalSize : 0);
</script>

{#if !mod}
  <Empty icon={Puzzle}>{$m.detailsEmpty.value}</Empty>
{:else}
  <div class="flex min-h-0 flex-1 flex-col overflow-y-auto">
    <header class="border-b border-border px-pad py-3">
      <h2 class="m-0 text-base leading-tight font-semibold break-words text-fg" data-selectable>{mod.name}</h2>
      <div class="mt-1.5 flex flex-wrap items-center gap-1.5">
        {#if mod.update_available}
          <Tag tone="warn">{$m.updateAvailable.value}{behind > 0 ? ` · ${$m.daysBehind({ days: behind }).value}` : ""}</Tag>
        {:else if mod.remote_updated}
          <Tag tone="ok">{$m.detailsUpToDate.value}</Tag>
        {:else}
          <Tag>{$m.detailsUnknown.value}</Tag>
        {/if}
        <Tag tone={mod.managed ? "accent" : "neutral"}>{mod.managed ? $m.statusLinked.value : $m.statusUnlinked.value}</Tag>
      </div>
    </header>

    <DetailList class="px-pad py-3">
      <DetailRow label={$m.colWorkshopId.value}><Copy text={String(mod.id)} /></DetailRow>
      <DetailRow label={$m.detailsLaunchParam.value}><Copy text={`@${mod.id}`} /></DetailRow>
      <DetailRow label={$m.colSize.value} value={`${mod.size_human} (${bytes(mod.size)})`} />
      <DetailRow label={$m.detailsShare.value}>
        <div class="flex items-center gap-2">
          <Meter class="flex-1" value={share} max={1} tone="muted" label={$m.detailsShare.value} />
          <span class="font-mono text-2xs text-fg-muted">{(share * 100).toFixed(1)}%</span>
        </div>
      </DetailRow>
      <DetailRow label={$m.colLocal.value}>
        <span class="block font-mono text-fg">{date(mod.local_updated * 1000)}</span>
        <span class="block text-2xs text-fg-faint">{relative(mod.local_updated)}</span>
      </DetailRow>
      <DetailRow label={$m.colRemote.value}>
        {#if mod.remote_updated}
          <span class={["block font-mono", mod.update_available ? "text-warn" : "text-fg"]}>{date(mod.remote_updated * 1000)}</span>
          <span class="block text-2xs text-fg-faint">{relative(mod.remote_updated)}</span>
        {:else}<span class="font-mono text-fg-faint">—</span>{/if}
      </DetailRow>
    </DetailList>

    <section class="mx-pad rounded-md border border-border bg-bg p-2.5">
      <div class="flex items-center justify-between gap-2">
        <span class="text-xs font-medium text-fg">{$m.detailsLink.value}</span>
        <MiniSwitch
          bind:checked={() => mod.managed, () => void mods.toggleManaged(mod)}
          aria-label={$m.detailsLink.value}
        />
      </div>
      <p class="m-0 mt-1 text-2xs leading-snug text-fg-faint">{$m.detailsLinkHint.value}</p>
    </section>

    <div class="mt-auto flex flex-wrap gap-1.5 border-t border-border px-pad py-2.5">
      <Button variant={mod.update_available ? "accent" : "default"} onclick={() => review.updateSelected([mod.id])}>
        <RefreshCw class="size-icon-sm" />{mod.update_available ? $m.update.value : $m.revalidate.value}
      </Button>
      <Button onclick={() => mods.openModDir(mod.id)}><FolderOpen class="size-icon-sm" />{$m.openModFolder.value}</Button>
      <Button onclick={() => openUrl(workshopUrl(mod.id))}><ExternalLink class="size-icon-sm" />Workshop</Button>
      <Button variant="danger" onclick={() => mods.remove(mod)}><Trash class="size-icon-sm" />{$m.delete.value}</Button>
    </div>
  </div>
{/if}
