/**
 * Shared virtual-list machinery for the server tabs (Servers / History /
 * Favorites). These tabs all render a tall fixed-row-height list inside a
 * scroll container and only mount the visible window of rows.
 *
 * This helper owns the identical bits: the scroll container ref, scrollTop /
 * containerHeight state, a rAF-throttled scroll handler, the ResizeObserver
 * wiring, and the start/end/offset derivations. The list contents (sorting,
 * filtering) stay in each component — only `getCount()` is passed in so the
 * window math can react to length changes.
 *
 * @example
 *   const vl = createVirtualList({ rowHeight: 48, overscan: 5, getCount: () => sorted.length });
 *   // template:
 *   //   <div bind:this={vl.container} onscroll={vl.handleScroll}> ... </div>
 *   //   {#each sorted.slice(vl.startIndex, vl.endIndex) as item} ...
 *   $effect(vl.observe);             // wire ResizeObserver + cleanup
 */
export interface VirtualListOptions {
  rowHeight: number;
  overscan: number;
  /** Reactive accessor for the total number of rows. */
  getCount: () => number;
  /** Sticky header height used by scrollToIndex (default 32). */
  theadHeight?: number;
}

export function createVirtualList(opts: VirtualListOptions) {
  const { rowHeight, overscan, getCount, theadHeight = 32 } = opts;

  let container = $state<HTMLDivElement | undefined>(undefined);
  let scrollTop = $state(0);
  let containerHeight = $state(600);

  // Coalesce scroll-driven state writes to one per frame so the derived window
  // (start/end/visible) can't update more often than the browser can paint.
  let _scrollRaf: number | null = null;
  function handleScroll() {
    if (!container || _scrollRaf !== null) return;
    _scrollRaf = requestAnimationFrame(() => {
      _scrollRaf = null;
      if (container) scrollTop = container.scrollTop;
    });
  }

  const totalHeight = $derived(getCount() * rowHeight);
  const startIndex = $derived(Math.max(0, Math.floor(scrollTop / rowHeight) - overscan));
  const endIndex = $derived(Math.min(getCount(), Math.ceil((scrollTop + containerHeight) / rowHeight) + overscan));
  const offsetY = $derived(startIndex * rowHeight);

  /** Use as `$effect(vl.observe)` — wires ResizeObserver and cancels the rAF. */
  function observe() {
    if (!container) return;
    const ro = new ResizeObserver((entries) => {
      for (const entry of entries) containerHeight = entry.contentRect.height;
    });
    ro.observe(container);
    containerHeight = container.clientHeight;
    return () => {
      ro.disconnect();
      if (_scrollRaf !== null) {
        cancelAnimationFrame(_scrollRaf);
        _scrollRaf = null;
      }
    };
  }

  /** Scroll the row at `idx` into view (accounting for the sticky header). */
  function scrollToIndex(idx: number) {
    if (!container) return;
    const rowTop = idx * rowHeight;
    const rowBot = rowTop + rowHeight;
    if (rowTop < container.scrollTop + theadHeight) {
      container.scrollTop = rowTop - theadHeight;
    } else if (rowBot > container.scrollTop + containerHeight) {
      container.scrollTop = rowBot - containerHeight;
    }
  }

  function scrollToTop() {
    if (container) container.scrollTo({ top: 0, behavior: 'smooth' });
    scrollTop = 0;
  }

  return {
    get container() {
      return container;
    },
    set container(el: HTMLDivElement | undefined) {
      container = el;
    },
    get scrollTop() {
      return scrollTop;
    },
    set scrollTop(v: number) {
      scrollTop = v;
    },
    get containerHeight() {
      return containerHeight;
    },
    get totalHeight() {
      return totalHeight;
    },
    get startIndex() {
      return startIndex;
    },
    get endIndex() {
      return endIndex;
    },
    get offsetY() {
      return offsetY;
    },
    rowHeight,
    handleScroll,
    observe,
    scrollToIndex,
    scrollToTop,
  };
}
