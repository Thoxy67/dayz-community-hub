<script lang="ts">
  import { dict } from "$lib/i18n";
  import ExternalLink from "~icons/lucide/external-link";
  import Info from "~icons/lucide/info";
  import TriangleAlert from "~icons/lucide/triangle-alert";
  import Download from "~icons/lucide/download";
  import RefreshCw from "~icons/lucide/refresh-cw";
  import { Dialog } from "$lib/components/ui/dialog";
  import { Button } from "$lib/components/ui/button";
  import { Tag } from "$lib/components/ui/tag";
  import { openUrl } from "$lib/ipc/native";
  import { bytes, date } from "$lib/format";
  import { profile } from "$lib/stores/profile.svelte";
  import { cn } from "$lib/cx";
  import { review, workshopUrl } from "./review.svelte";

  /** What an update or install will touch, before SteamCMD is started. */
  const m = dict("mods");
  const p = $derived(review.pending);
  const install = $derived(p?.kind === "install");
  const count = $derived(p?.mods.length ?? 0);
  const total = $derived(p?.mods.reduce((a, x) => a + (x.size ?? 0), 0) ?? 0);
  const behind = (l?: number, r?: number | null) =>
    l && r && r > l ? Math.floor((r - l) / 86400) : 0;
</script>

<Dialog
  bind:open={() => p !== null, (v) => !v && (review.pending = null)}
  title={install ? $m.confirmTitleInstall({ count }).value : $m.confirmTitleUpdate({ count }).value}
  description={p?.kind === "update_all" ? $m.reviewUpdateAll.value : ""}
  size="md"
  closeLabel={$m.confirmCancel.value}
>
  {#if p}
    <div class="mb-2 flex items-center gap-2 text-2xs text-fg-muted">
      <Tag
        >{total > 0 ? $m.reviewSize({ size: bytes(total) }).value : $m.reviewUnknownSize.value}</Tag
      >
    </div>
    <ul
      class="m-0 max-h-80 list-none divide-y divide-border/60 overflow-y-auto rounded-sm border border-border bg-bg p-0"
    >
      {#each p.mods as mod (mod.id)}
        {@const days = behind(mod.local_updated, mod.remote_updated)}
        <li class="flex items-center gap-2 px-2 py-1.5">
          <div class="min-w-0 flex-1">
            <p class="m-0 truncate text-xs font-medium text-fg">{mod.name}</p>
            <p class="m-0 flex flex-wrap gap-x-3 font-mono text-3xs text-fg-faint">
              <span>#{mod.id}</span>
              {#if mod.local_updated}<span
                  >{$m.confirmLocalDate.value} {date(mod.local_updated * 1000)}</span
                >{/if}
              {#if mod.remote_updated}<span
                  >{$m.confirmRemoteDate.value} {date(mod.remote_updated * 1000)}</span
                >{/if}
              {#if mod.size}<span>{bytes(mod.size)}</span>{/if}
            </p>
          </div>
          {#if install && mod.installed}<Tag>{$m.installAlready.value}</Tag>{/if}
          {#if days > 0}<Tag tone="warn">{$m.confirmNewer({ days }).value}</Tag>{/if}
          <button
            class="grid size-control-sm shrink-0 place-items-center rounded-sm text-fg-faint hover:bg-raised hover:text-fg"
            aria-label={$m.confirmOpenWorkshop.value}
            title={$m.confirmOpenWorkshop.value}
            onclick={() => openUrl(workshopUrl(mod.id))}
            ><ExternalLink class="size-icon-sm" /></button
          >
        </li>
      {/each}
    </ul>
    <p
      class={cn(
        "m-0 mt-2.5 flex items-start gap-1.5 text-2xs",
        profile.viaSteam ? "text-fg-faint" : "text-warn",
      )}
    >
      {#if profile.viaSteam}<Info class="size-3.5 shrink-0" />{:else}<TriangleAlert
          class="size-3.5 shrink-0"
        />{/if}{profile.viaSteam ? $m.confirmWarningSteam.value : $m.confirmWarning.value}
    </p>
  {/if}
  {#snippet footer()}
    <Button variant="ghost" onclick={() => (review.pending = null)}>{$m.confirmCancel.value}</Button
    >
    <Button variant="accent" onclick={() => review.confirm()}>
      {#if install}<Download class="size-icon-sm" />{:else}<RefreshCw class="size-icon-sm" />{/if}
      {install ? $m.confirmInstall({ count }).value : $m.confirmUpdate({ count }).value}
    </Button>
  {/snippet}
</Dialog>
