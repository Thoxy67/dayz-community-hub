<script lang="ts">
  import type { Component, Snippet } from "svelte";
  import { SectionLabel } from "$lib/components/ui/section-label";
  import { cn } from "$lib/cx";

  /**
   * One titled block of a panel: a stencilled label with its icon and
   * controls, then the content. Consecutive sections are divided by a rule.
   */
  let {
    title,
    icon,
    actions,
    class: klass = "",
    el = $bindable(),
    children,
  }: {
    title: string;
    icon?: Component<{ class?: string }>;
    actions?: Snippet;
    class?: string;
    /** The section element, for a caller that scrolls to it. */
    el?: HTMLElement;
    children: Snippet;
  } = $props();
</script>

<section
  bind:this={el}
  class={cn("flex flex-col gap-2 border-t border-border px-pad py-2.5 first:border-t-0", klass)}
>
  <SectionLabel {icon} {actions}>{title}</SectionLabel>
  {@render children()}
</section>
