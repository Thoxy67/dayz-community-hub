<script lang="ts" generics="T extends string">
  import type { Component } from "svelte";
  import { Tabs } from "bits-ui";
  import { cn } from "$lib/cx";

  /**
   * The strip of tabs above a set of panes.
   *
   * bits-ui's Tabs, the list only. The panes are whoever owns them, because
   * in this app a pane that has been opened stays mounted and is hidden, and
   * `Tabs.Content` unmounts what is not showing. One tab stop, arrows to
   * move, Home and End, and activation follows focus: all bits-ui's.
   *
   * There were six of these in four visual languages and with five different
   * ideas of what ARIA a tab carries. This is the underline one, with the
   * accent tick under the tab that is on.
   */
  type Tab = {
    id: T;
    label: string;
    icon?: Component<{ class?: string }>;
    /** A figure after the label: how many are in there. */
    count?: number;
    title?: string;
  };

  let {
    value = $bindable(),
    tabs,
    size = "sm",
    fill = false,
    stretch = false,
    onchange,
    panels,
    class: klass = "",
    "aria-label": ariaLabel,
  }: {
    value: T;
    tabs: readonly Tab[];
    size?: "sm" | "xs";
    /** Share the width equally instead of sitting at the start. */
    fill?: boolean;
    /**
     * Be as tall as whatever this is put in, rather than a control's height:
     * for a strip that is part of a taller header, so the mark under the tab
     * that is on lands on the header's own bottom edge and not above it.
     */
    stretch?: boolean;
    onchange?: (id: T) => void;
    /**
     * A prefix naming the panels these tabs show, when they live elsewhere:
     * tab `x` is then `<panels>-tab-x` and says it controls `<panels>-panel-x`,
     * which is the id the panel wears (with `aria-labelledby` back to the tab).
     * Without `Tabs.Content` nothing else ties a tab to what it opens.
     */
    panels?: string;
    class?: string;
    "aria-label": string;
  } = $props();
</script>

<Tabs.Root
  bind:value={
    () => value,
    (v) => {
      if (!v || v === value) return;
      value = v as T;
      onchange?.(v as T);
    }
  }
  class="contents"
>
  <Tabs.List
    aria-label={ariaLabel}
    class={cn(
      // Sideways scroll, without a bar: six tabs in a narrow window used to
      // run off the edge, and the ones past it could only be reached with the
      // keyboard. The wheel and a drag reach them now, and the arrows still do.
      "flex min-w-0 items-stretch overflow-x-auto border-b border-border [scrollbar-width:none] [&::-webkit-scrollbar]:hidden",
      stretch ? "flex-1 self-stretch" : "shrink-0",
      klass,
    )}
  >
    {#each tabs as t (t.id)}
      <Tabs.Trigger
        value={t.id}
        title={t.title}
        class={cn(
          "group relative inline-flex items-center justify-center whitespace-nowrap",
          fill ? "min-w-0" : "shrink-0",
          // Sharing a width, every pixel of padding is a letter of somebody's
          // label: three tabs in a 260 pixel sidebar read "Ressou...".
          fill ? "gap-1 px-1" : "gap-1.5 px-2.5",
          "text-fg-muted hover:bg-raised/60 hover:text-fg data-[state=active]:text-accent",
          size === "sm" ? "text-xs" : "text-2xs",
          stretch ? "h-full" : size === "sm" ? "h-control" : "h-control-sm",
          fill && "flex-1",
        )}
      >
        <!-- Rendered by hand so the ids are this strip's to give: bits-ui names
             the tab itself and points it at a `Tabs.Content` that is not there. -->
        {#snippet child({ props })}
          <button
            {...props}
            id={panels ? `${panels}-tab-${t.id}` : (props.id as string)}
            aria-controls={panels ? `${panels}-panel-${t.id}` : undefined}
          >
            {#if t.icon}
              {@const Icon = t.icon}
              <Icon class="size-icon shrink-0" />
            {/if}
            <span class="truncate">{t.label}</span>
            {#if t.count !== undefined}
              <span
                class="rounded-xs bg-raised px-1 font-mono text-2xs text-fg-faint tabular-nums
                       group-data-[state=active]:bg-accent/15 group-data-[state=active]:text-accent"
                >{t.count}</span
              >
            {/if}
            <span
              class="absolute inset-x-1.5 -bottom-px hidden h-0.5 rounded-full bg-accent group-data-[state=active]:block"
            ></span>
          </button>
        {/snippet}
      </Tabs.Trigger>
    {/each}
  </Tabs.List>
</Tabs.Root>
