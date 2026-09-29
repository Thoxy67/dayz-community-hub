<script lang="ts">
  import type { Snippet } from "svelte";
  import { cn } from "$lib/cx";

  /**
   * The top of every view: its name in the display face, the figures it is
   * about, its actions on the right, and its toolbar (search, filters) under.
   * A size container: when the figures do not fit beside the name and the
   * actions, they take a line of their own under them instead of piling up
   * in a column.
   */
  let {
    title,
    stats,
    actions,
    children,
    class: klass = "",
  }: {
    title: string;
    /** `Figure`s, usually. */
    stats?: Snippet;
    actions?: Snippet;
    /** A second row: search, filters. */
    children?: Snippet;
    class?: string;
  } = $props();
</script>

<header class={cn("@container flex shrink-0 flex-col gap-2 border-b border-border bg-bg/40 px-pad pt-2.5 pb-2", klass)}>
  <div
    class="grid min-w-0 grid-cols-[auto_minmax(0,1fr)_auto] items-end gap-x-6 gap-y-2 @max-[760px]:grid-cols-[minmax(0,1fr)_auto]"
  >
    <h1 class="m-0 truncate title-display text-2xl leading-none text-fg">{title}</h1>
    {#if stats}
      <div
        class="flex min-w-0 flex-wrap items-end gap-x-5 gap-y-1 @max-[760px]:col-span-2 @max-[760px]:row-start-2"
      >
        {@render stats()}
      </div>
    {:else}
      <span class="@max-[760px]:hidden"></span>
    {/if}
    {#if actions}
      <div class="flex shrink-0 items-center gap-1 justify-self-end @max-[760px]:col-start-2 @max-[760px]:row-start-1">
        {@render actions()}
      </div>
    {/if}
  </div>
  {#if children}{@render children()}{/if}
</header>
