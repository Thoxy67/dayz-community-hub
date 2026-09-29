<script lang="ts">
  import { cn } from "$lib/cx";

  /** One figure in a header: a stencilled label over a mono value. */
  let {
    label,
    value,
    tone = "text-fg",
    title = "",
  }: { label: string; value: string; tone?: string; title?: string } = $props();
</script>

<span class="flex flex-col gap-0.5" title={title || undefined}>
  <span class="label-stencil whitespace-nowrap text-fg-faint">{label}</span>
  <span class={cn("num font-mono text-sm font-semibold whitespace-nowrap", tone)}>{value}</span>
</span>
