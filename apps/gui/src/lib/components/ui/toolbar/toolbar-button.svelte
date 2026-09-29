<script lang="ts">
  import type { Component } from "svelte";
  import { Toolbar } from "bits-ui";
  import { Tooltip } from "$lib/components/ui/tooltip";
  import { button } from "$lib/components/ui/button";
  import { cn } from "$lib/cx";

  // One action on a toolbar: an icon, the name it goes by, the key that does
  // the same. Part of the bar's arrow-key order, which a plain button is not.
  let {
    icon,
    label,
    kbd = "",
    disabled = false,
    onclick,
    class: klass = "",
  }: {
    icon: Component<{ class?: string }>;
    label: string;
    kbd?: string;
    disabled?: boolean;
    onclick?: (e: MouseEvent) => void;
    class?: string;
  } = $props();

  const Icon = $derived(icon);
</script>

<Tooltip text={label} {kbd} side="bottom">
  <Toolbar.Button
    aria-label={label}
    {disabled}
    {onclick}
    class={cn(button({ variant: "ghost", size: "icon" }), klass)}
  >
    <Icon class="size-icon" />
  </Toolbar.Button>
</Tooltip>
