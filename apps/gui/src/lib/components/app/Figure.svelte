<script lang="ts">
  import { cn } from "$lib/cx";

  /** One figure in a header: a stencilled label over a mono value. */
  let {
    label,
    value,
    tone = "text-fg",
    title = "",
    secondary = false,
  }: {
    label: string;
    value: string;
    tone?: string;
    title?: string;
    /** A figure that can go when the window is narrow, before the header wraps. */
    secondary?: boolean;
  } = $props();
</script>

<span
  class={cn("flex flex-col gap-0.5", secondary && "max-[1240px]:hidden")}
  title={title || undefined}
>
  <span class="label-stencil whitespace-nowrap text-fg-faint">{label}</span>
  <span class={cn("num font-mono text-sm font-semibold whitespace-nowrap", tone)}>{value}</span>
</span>
