<script lang="ts">
  import type { Snippet } from "svelte";
  import { cn } from "$lib/cx";

  // Flat instrument panel — no outer border, no radius. Dividers between
  // panels belong to the shell/view grids (border-l/-t), so panels sit flush
  // and there is never a doubled hairline between two of them.
  let {
    title = "",
    toolbar,
    scroll = true,
    padded = false,
    class: klass = "",
    children,
  }: {
    title?: string;
    /** Right-aligned controls in the title strip. Rendered only when there
        is also a title: a toolbar without one would float over a header that
        is not there. */
    toolbar?: Snippet;
    scroll?: boolean;
    padded?: boolean;
    class?: string;
    children: Snippet;
  } = $props();
</script>

<section class={cn("flex min-h-0 min-w-0 flex-col bg-panel", klass)}>
  {#if title}
    <header
      class="flex h-control shrink-0 items-center justify-between gap-2 border-b border-border
             bg-raised/30 pr-1.5 pl-2"
    >
      <!-- The tick marks this header as a header: the same accent the app
           acts in, at quarter strength, so the panel's name sits beside a
           dash of the colour the app does things in. -->
      <h2
        class="flex min-w-0 items-center gap-1.5 font-mono text-2xs tracking-wider text-fg-muted
               uppercase"
      >
        <span class="h-2.5 w-[2px] shrink-0 rounded-full bg-accent/60"></span>
        <!-- One line, cut short in a narrow window: wrapped, it grew past the
             header's fixed height and slid under the toolbar. -->
        <span class="truncate">{title}</span>
      </h2>
      {#if toolbar}<div class="flex items-center gap-1">{@render toolbar()}</div>{/if}
    </header>
  {/if}
  <div class={cn("flex min-h-0 flex-1 flex-col", scroll && "overflow-y-auto", padded && "p-2")}>
    {@render children()}
  </div>
</section>
