<script lang="ts">
  import type { Snippet } from "svelte";
  import { cn } from "$lib/cx";

  /**
   * The strip at the foot of a list while some of it is ticked: how many,
   * a line about them, and what can be done to all of them at once.
   */
  let {
    label,
    info = "",
    lead,
    children,
    class: klass = "",
  }: {
    /** "3 mods selected". */
    label: string;
    /** A second, quieter figure: their size, their share. */
    info?: string;
    /** Buttons right after the label ("select all with updates"). */
    lead?: Snippet;
    /** The bulk actions, at the far end. */
    children: Snippet;
    class?: string;
  } = $props();
</script>

<div
  class={cn(
    "flex shrink-0 animate-slide-up flex-wrap items-center gap-1.5 border-t border-accent/30 bg-bg px-pad py-1.5",
    klass,
  )}
>
  <span class="h-4 w-0.5 rounded-full bg-accent"></span>
  <span class="text-xs font-medium text-fg">{label}</span>
  {#if info}<span class="font-mono text-2xs text-fg-faint">{info}</span>{/if}
  {@render lead?.()}
  <div class="ml-auto flex flex-wrap items-center gap-1">{@render children()}</div>
</div>
