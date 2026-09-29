/**
 * The density's row height, in pixels, kept current.
 *
 * A virtual list does arithmetic in pixels and the row height is a token
 * that the theme's density moves, so something has to read it back. One
 * element measured once per change of theme, shared by every list.
 */
let px = $state(22);
let watching = false;

function measure() {
  const probe = document.createElement("div");
  probe.style.cssText = "position:absolute;visibility:hidden;height:var(--spacing-row)";
  document.body.appendChild(probe);
  const h = probe.getBoundingClientRect().height;
  probe.remove();
  if (h > 0) px = Math.round(h);
}

/** The height of one list row under the density being worn. */
export function rowPx(): number {
  if (!watching && typeof document !== "undefined") {
    watching = true;
    // Not now: this is called from templates, and writing state while one is
    // being rendered is an error. The compact height it starts at is right
    // for the default theme, so the first frame is correct either way.
    queueMicrotask(() => {
      measure();
      // Density is an attribute on the root, and that is the only thing that
      // moves this measure.
      new MutationObserver(measure).observe(document.documentElement, {
        attributes: true,
        attributeFilter: ["data-density", "style"],
      });
    });
  }
  return px;
}
