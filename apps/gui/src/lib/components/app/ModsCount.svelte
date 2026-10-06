<script lang="ts">
  import { dict } from "$lib/i18n";
  import Puzzle from "~icons/lucide/puzzle";
  import { cn } from "$lib/cx";

  /**
   * How many mods a server runs, and whether yours are ready for it: green
   * when every one is installed and current, orange with ↑n when n of your
   * copies are behind, red with −n when n are missing. A button when there
   * is somewhere to show the list. `missing` is null while the mods on disk
   * are not known: the count alone, in the mods colour.
   */
  let {
    count,
    missing = null,
    stale = 0,
    onclick,
  }: { count: number; missing?: number | null; stale?: number; onclick?: () => void } = $props();
  const c = dict("servers");

  const tone = $derived(
    missing == null ? "text-mods" : missing > 0 ? "text-err" : stale > 0 ? "text-warn" : "text-ok",
  );
  const tip = $derived.by(() => {
    if (missing == null) return $c.showMods.value;
    const parts: string[] = [];
    if (missing > 0) parts.push($c.modsMissingTip({ missing, count }).value);
    if (stale > 0) parts.push($c.modsStaleTip({ stale }).value);
    if (parts.length === 0) parts.push($c.modsAllReady({ count }).value);
    return parts.join(" · ");
  });
</script>

{#if count > 0}
  <button
    class={cn(
      "flex items-center gap-1 justify-self-start rounded-xs px-1 font-mono text-2xs hover:bg-mods/10 disabled:hover:bg-transparent",
      tone,
    )}
    title={tip}
    aria-label={tip}
    disabled={!onclick}
    onclick={(e) => {
      e.stopPropagation();
      onclick?.();
    }}
  >
    <Puzzle class="size-3" /><span class="num">{count}</span>
    {#if missing != null && missing > 0}
      <span class="num font-semibold">−{missing}</span>
    {:else if stale > 0}
      <span class="num font-semibold">↑{stale}</span>
    {/if}
  </button>
{:else}
  <span class="text-fg-faint/60">—</span>
{/if}
