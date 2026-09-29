<script lang="ts">
  // A drag handle between two panes. 1px visually, but with a padded hit area:
  // a hairline you cannot grab is a hairline nobody resizes.
  let {
    orientation = "vertical",
    ondrag,
    onreset,
    value,
    min,
    max,
    "aria-label": label = "Resize",
  }: {
    orientation?: "vertical" | "horizontal";
    /** Delta in pixels along the axis of movement. */
    ondrag: (delta: number) => void;
    /** Back to the view's own default. Double-click, or Home. */
    onreset?: () => void;
    /** Current size of the pane this handle sizes, for assistive tech. */
    value?: number;
    min?: number;
    max?: number;
    "aria-label"?: string;
  } = $props();

  const vertical = $derived(orientation === "vertical");

  function start(e: PointerEvent) {
    e.preventDefault();
    const target = e.currentTarget as HTMLElement;
    target.setPointerCapture(e.pointerId);
    let last = vertical ? e.clientX : e.clientY;

    const move = (ev: PointerEvent) => {
      const now = vertical ? ev.clientX : ev.clientY;
      ondrag(now - last);
      last = now;
    };
    const end = (ev: PointerEvent) => {
      target.releasePointerCapture(ev.pointerId);
      target.removeEventListener("pointermove", move);
      target.removeEventListener("pointerup", end);
    };
    target.addEventListener("pointermove", move);
    target.addEventListener("pointerup", end);
  }

  // Keyboard resizing, because a splitter that only responds to a pointer is
  // unreachable for anyone who cannot use one.
  function key(e: KeyboardEvent) {
    const step = e.shiftKey ? 32 : 8;
    const back = vertical ? "ArrowLeft" : "ArrowUp";
    const fwd = vertical ? "ArrowRight" : "ArrowDown";
    if (e.key === back) ondrag(-step);
    else if (e.key === fwd) ondrag(step);
    // The same escape as the double-click, for anyone driving from the
    // keyboard: a pane dragged somewhere useless should not have to be dragged
    // back by eye.
    else if (e.key === "Home") onreset?.();
    else return;
    e.preventDefault();
  }
</script>

<!-- `separator` is the ARIA role for a window splitter, and a *focusable*
     separator is precisely how the pattern specifies a resizable one. Svelte's
     a11y rules treat every separator as static, so the two warnings below are
     the lint being wrong about this case rather than the markup being wrong.
     Removing the tabindex would make the panes keyboard-unresizable, which is
     the actual accessibility failure. -->
<!-- svelte-ignore a11y_no_noninteractive_tabindex -->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<div
  role="separator"
  aria-label={label}
  aria-orientation={orientation}
  aria-valuenow={value}
  aria-valuemin={min}
  aria-valuemax={max}
  tabindex="0"
  class="group relative z-10 shrink-0 {vertical
    ? 'w-1 cursor-col-resize'
    : 'h-1 cursor-row-resize'} -mx-0 outline-none"
  onpointerdown={start}
  onkeydown={key}
  ondblclick={() => onreset?.()}
>
  <!-- The visible grip: a centred dash that brightens on hover and on focus,
       so the handle is findable without being a second border the whole time.
       The rest of the hit area stays invisible but wide. -->
  <span
    class="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 rounded-full
           bg-border group-hover:bg-border-strong group-focus-visible:bg-accent
           {vertical ? 'h-4 w-[3px]' : 'h-[3px] w-4'}"
  ></span>
  <!-- The real hit area, invisible and centred on the grip. -->
  <span class="absolute {vertical ? '-inset-x-1 inset-y-0' : '-inset-y-1 inset-x-0'}"></span>
</div>
