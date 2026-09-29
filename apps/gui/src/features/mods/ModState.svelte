<script lang="ts">
  import { dict } from "$lib/i18n";
  import { Tag } from "$lib/components/ui/tag";
  import { Spinner } from "$lib/components/ui/spinner";
  import type { InstalledModDto } from "$lib/ipc/types";
  import { mods } from "$lib/stores/mods.svelte";

  /**
   * Where one mod stands, in words: being downloaded, waiting its turn,
   * behind the Workshop (and by how long), up to date, or not checked yet.
   * `behind` adds, under the tag, how far behind the Workshop it is.
   */
  let {
    mod,
    behind = false,
  }: {
    mod: Pick<InstalledModDto, "id" | "update_available" | "remote_updated" | "local_updated">;
    behind?: boolean;
  } = $props();
  const m = dict("mods");

  const op = $derived(mods.opState(mod.id));
  const days = $derived(
    mod.remote_updated && mod.remote_updated > mod.local_updated
      ? Math.floor((mod.remote_updated - mod.local_updated) / 86400)
      : 0,
  );
</script>

<span class="flex min-w-0 flex-col items-start gap-0.5">
  {#if op === "downloading"}
    <Tag tone="accent"><Spinner class="size-2.5 text-current" />{$m.stateDownloading.value}</Tag>
  {:else if op === "queued"}
    <Tag>{$m.stateQueued.value}</Tag>
  {:else if mod.update_available}
    <Tag tone="warn">{$m.updateAvailable.value}</Tag>
  {:else if mod.remote_updated}
    <Tag tone="ok">{$m.detailsUpToDate.value}</Tag>
  {:else}
    <Tag>{$m.detailsUnknown.value}</Tag>
  {/if}
  {#if behind && mod.update_available && !op}
    <span class="truncate font-mono text-2xs text-warn/80"
      >{days > 0 ? $m.daysBehind({ days }).value : $m.behindToday.value}</span
    >
  {/if}
</span>
