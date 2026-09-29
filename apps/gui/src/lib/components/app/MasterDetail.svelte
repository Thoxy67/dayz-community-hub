<script lang="ts">
  import type { Snippet } from "svelte";
  import { Split } from "$lib/components/ui/split";
  import { cn } from "$lib/cx";

  /**
   * A list and the details of what is selected in it, as every server view
   * lays them out. Wide: side by side, the details pane resizable and
   * remembered under `id`. Narrow (below `breakpoint` pixels of its own
   * width): the list takes the whole width and the details slide over its
   * right edge while something is selected, instead of the two squeezing
   * each other or stacking.
   */
  let {
    id,
    show = true,
    selected = true,
    initial = 400,
    min = 320,
    max = 620,
    breakpoint = 980,
    main,
    detail,
  }: {
    id: string;
    /** The details pane is wanted at all (the view's toggle). */
    show?: boolean;
    /** Something is selected: the narrow drawer only opens then. */
    selected?: boolean;
    initial?: number;
    min?: number;
    max?: number;
    breakpoint?: number;
    main: Snippet;
    detail: Snippet;
  } = $props();

  let width = $state(0);
  const narrow = $derived(width > 0 && width < breakpoint);
</script>

<div class="relative flex min-h-0 min-w-0 flex-1" bind:clientWidth={width}>
  {#if show && !narrow}
    <Split {id} pane="end" {initial} {min} {max} keep={Math.min(520, breakpoint - initial)} {main} aside={detail} />
  {:else}
    <div class="flex min-h-0 min-w-0 flex-1 flex-col">{@render main()}</div>
    {#if show && selected}
      <div
        class={cn(
          "absolute inset-y-0 right-0 z-raised flex w-[min(24rem,88%)] flex-col",
          "animate-fade-in border-l border-border-strong bg-bg shadow-pop",
        )}
      >
        {@render detail()}
      </div>
    {/if}
  {/if}
</div>
