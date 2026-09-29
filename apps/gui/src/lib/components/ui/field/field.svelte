<script lang="ts">
  import type { Snippet } from "svelte";
  import { cn } from "$lib/cx";

  /**
   * One row of a settings form: what it is on the left, the control on the
   * right, and a line under the name saying what it changes.
   */
  let {
    label,
    hint = "",
    for: htmlFor,
    stacked = false,
    class: klass = "",
    children,
  }: {
    label: string;
    hint?: string;
    for?: string;
    /** Control under the label rather than beside it, for wide inputs. */
    stacked?: boolean;
    class?: string;
    children: Snippet;
  } = $props();
</script>

<div
  class={cn(
    "flex gap-3 border-b border-border/60 px-pad py-2 last:border-b-0",
    stacked ? "flex-col gap-1.5" : "items-center",
    klass,
  )}
>
  <div class={cn("min-w-0", stacked ? "" : "w-44 shrink-0")}>
    <label for={htmlFor} class="block text-xs font-medium text-fg">{label}</label>
    {#if hint}<p class="m-0 mt-0.5 text-2xs leading-snug text-fg-faint">{hint}</p>{/if}
  </div>
  <div class="flex min-w-0 flex-1 items-center gap-1.5">{@render children()}</div>
</div>
