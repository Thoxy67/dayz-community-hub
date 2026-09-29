<script lang="ts">
  import type { Snippet } from "svelte";
  import { cn } from "$lib/cx";

  /** The sticky heading row of a list: pass the same grid class as the rows. */
  let { grid, children }: { grid: string; children: Snippet } = $props();
</script>

<div
  class={cn(
    grid,
    "h-7 border-b border-border bg-panel/95 px-2 font-display text-2xs font-bold tracking-[0.1em] text-fg-faint",
  )}
>
  {@render children()}
</div>
