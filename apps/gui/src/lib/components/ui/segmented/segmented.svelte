<script lang="ts" generics="T extends string">
  import type { Component } from "svelte";
  import { RadioGroup } from "bits-ui";
  import { cn } from "$lib/cx";

  /**
   * A few choices of which exactly one holds, side by side.
   *
   * bits-ui's RadioGroup drawn as one control: the choices share a frame and
   * the chosen one is lit. A radio group and not a toggle group, because a
   * toggle can be pressed off and here something always holds. Arrows move
   * the choice and only the chosen one is a tab stop.
   *
   * No `name` is passed, so no hidden radio inputs are rendered: see `switch`.
   */
  type Option = {
    value: T;
    label: string;
    icon?: Component<{ class?: string }>;
    /** A longer word for the native tooltip, when the label is an icon or an abbreviation. */
    title?: string;
    disabled?: boolean;
  };

  let {
    value = $bindable(),
    options,
    size = "sm",
    iconOnly = false,
    fill = false,
    onchange,
    class: klass = "",
    "aria-label": ariaLabel,
  }: {
    value: T;
    options: readonly Option[];
    size?: "sm" | "xs";
    /** Draw the icon alone; the label becomes the accessible name. */
    iconOnly?: boolean;
    /** Stretch to the width on offer, the choices sharing it equally. */
    fill?: boolean;
    onchange?: (value: T) => void;
    class?: string;
    "aria-label": string;
  } = $props();
</script>

<RadioGroup.Root
  bind:value={
    () => value,
    (v) => {
      if (!v || v === value) return;
      value = v as T;
      onchange?.(v as T);
    }
  }
  orientation="horizontal"
  aria-label={ariaLabel}
  class={cn(
    "inline-flex shrink-0 overflow-hidden rounded-sm border border-border bg-bg",
    fill && "flex w-full",
    klass,
  )}
>
  {#each options as o (o.value)}
    <RadioGroup.Item
      value={o.value}
      disabled={o.disabled}
      aria-label={iconOnly ? o.label : undefined}
      title={o.title ?? (iconOnly ? o.label : undefined)}
      class={cn(
        "inline-flex items-center justify-center gap-1 border-r border-border last:border-r-0",
        "text-fg-muted hover:bg-raised hover:text-fg",
        "data-[state=checked]:bg-accent/15 data-[state=checked]:text-accent",
        "disabled:cursor-not-allowed disabled:opacity-40",
        size === "sm" ? "h-control text-xs" : "h-control-sm text-2xs",
        iconOnly ? (size === "sm" ? "w-control" : "w-control-sm") : "px-2",
        fill && "min-w-0 flex-1",
      )}
    >
      {#if o.icon}
        {@const Icon = o.icon}
        <Icon class="size-icon shrink-0" />
      {/if}
      {#if !iconOnly}<span class="truncate">{o.label}</span>{/if}
    </RadioGroup.Item>
  {/each}
</RadioGroup.Root>
