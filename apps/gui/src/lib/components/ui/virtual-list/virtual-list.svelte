<script lang="ts" generics="T">
  import type { Snippet } from "svelte";
  import { cn } from "$lib/cx";

  /**
   * A long list of which only the rows on screen exist.
   *
   * The entity list rewrote three hundred rows twenty times a second, the
   * traffic panel kept a thousand in the document, and a market tab of five
   * hundred items asked the backend for five hundred pictures the moment it
   * was opened, because a row that is mounted fetches its icon whether or not
   * anybody can see it. A row that is not on screen is not mounted here, so
   * all three go away at once.
   *
   * Rows are one height, which every list in the app already was: that is
   * what lets this be arithmetic instead of measurement. `rowHeight` is in
   * pixels and should be the density's row measure; `rowPx()` reads it.
   */
  let {
    items,
    rowHeight,
    key,
    overscan = 6,
    class: klass = "",
    stickToEnd = false,
    row,
    empty,
    header,
    "aria-label": ariaLabel,
  }: {
    items: readonly T[];
    rowHeight: number;
    key: (item: T, index: number) => string | number;
    /** Rows kept mounted beyond each edge, so a flick does not show a gap. */
    overscan?: number;
    class?: string;
    /** Follow the end while the reader is at it, as a log does. */
    stickToEnd?: boolean;
    row: Snippet<[item: T, index: number]>;
    empty?: Snippet;
    /** Stays at the top while the rows scroll under it. */
    header?: Snippet;
    "aria-label"?: string;
  } = $props();

  let viewport: HTMLDivElement | undefined = $state();
  let scrollTop = $state(0);
  let height = $state(0);
  let atEnd = true;

  const total = $derived(items.length * rowHeight);
  const first = $derived(Math.max(0, Math.floor(scrollTop / rowHeight) - overscan));
  const last = $derived(
    Math.min(items.length, Math.ceil((scrollTop + height) / rowHeight) + overscan),
  );
  const shown = $derived(items.slice(first, last));

  function onscroll() {
    if (!viewport) return;
    scrollTop = viewport.scrollTop;
    atEnd = viewport.scrollTop + viewport.clientHeight >= viewport.scrollHeight - rowHeight;
  }

  $effect(() => {
    if (!viewport) return;
    const el = viewport;
    const watch = new ResizeObserver(() => (height = el.clientHeight));
    watch.observe(el);
    height = el.clientHeight;
    return () => watch.disconnect();
  });

  // After the rows have grown, and only if the reader had not scrolled away:
  // somebody reading a line further up must not have it pulled from under them.
  $effect(() => {
    void total;
    if (stickToEnd && atEnd && viewport) viewport.scrollTop = viewport.scrollHeight;
  });

  /** Put a row in view: the radar does this for the entity under the pointer. */
  export function scrollToIndex(index: number) {
    if (!viewport) return;
    const top = index * rowHeight;
    if (top < viewport.scrollTop) viewport.scrollTop = top;
    else if (top + rowHeight > viewport.scrollTop + viewport.clientHeight) {
      viewport.scrollTop = top + rowHeight - viewport.clientHeight;
    }
  }
</script>

<div
  bind:this={viewport}
  role={ariaLabel ? "list" : undefined}
  aria-label={ariaLabel}
  class={cn("relative min-h-0 flex-1 overflow-y-auto overscroll-contain", klass)}
  {onscroll}
>
  {#if header}<div class="sticky top-0 z-raised">{@render header()}</div>{/if}
  {#if items.length === 0}
    {@render empty?.()}
  {:else}
    <!-- The spacer is as tall as every row would be, so the scrollbar tells
         the truth; the rows themselves are moved down to where they belong
         with a transform, which does not lay anything out. -->
    <div style:height="{total}px" class="relative">
      <div class="absolute inset-x-0 top-0" style:transform="translateY({first * rowHeight}px)">
        {#each shown as item, i (key(item, first + i))}
          <div style:height="{rowHeight}px" role={ariaLabel ? "listitem" : undefined}>
            {@render row(item, first + i)}
          </div>
        {/each}
      </div>
    </div>
  {/if}
</div>
