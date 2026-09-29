<script lang="ts">
  import type { Component, Snippet } from "svelte";
  import { EmptyState } from "$lib/components/ui/empty-state";
  import { Topo } from "$lib/components/ui/topo";
  import { cn } from "$lib/cx";

  /** Nothing here: the kit's empty state over the contour texture, filling its parent. */
  let {
    icon,
    title = "",
    compact = false,
    class: klass = "",
    action,
    children,
  }: {
    icon?: Component<{ class?: string }>;
    title?: string;
    compact?: boolean;
    class?: string;
    action?: Snippet;
    children?: Snippet;
  } = $props();
</script>

<div class={cn("relative h-full min-h-48 overflow-hidden", klass)}>
  <Topo />
  <EmptyState {icon} {title} {compact} {action} {children} class="relative h-full" />
</div>
