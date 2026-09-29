<script lang="ts">
  import { dict } from "$lib/i18n";
  import { Tag } from "$lib/components/ui/tag";
  import { Spinner } from "$lib/components/ui/spinner";
  import { Meter } from "$lib/components/ui/meter";
  import type { InstalledModDto } from "$lib/ipc/types";
  import { mods } from "$lib/stores/mods.svelte";
  import { latestProgress } from "./steamcmd-log";

  /**
   * Where one mod stands, in words, first match wins: the launcher is
   * downloading it or has it waiting in the running operation; Steam is
   * downloading it or has it waiting; it is behind the Workshop; up to
   * date; not checked. `id` alone (no `mod`) is an item Steam is fetching
   * that is not on disk yet. `behind` adds a second line: how far behind,
   * or the download's bar.
   */
  let {
    id,
    mod = null,
    behind = false,
  }: {
    id: number;
    mod?: Pick<InstalledModDto, "update_available" | "remote_updated" | "local_updated"> | null;
    behind?: boolean;
  } = $props();
  const m = dict("mods");

  const op = $derived(mods.opState(id));
  const steam = $derived(mods.steamById.get(id) ?? null);
  const steamWaiting = $derived(
    !!steam && !steam.downloading && (steam.pending || (steam.subscribed && steam.needs_update)),
  );
  /** The launcher's download of this mod, from the operation's log. */
  const opShare = $derived.by(() => {
    if (op !== "downloading") return null;
    const p = latestProgress(mods.op.log, mods.op.logAt);
    return p ? p.percent / 100 : null;
  });
  const steamShare = $derived(
    steam?.downloading && steam.bytes_total > 0 ? steam.bytes_done / steam.bytes_total : null,
  );
  const days = $derived(
    mod?.remote_updated && mod.remote_updated > mod.local_updated
      ? Math.floor((mod.remote_updated - mod.local_updated) / 86400)
      : 0,
  );
  const pct = (f: number | null) => (f === null ? "" : ` ${Math.floor(f * 100)}%`);
</script>

{#snippet bar(share: number | null)}
  {#if behind}
    <Meter
      class="w-24"
      size="xs"
      value={share ?? 0}
      max={1}
      animate={share !== null}
      label={$m.stateDownloading.value}
    />
  {/if}
{/snippet}

<span class="flex min-w-0 flex-col items-start gap-1">
  {#if op === "downloading"}
    <Tag tone="accent"
      ><Spinner class="size-2.5 text-current" />{$m.stateDownloading.value}{pct(opShare)}</Tag
    >
    {@render bar(opShare)}
  {:else if op === "queued"}
    <Tag>{$m.stateQueued.value}</Tag>
  {:else if steam?.downloading}
    <Tag tone="accent" title={$m.steamDownloadsTitle.value}
      ><Spinner class="size-2.5 text-current" />{$m.steamDownloading.value}{pct(steamShare)}</Tag
    >
    {@render bar(steamShare)}
  {:else if steamWaiting || (!mod && steam)}
    <Tag title={$m.steamDownloadsTitle.value}>{$m.steamQueued.value}</Tag>
  {:else if mod?.update_available}
    <Tag tone="warn">{$m.updateAvailable.value}</Tag>
    {#if behind}
      <span class="truncate font-mono text-2xs text-warn/80"
        >{days > 0 ? $m.daysBehind({ days }).value : $m.behindToday.value}</span
      >
    {/if}
  {:else if mod?.remote_updated}
    <Tag tone="ok">{$m.detailsUpToDate.value}</Tag>
  {:else}
    <Tag>{$m.detailsUnknown.value}</Tag>
  {/if}
</span>
