<script lang="ts">
  import type { Component, Snippet } from "svelte";
  import { cn } from "$lib/cx";

  /**
   * A titled card of related settings or facts: the header says what the
   * group is for (and, beside it, what it is used for), the body holds
   * `Field`s or anything else.
   */
  let {
    title,
    description = "",
    icon,
    tone = "text-accent",
    actions,
    padded = false,
    class: klass = "",
    children,
  }: {
    title: string;
    /** A short line beside the title: what this group is used for. */
    description?: string;
    icon?: Component<{ class?: string }>;
    tone?: string;
    actions?: Snippet;
    padded?: boolean;
    class?: string;
    children: Snippet;
  } = $props();
</script>

<section class={cn("overflow-hidden rounded-md border border-border bg-bg/40", klass)}>
  <header class="flex min-h-control items-center gap-2 border-b border-border bg-raised/30 px-3 py-1">
    {#if icon}
      {@const Icon = icon}
      <Icon class={cn("size-icon-sm shrink-0", tone)} />
    {/if}
    <h2 class="m-0 label-stencil whitespace-nowrap text-fg-muted">{title}</h2>
    {#if description}<span class="min-w-0 truncate text-2xs text-fg-faint">{description}</span>{/if}
    {#if actions}<div class="ml-auto flex shrink-0 items-center gap-1">{@render actions()}</div>{/if}
  </header>
  <div class={cn(padded && "p-3")}>{@render children()}</div>
</section>
