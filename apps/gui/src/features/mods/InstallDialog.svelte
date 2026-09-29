<script lang="ts">
  import { dict } from "$lib/i18n";
  import Download from "~icons/lucide/download";
  import { Dialog } from "$lib/components/ui/dialog";
  import { Button } from "$lib/components/ui/button";
  import { Tag } from "$lib/components/ui/tag";
  import { mods } from "$lib/stores/mods.svelte";
  import { parseWorkshopIds, review } from "./review.svelte";

  /** Workshop mods by id or link, pasted in, then reviewed like any update. */
  let { open = $bindable(false) }: { open?: boolean } = $props();
  const m = dict("mods");
  let text = $state("");
  const ids = $derived(parseWorkshopIds(text));
  $effect(() => {
    if (open) text = "";
  });

  function submit() {
    if (ids.length === 0) return;
    open = false;
    review.install(ids);
  }
</script>

<Dialog
  bind:open
  title={$m.installModalTitle.value}
  description={$m.installModalDesc.value}
  size="md"
  closeLabel={$m.close.value}
>
  <label class="block">
    <span class="flex items-baseline justify-between text-2xs">
      <span class="font-medium text-fg">{$m.workshopIdsLabel.value}</span>
      <span class="text-fg-faint">{$m.workshopIdsHint.value}</span>
    </span>
    <textarea
      bind:value={text}
      rows="6"
      class="mt-1 w-full resize-y rounded-sm border border-border bg-bg p-2 font-mono text-2xs text-fg placeholder:text-fg-faint focus:border-accent/60 focus:outline-none"
      placeholder={"1559212036\nhttps://steamcommunity.com/sharedfiles/filedetails/?id=1564026768"}
    ></textarea>
  </label>
  <div class="mt-1.5 flex min-h-6 flex-wrap items-center gap-1">
    {#if text.trim() && ids.length === 0}
      <span class="text-2xs text-err">{$m.noValidDetected.value}</span>
    {:else if ids.length > 0}
      <span class="mr-1 text-2xs text-fg-muted"
        >{$m.installParsed({ count: ids.length }).value}</span
      >
      {#each ids.slice(0, 12) as id (id)}
        <Tag tone={mods.byId.has(id) ? "neutral" : "accent"} title={mods.byId.get(id)?.name ?? ""}>
          <span class="font-mono">{id}</span>
        </Tag>
      {/each}
      {#if ids.length > 12}<span class="text-2xs text-fg-faint">+{ids.length - 12}</span>{/if}
    {/if}
  </div>
  {#snippet footer()}
    <Button variant="ghost" onclick={() => (open = false)}>{$m.cancel.value}</Button>
    <Button variant="accent" disabled={ids.length === 0} onclick={submit}>
      <Download class="size-icon-sm" />
      {ids.length > 1 ? $m.installMods({ count: ids.length }).value : $m.installMod.value}
    </Button>
  {/snippet}
</Dialog>
