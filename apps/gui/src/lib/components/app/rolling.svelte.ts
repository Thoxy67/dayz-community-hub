import { Tween } from "svelte/motion";
import { cubicOut } from "svelte/easing";

const still =
  typeof matchMedia === "function" && matchMedia("(prefers-reduced-motion: reduce)").matches;

/**
 * A count that rolls to its new value instead of jumping: a header's figure,
 * the title bar's players, the status bar's servers. The first value is shown
 * as it is (no count up from zero on opening a view), and so is every value
 * when the system asks for reduced motion. Whole numbers only: the in-between
 * values are rounded, so a tabular figure never shows a fraction.
 *
 * Call it while a component initialises; `get` is read in an effect.
 */
export function rolling(get: () => number | null | undefined, duration = 400) {
  const t = new Tween(0, { duration, easing: cubicOut });
  let seen = false;
  $effect.pre(() => {
    const v = get();
    if (v == null) return;
    if (!seen || still) {
      seen = true;
      void t.set(v, { duration: 0 });
    } else t.target = v;
  });
  return {
    /** What to show now: the value itself once arrived, else the rounded step. */
    get current(): number | null {
      const v = get();
      if (v == null) return null;
      return t.current === v ? v : Math.round(t.current);
    },
  };
}
