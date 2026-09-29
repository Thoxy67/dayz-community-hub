<script lang="ts">
  import type { Component, Snippet } from "svelte";
  import { cn } from "$lib/cx";

  // One tile of a StatStrip: the number on top, what it is underneath.
  //
  // Value first because that is what the eye comes for; the label is a
  // caption, not a heading. `grow` lets a tile that holds words rather than
  // figures (a zone, a kind) take the slack the figures do not need.
  let {
    label,
    value,
    tone = "text-fg",
    title = "",
    grow = false,
    icon: Icon,
    mark,
  }: {
    label: string;
    value: string;
    /** Text colour for the value: an accent for the number the tab exists for. */
    tone?: string;
    title?: string;
    grow?: boolean;
    /** What the figure counts, as the glyph the rows under it use. */
    icon?: Component<{ class?: string }>;
    /** A small sign after the label: a lock, a warning. */
    mark?: Snippet;
  } = $props();
</script>

<div
  {title}
  class={cn("flex min-w-14 flex-col justify-center gap-px px-2 py-1", grow && "min-w-20 flex-1")}
>
  <dd class={cn("truncate font-mono tabular-nums", tone)}>{value}</dd>
  <dt class="flex min-w-0 items-center gap-1 text-[10px] leading-3 text-fg-faint">
    {#if Icon}<Icon class="size-icon-sm shrink-0" />{/if}
    <span class="truncate">{label}</span>
    {@render mark?.()}
  </dt>
</div>
