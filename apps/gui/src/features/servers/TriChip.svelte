<script lang="ts">
  import type { Component } from "svelte";
  import Check from "~icons/lucide/check";
  import X from "~icons/lucide/x";
  import { cn } from "$lib/cx";
  import { TRI_NEXT, type Tri } from "./filters.svelte";

  /**
   * A filter with three answers: every server, only these, none of these.
   * One click moves to the next; the chip wears a tick or a cross so the
   * state reads without hovering.
   */
  let {
    value = $bindable(),
    label,
    title,
    icon,
  }: { value: Tri; label: string; title: string; icon?: Component<{ class?: string }> } = $props();
</script>

<button
  type="button"
  {title}
  aria-label={`${label}: ${value}`}
  onclick={() => (value = TRI_NEXT[value])}
  class={cn(
    "inline-flex h-control-sm items-center gap-1 rounded-sm border px-1.5 text-2xs font-medium transition-colors",
    value === "all" && "border-border text-fg-muted hover:border-border-strong hover:text-fg",
    value === "only" && "border-ok/50 bg-ok/10 text-ok",
    value === "none" && "border-err/50 bg-err/10 text-err",
  )}
>
  {#if value === "only"}<Check class="size-3" />{:else if value === "none"}<X class="size-3" />{:else if icon}{@const I = icon}<I class="size-3" />{/if}
  {label}
</button>
