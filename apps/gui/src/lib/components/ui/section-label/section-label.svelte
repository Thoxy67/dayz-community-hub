<script lang="ts">
  import type { Component, Snippet } from "svelte";
  import { cn } from "$lib/cx";

  /**
   * The name of a cluster of rows.
   *
   * Mono, small, tracked and upper case, behind the accent tick that marks
   * every heading in the app. It was a class in the stylesheet that two
   * places used and eleven wrote out again by hand, at a slightly different
   * tracking, three of them in a different ink.
   *
   * `actions` sits at the far end of the same line: the "all / none" of a
   * filter group, the count of what is under the heading.
   */
  let {
    icon,
    tick = true,
    as = "h3",
    class: klass = "",
    actions,
    children,
  }: {
    icon?: Component<{ class?: string }>;
    /** The accent tick. Off inside something that already carries one. */
    tick?: boolean;
    as?: "h2" | "h3" | "h4" | "p";
    class?: string;
    actions?: Snippet;
    children: Snippet;
  } = $props();
</script>

<div class={cn("flex min-w-0 items-center gap-1.5", klass)}>
  {#if tick}<span class="h-2.5 w-0.5 shrink-0 rounded-full bg-accent/60"></span>{/if}
  {#if icon}
    {@const Icon = icon}
    <Icon class="size-icon-sm shrink-0 text-fg-faint" />
  {/if}
  <svelte:element
    this={as}
    class="m-0 min-w-0 flex-1 truncate font-mono text-2xs font-normal tracking-[0.08em] text-fg-muted uppercase"
  >
    {@render children()}
  </svelte:element>
  {#if actions}<div class="flex shrink-0 items-center gap-1">{@render actions()}</div>{/if}
</div>
