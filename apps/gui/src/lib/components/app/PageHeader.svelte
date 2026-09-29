<script lang="ts">
  import type { Snippet } from "svelte";
  import { cn } from "$lib/cx";

  /**
   * The top of every view: its name in the display face, the figures it is
   * about, its actions on the right, and its toolbar (search, filters) under.
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

<header class={cn("flex shrink-0 flex-col gap-2 border-b border-border bg-bg/40 px-pad pt-2.5 pb-2", klass)}>
  <div class="flex min-w-0 items-end gap-6">
    <h1 class="m-0 shrink-0 title-display text-2xl leading-none text-fg">{title}</h1>
    {#if stats}<div class="flex min-w-0 flex-wrap items-end gap-x-5 gap-y-1">{@render stats()}</div>{/if}
    {#if actions}<div class="ml-auto flex shrink-0 items-center gap-1">{@render actions()}</div>{/if}
  </div>
  {#if children}{@render children()}{/if}
</header>
