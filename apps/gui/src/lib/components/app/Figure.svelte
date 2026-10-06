<script lang="ts">
  import { cn } from "$lib/cx";

  /**
   * One figure in a header: a stencilled label over a mono value. With
   * `onclick` it is a shortcut to what it counts (a filter, a sort): it says
   * so with a dotted underline on hover, and `active` marks it while that
   * filter or sort is in force.
   */
  let {
    label,
    value,
    tone = "text-fg",
    title = "",
    secondary = false,
    active = false,
    onclick,
  }: {
    label: string;
    value: string;
    tone?: string;
    title?: string;
    /** A figure that can go when the window is narrow, before the header wraps. */
    secondary?: boolean;
    active?: boolean;
    onclick?: () => void;
  } = $props();
</script>

{#snippet body()}
  <span
    class={cn(
      "label-stencil whitespace-nowrap",
      active ? "text-accent" : "text-fg-faint",
      onclick && "group-hover/fig:text-fg-muted",
    )}>{label}</span
  >
  <span
    class={cn(
      "num font-mono text-sm font-semibold whitespace-nowrap",
      tone,
      onclick && "decoration-dotted decoration-1 underline-offset-3 group-hover/fig:underline",
    )}>{value}</span
  >
{/snippet}

{#if onclick}
  <button
    type="button"
    class={cn(
      "group/fig relative flex flex-col gap-0.5 rounded-xs text-left",
      "after:absolute after:inset-x-0 after:-bottom-1 after:h-0.5 after:rounded-full",
      active ? "after:bg-accent" : "after:bg-transparent",
      secondary && "max-[1240px]:hidden",
    )}
    title={title || undefined}
    aria-pressed={active}
    {onclick}
  >
    {@render body()}
  </button>
{:else}
  <span
    class={cn("flex flex-col gap-0.5", secondary && "max-[1240px]:hidden")}
    title={title || undefined}
  >
    {@render body()}
  </span>
{/if}
