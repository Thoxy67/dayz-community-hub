<script lang="ts">
  import type { Snippet } from "svelte";
  import { prefs } from "$lib/stores/prefs.svelte";
  import { Splitter } from "$lib/components/ui/split";

  // A main area and a side pane, with the handle between them.
  //
  // Every view in this app is this shape -- a plot, a feed or a list, with a
  // column of detail beside it -- and before this component they each said so
  // in their own words: one was draggable and saved, and the others were
  // frozen at 20rem, 21rem, 22rem and 24rem, four numbers picked by hand for
  // the same job. Sizing is a property of a pane, not of a view, so it lives
  // in one place and every pane gets the same gesture: drag, arrow keys,
  // double-click or Home to put it back.
  //
  // One pane keeps its size and the other takes what is left. Which one is
  // the point: widening the window should grow the map and the feed, which
  // have no natural width, and leave the column of labels beside them alone,
  // because it does have one. `pane` says which side the sized one sits on --
  // usually the end, but the radar's filters are a fixed set of controls above
  // an unbounded list, and there the sized one is on top.

  let {
    id,
    orientation = "vertical",
    pane = "end",
    initial,
    min = 180,
    max = 640,
    keep = 340,
    label = "Resize",
    asideClass = "",
    main,
    aside,
  }: {
    /** Name this pane is remembered under. */
    id: string;
    orientation?: "vertical" | "horizontal";
    /** Which side the sized pane sits on. */
    pane?: "start" | "end";
    /** Size before anyone has dragged it. */
    initial: number;
    min?: number;
    max?: number;
    /**
     * Pixels the main area may never drop below. It is also what decides when
     * the panes stack: 260 left a strip of figures two tiles wide beside a
     * full side pane in a 480 pixel window, which is side by side in name only.
     */
    keep?: number;
    label?: string;
    asideClass?: string;
    main: Snippet;
    aside: Snippet;
  } = $props();

  // Side by side needs room for both: the sized pane's floor and the main
  // area's. A window snapped to a third of a screen has neither, and two
  // columns of 150 px each show nothing. Below that the same two panes stack,
  // main on top, and the handle between them works the other way. The stacked
  // size is remembered apart from the side-by-side one, since a height and a
  // width are not the same number.
  let box = $state<HTMLElement | null>(null);
  let width = $state(0);
  let height = $state(0);

  const stacked = $derived(orientation === "vertical" && width > 0 && width < min + keep);
  const vertical = $derived(orientation === "vertical" && !stacked);
  const atStart = $derived(pane === "start");
  const available = $derived(vertical ? width : height);

  // What the window can actually spare, watched rather than read once: a size
  // that was fine on a wide monitor must not eat a narrow one, and the app is
  // routinely resized to sit beside a running game.
  $effect(() => {
    if (!box) return;
    const el = box;
    const ro = new ResizeObserver(() => {
      const r = el.getBoundingClientRect();
      width = r.width;
      height = r.height;
    });
    ro.observe(el);
    return () => ro.disconnect();
  });

  const key = $derived(stacked ? `${id}.stacked` : id);
  const floor = $derived(stacked ? 96 : min);
  const roof = $derived(stacked ? Math.max(floor, height - 140) : max);
  const spare = $derived(stacked ? 140 : keep);
  const first = $derived(stacked ? Math.round(height * 0.42) : initial);

  // The ceiling is whichever is smaller: what this pane is allowed, and what
  // is left after the main area keeps its floor. The floor wins over both, so
  // a window too small for either still shows the pane rather than collapsing
  // it to nothing.
  const ceiling = $derived(
    available > 0 ? Math.max(floor, Math.min(roof, available - spare)) : roof,
  );
  const size = $derived(Math.min(ceiling, Math.max(floor, prefs.pane(key, first))));

  // Clamped against the *live* value rather than the saved one, so dragging
  // past the end does not bank a size that springs back the moment the
  // pointer crosses the limit again.
  function drag(delta: number) {
    const next = atStart ? size + delta : size - delta;
    prefs.setPane(key, Math.min(ceiling, Math.max(floor, next)));
  }
</script>

<!-- `*:` makes whatever a pane contains fill it, which is what these panes
     used to get for free from being grid cells. Without it a panel sizes to
     its content and leaves the bottom of its own pane empty. -->
{#snippet flexible()}
  <div class="flex min-h-0 min-w-0 flex-1 flex-col *:min-h-0 *:flex-1">
    {@render main()}
  </div>
{/snippet}

{#snippet sized()}
  <div
    class="flex min-h-0 min-w-0 shrink-0 flex-col *:min-h-0 *:flex-1 {asideClass}"
    style:width={vertical ? `${size}px` : undefined}
    style:height={vertical ? undefined : `${size}px`}
  >
    {@render aside()}
  </div>
{/snippet}

<div bind:this={box} class="flex min-h-0 min-w-0 flex-1 {vertical ? '' : 'flex-col'}">
  {#if atStart}{@render sized()}{:else}{@render flexible()}{/if}

  <Splitter
    orientation={vertical ? "vertical" : "horizontal"}
    ondrag={drag}
    onreset={() => prefs.resetPane(key)}
    value={size}
    min={floor}
    max={ceiling}
    aria-label={label}
  />

  {#if atStart}{@render flexible()}{:else}{@render sized()}{/if}
</div>
