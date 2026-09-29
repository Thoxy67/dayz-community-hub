<script lang="ts">
  import { Checkbox } from "bits-ui";
  import Check from "~icons/lucide/check";
  import { cn } from "$lib/cx";

  // A checkbox drawn as a dense row, the height of a list row, for filter
  // panels: the label and the box are one hit target.
  //
  // bits-ui's Checkbox, which is a button with `role="checkbox"` and draws no
  // `<input>` at all unless it is given a `name`. **Never pass one**: see
  // `switch`. That also settles something the native box could not do. Some
  // callers own the writing (`onchange`) because the truth lives behind an
  // IPC and may refuse; a native checkbox flips under the pointer whatever its
  // `checked` says, so a refused change left the box lying. This one shows
  // what it is told and nothing else.
  let {
    checked = $bindable(),
    label,
    onchange,
    color = "",
    count,
    title = "",
    class: klass = "",
  }: {
    checked: boolean;
    label: string;
    /** Told what was asked for, when the caller owns the writing. */
    onchange?: (checked: boolean) => void;
    /** Hover text, for a layer whose name does not say what it means. */
    title?: string;
    /** A swatch matching how the thing is drawn on the plot. */
    color?: string;
    count?: number;
    class?: string;
  } = $props();
</script>

<!-- The label is truncated in a narrow column, so the whole of it is the
     tooltip, unless the row has a hint of its own to give. -->
<Checkbox.Root
  bind:checked={
    () => checked,
    (v) => {
      if (onchange) onchange(v);
      else checked = v;
    }
  }
  title={title || label}
  class={cn(
    "group flex h-row w-full cursor-pointer items-center gap-1.5 rounded-xs px-1 text-left text-2xs select-none",
    "text-fg-faint hover:bg-raised data-[state=checked]:text-fg",
    klass,
  )}
>
  <span
    class="grid size-3.5 shrink-0 place-items-center rounded-xs border border-border-strong bg-bg
           group-data-[state=checked]:border-accent group-data-[state=checked]:bg-accent"
  >
    <Check class="hidden size-3 text-accent-fg group-data-[state=checked]:block" />
  </span>
  {#if color}
    <span class="size-2 shrink-0 rounded-xs" style:background={color}></span>
  {/if}
  <span class="min-w-0 flex-1 truncate">{label}</span>
  {#if count !== undefined}
    <span class="font-mono text-fg-faint tabular-nums">{count}</span>
  {/if}
</Checkbox.Root>
