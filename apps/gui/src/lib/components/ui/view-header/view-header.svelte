<script lang="ts">
  import type { Component, Snippet } from "svelte";
  import { cn } from "$lib/cx";

  /**
   * The top of a view: what this is, the numbers it is about, what can be
   * done to it.
   *
   * Every data view had its own arrangement of these three and so none of
   * them could be learned once. Name at the start behind its icon, the
   * figures after it, and the actions at the far end, always in the order
   * filter, export, reset, settings.
   */
  let {
    title,
    icon,
    class: klass = "",
    stats,
    actions,
    children,
  }: {
    title: string;
    icon?: Component<{ class?: string }>;
    class?: string;
    /** The figures the view is about: a `StatStrip`, usually. */
    stats?: Snippet;
    actions?: Snippet;
    /** A second row under the first: the view's own filters and search. */
    children?: Snippet;
  } = $props();
</script>

<header class={cn("shrink-0 border-b border-border bg-panel", klass)}>
  <div class="flex min-h-control-lg items-center gap-3 px-pad">
    <h1 class="m-0 flex shrink-0 items-center gap-1.5 text-sm font-semibold text-fg">
      <span class="h-3 w-0.5 rounded-full bg-accent"></span>
      {#if icon}
        {@const Icon = icon}
        <Icon class="size-icon text-accent" />
      {/if}
      {title}
    </h1>
    {#if stats}<div class="min-w-0 flex-1">{@render stats()}</div>{:else}<div
        class="flex-1"
      ></div>{/if}
    {#if actions}<div class="flex shrink-0 items-center gap-1">{@render actions()}</div>{/if}
  </div>
  {#if children}
    <div class="flex flex-wrap items-center gap-1.5 border-t border-border/60 px-pad py-1">
      {@render children()}
    </div>
  {/if}
</header>
