<script lang="ts" module>
  /**
   * How a segment reads at a glance: idle, working on something (`accent`
   * for the server list, `info` for a download), asking for attention, or
   * failed, plus `ok` for good news.
   */
  export type Tone = "idle" | "accent" | "info" | "ok" | "warn" | "err";

  /** The narrow bar keeps icon and figures: words go below this width. */
  export const WORD = "@max-[1100px]:hidden";
</script>

<script lang="ts">
  import type { Component, Snippet } from "svelte";
  import type { HTMLButtonAttributes } from "svelte/elements";
  import { Tooltip } from "$lib/components/ui/tooltip";
  import { cn } from "$lib/cx";

  /**
   * One segment of the status bar, always built the same way: an icon in
   * the segment's tone, a short text, and, while something is under way, a
   * thin bar along its foot (a sweeping band when there is nothing to
   * measure). A tooltip says what it shows and what a click does; a segment
   * that acts is a button, one that only says something is not. `actions`
   * are small icon buttons beside it (a scan's pause), siblings of the main
   * button since a button cannot hold another.
   */
  let {
    icon: Icon,
    tone = "idle",
    tip,
    label,
    onclick,
    trigger,
    progress,
    pulse = false,
    class: klass = "",
    actions,
    children,
  }: {
    icon: Component<{ class?: string }>;
    tone?: Tone;
    tip: string;
    /** What the button does, for a screen reader (the tooltip is not read). */
    label?: string;
    onclick?: () => void;
    /**
     * A menu trigger's props (bits-ui's `child` snippet): the segment's
     * button becomes the trigger. They must be spread on the element itself
     * (https://bits-ui.com/docs/child-snippet).
     */
    trigger?: Record<string, unknown>;
    /** 0–100 for a bar, null for a sweep, undefined for none. */
    progress?: number | null;
    /** The icon breathes: something runs that has no bar of its own. */
    pulse?: boolean;
    /** On the segment: container-query classes that hide it when narrow. */
    class?: string;
    actions?: Snippet;
    children: Snippet;
  } = $props();

  const icon = $derived(
    {
      idle: "text-fg-faint",
      accent: "text-accent",
      info: "text-info",
      ok: "text-ok",
      warn: "text-warn",
      err: "text-err",
    }[tone],
  );
  // Words stay quiet while work runs; attention and failure say it in colour.
  const text = $derived(
    tone === "warn"
      ? "text-warn"
      : tone === "err"
        ? "text-err"
        : tone === "ok"
          ? "text-ok font-medium"
          : "text-fg-muted",
  );
  const fill = $derived(
    {
      idle: "bg-fg-faint",
      accent: "bg-accent",
      info: "bg-info",
      ok: "bg-ok",
      warn: "bg-warn",
      err: "bg-err",
    }[tone],
  );
  const acts = $derived(!!onclick || !!trigger);
  const body = "flex h-full min-w-0 items-center gap-1.5 rounded-xs px-2 whitespace-nowrap";
</script>

{#snippet inner()}
  <Icon class={cn("size-3 shrink-0", icon, pulse && "animate-pulse")} />
  {@render children()}
{/snippet}

<div class={cn("relative flex h-control-sm min-w-0 shrink-0 items-center", klass)}>
  <Tooltip text={tip} side="top" class="flex h-full min-w-0">
    {#if acts}
      <button
        type="button"
        {...(trigger ?? { onclick }) as HTMLButtonAttributes}
        class={cn(
          body,
          text,
          "transition-colors duration-fast hover:bg-raised hover:text-fg focus-visible:bg-raised",
        )}
        aria-label={label}
      >
        {@render inner()}
      </button>
    {:else}
      <span class={cn(body, text)}>{@render inner()}</span>
    {/if}
  </Tooltip>
  {@render actions?.()}
  {#if progress !== undefined}
    <!-- The bar along the segment's foot: it eases to each new value. -->
    <span
      class="pointer-events-none absolute inset-x-2 bottom-0 h-0.5 overflow-hidden rounded-full bg-raised"
      aria-hidden="true"
    >
      {#if progress === null}
        <span class={cn("absolute inset-y-0 left-0 w-1/3 animate-sweep rounded-full", fill)}></span>
      {:else}
        <span
          class={cn(
            "absolute inset-y-0 left-0 rounded-full transition-[width] duration-base ease-out",
            fill,
          )}
          style="width: {Math.max(0, Math.min(100, progress))}%"
        ></span>
      {/if}
    </span>
  {/if}
</div>
