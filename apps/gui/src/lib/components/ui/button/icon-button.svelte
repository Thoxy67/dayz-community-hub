<script lang="ts">
  import type { Component } from "svelte";
  import { Tooltip } from "$lib/components/ui/tooltip";
  import Button from "./button.svelte";

  /**
   * A button that is only an icon.
   *
   * The icon, the name a screen reader says, the hint a pointer gets and the
   * shortcut beside it are one thing said four ways, so they are asked for
   * once. Written out by hand, each of the forty of these in the app had
   * decided for itself which of the four to leave out: usually the name.
   */
  let {
    icon,
    label,
    kbd = "",
    side = "bottom",
    size = "icon",
    variant = "ghost",
    active = false,
    disabled = false,
    iconClass = "",
    class: klass = "",
    onclick,
  }: {
    icon: Component<{ class?: string }>;
    /** What it does. The accessible name and the tooltip. */
    label: string;
    kbd?: string;
    side?: "right" | "top" | "bottom" | "left";
    size?: "icon" | "icon-xs";
    variant?: "default" | "accent" | "ghost" | "danger";
    active?: boolean;
    disabled?: boolean;
    iconClass?: string;
    class?: string;
    onclick?: (e: MouseEvent) => void;
  } = $props();

  const Icon = $derived(icon);
</script>

<Tooltip text={label} {kbd} {side}>
  <Button {variant} {size} {active} {disabled} {onclick} class={klass} aria-label={label}>
    <Icon class="{size === 'icon' ? 'size-icon' : 'size-icon-sm'} {iconClass}" />
  </Button>
</Tooltip>
