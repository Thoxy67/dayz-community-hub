<script lang="ts">
  import type { Component, Snippet } from "svelte";
  import { cn } from "$lib/cx";

  /**
   * What a list says when it has nothing in it.
   *
   * Not an apology and not a mood: what would put something here, in a
   * sentence, and the button that does it when there is one. The same grey
   * paragraph was pasted into nine places, which is how eight of them came to
   * say nothing about what to do next.
   *
   * `compact` is one line for a narrow column; the full form stands in the
   * middle of the space the list would have filled.
   */
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

{#if compact}
  <p class={cn("m-0 flex items-start gap-1.5 p-2 text-2xs leading-relaxed text-fg-muted", klass)}>
    {#if icon}
      {@const Icon = icon}
      <Icon class="mt-0.5 size-icon-sm shrink-0 text-fg-faint" />
    {/if}
    <span class="min-w-0">
      {#if title}<span class="text-fg">{title}</span>{" "}{/if}
      {@render children?.()}
    </span>
  </p>
{:else}
  <div
    class={cn(
      "flex min-h-0 flex-1 flex-col items-center justify-center gap-2 p-6 text-center",
      klass,
    )}
  >
    {#if icon}
      {@const Icon = icon}
      <span
        class="grid size-9 place-items-center rounded-md border border-border bg-bg text-fg-faint"
      >
        <Icon class="size-icon-lg" />
      </span>
    {/if}
    {#if title}<p class="m-0 text-sm font-medium text-fg">{title}</p>{/if}
    {#if children}
      <p class="m-0 max-w-[42ch] text-xs leading-relaxed text-fg-muted">{@render children()}</p>
    {/if}
    {#if action}<div class="mt-1 flex items-center gap-1.5">{@render action()}</div>{/if}
  </div>
{/if}
