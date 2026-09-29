<script lang="ts">
  import Check from "~icons/lucide/check";
  import Minus from "~icons/lucide/minus";
  import { cn } from "$lib/cx";

  /**
   * A box that is ticked or not. Hand-made, like the switch: bits-ui's
   * Checkbox renders a hidden input, and WebKitGTK's web process has aborted
   * on those (see README). `indeterminate` for a "select all" over a part.
   */
  let {
    checked = $bindable(false),
    indeterminate = false,
    disabled = false,
    onchange,
    class: klass = "",
    "aria-label": ariaLabel,
  }: {
    checked?: boolean;
    indeterminate?: boolean;
    disabled?: boolean;
    onchange?: (checked: boolean) => void;
    class?: string;
    "aria-label": string;
  } = $props();

  function toggle(e: MouseEvent) {
    e.stopPropagation();
    const next = !checked;
    if (onchange) onchange(next);
    else checked = next;
  }
</script>

<button
  type="button"
  role="checkbox"
  aria-checked={indeterminate ? "mixed" : checked}
  aria-label={ariaLabel}
  {disabled}
  onclick={toggle}
  class={cn(
    "grid size-3.5 shrink-0 place-items-center rounded-xs border transition-colors disabled:opacity-40",
    checked || indeterminate
      ? "border-accent bg-accent text-accent-fg"
      : "border-border-strong bg-bg hover:border-fg-faint",
    klass,
  )}
>
  {#if indeterminate}<Minus class="size-3" />{:else if checked}<Check class="size-3" />{/if}
</button>
