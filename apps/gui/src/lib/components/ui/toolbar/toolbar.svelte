<script lang="ts">
  import type { Snippet } from "svelte";
  import { Toolbar } from "bits-ui";
  import { cn } from "$lib/cx";

  // A row of controls that is one tab stop: bits-ui's Toolbar. Tab reaches
  // the bar, the arrows move along it, Home and End jump to its ends. A bar
  // of twenty buttons that each took a Tab was twenty presses to get past.
  //
  // It is a size container, so what is inside can answer to the bar's own
  // width (`@[760px]:inline`) and not the window's: the same bar is wide in a
  // wide window and narrow beside an open sidebar.
  let {
    class: klass = "",
    children,
    "aria-label": ariaLabel,
  }: {
    class?: string;
    children: Snippet;
    "aria-label": string;
  } = $props();
</script>

<Toolbar.Root
  aria-label={ariaLabel}
  class={cn(
    // It wraps. Groups give up their captions and their roomier forms as the
    // bar narrows (see the callers), but that only goes so far, and past it a
    // bar that does not wrap cuts its last controls off at the window's edge:
    // on the radar that was the menu holding everything else that had not
    // fitted. A second row costs a control's height; a control nobody can
    // reach costs the feature.
    "@container flex min-h-control-lg shrink-0 flex-wrap items-center gap-x-2 gap-y-1 border-b border-border bg-bg px-2 py-[3px]",
    klass,
  )}
>
  {@render children()}
</Toolbar.Root>
