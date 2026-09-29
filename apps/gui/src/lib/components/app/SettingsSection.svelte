<script lang="ts">
  import type { Component, Snippet } from "svelte";
  import { cn } from "$lib/cx";

  /**
   * One part of the settings page: a stencilled title, a sentence saying what
   * it governs, and its controls in a panel. `id` is what the page's own
   * navigation and `app.focus` scroll to.
   */
  let {
    id,
    title,
    description = "",
    icon,
    tone = "neutral",
    aside,
    children,
  }: {
    id: string;
    title: string;
    description?: string;
    icon?: Component<{ class?: string }>;
    tone?: "neutral" | "danger";
    /** Right of the title: a status tag, a count. */
    aside?: Snippet;
    children: Snippet;
  } = $props();
</script>

<section {id} class="scroll-mt-4" aria-labelledby="{id}-title">
  <header class="mb-2 flex items-start gap-2">
    {#if icon}
      {@const Icon = icon}
      <Icon class={cn("mt-[3px] size-icon-lg shrink-0", tone === "danger" ? "text-err" : "text-accent")} />
    {/if}
    <div class="min-w-0 flex-1">
      <h2 id="{id}-title" class={cn("m-0 title-display text-xl", tone === "danger" ? "text-err" : "text-fg")}>
        {title}
      </h2>
      {#if description}<p class="m-0 mt-0.5 text-xs text-fg-muted">{description}</p>{/if}
    </div>
    {#if aside}<div class="flex shrink-0 items-center gap-1.5">{@render aside()}</div>{/if}
  </header>
  <div
    class={cn(
      "overflow-hidden rounded-md border bg-bg/60",
      tone === "danger" ? "border-err/40" : "border-border",
    )}
  >
    {@render children()}
  </div>
</section>
