<script lang="ts">
  import type { Snippet } from "svelte";

  /**
   * The tooltip of a chart: over the mark the pointer or the keyboard is on
   * (`anchor`, its box on screen), below it when there is no room above.
   * Never takes the pointer, so moving across marks does not flicker. The
   * same plate as the app's tooltips.
   */
  let { anchor, children }: { anchor: DOMRect | null; children: Snippet } = $props();

  let el: HTMLDivElement | undefined = $state();
  let size = $state({ w: 0, h: 0 });
  $effect(() => {
    if (!el) return;
    const ro = new ResizeObserver(() => (size = { w: el!.offsetWidth, h: el!.offsetHeight }));
    ro.observe(el);
    return () => ro.disconnect();
  });

  const pos = $derived.by(() => {
    if (!anchor) return null;
    const gap = 6;
    const above = anchor.top - size.h - gap;
    const top = above >= 4 ? above : anchor.bottom + gap;
    const left = Math.min(
      window.innerWidth - size.w - 4,
      Math.max(4, anchor.left + anchor.width / 2 - size.w / 2),
    );
    return { top, left };
  });
</script>

{#if anchor}
  <div
    bind:this={el}
    role="tooltip"
    class="pointer-events-none fixed z-tip flex max-w-80 flex-col gap-0.5 rounded-md border border-border bg-overlay px-2 py-1.5 text-2xs leading-snug text-fg shadow-pop"
    style:top="{pos?.top ?? 0}px"
    style:left="{pos?.left ?? 0}px"
    style:visibility={size.w ? "visible" : "hidden"}
  >
    {@render children()}
  </div>
{/if}
