<script lang="ts">
  import { cn } from "$lib/cx";
  import Skeleton from "./skeleton.svelte";

  /**
   * A list that is loading: `count` rows the height of the real ones, each a
   * line and a shorter one under it, with a square in front when the rows
   * have a picture (`media`). Widths vary a little from row to row so it
   * reads as a list and not as a pattern.
   */
  let {
    count = 8,
    media = false,
    class: klass = "",
  }: { count?: number; media?: boolean; class?: string } = $props();

  const WIDTHS = ["w-3/5", "w-2/5", "w-1/2", "w-2/3", "w-1/3", "w-3/4"];
</script>

<div class={cn("flex flex-col", klass)} role="status" aria-busy="true">
  {#each { length: count }, i (i)}
    <div class="flex h-row items-center gap-3 border-b border-border/50 px-pad">
      {#if media}<Skeleton class="size-8 shrink-0 rounded-sm" />{/if}
      <div class="flex min-w-0 flex-1 flex-col gap-1.5">
        <Skeleton class={cn("h-2.5", WIDTHS[i % WIDTHS.length])} />
        <Skeleton class={cn("h-2", WIDTHS[(i + 3) % WIDTHS.length], "opacity-60")} />
      </div>
    </div>
  {/each}
</div>
