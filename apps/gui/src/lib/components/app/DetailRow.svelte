<script lang="ts">
  import type { Snippet } from "svelte";
  import { cn } from "$lib/cx";

  /**
   * One fact in a `DetailList`. A plain `value` is set in mono, as figures
   * and ids are; `children` for anything richer (a copy button, a meter).
   */
  let {
    label,
    value,
    tone = "text-fg",
    title = "",
    children,
  }: { label: string; value?: string; tone?: string; title?: string; children?: Snippet } = $props();
</script>

<dt class="text-fg-faint">{label}</dt>
<dd class={cn("m-0 min-w-0", !children && "truncate font-mono", !children && tone)} title={title || value || undefined}>
  {#if children}{@render children()}{:else}{value ?? "—"}{/if}
</dd>
