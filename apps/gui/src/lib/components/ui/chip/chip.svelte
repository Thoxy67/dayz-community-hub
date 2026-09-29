<script lang="ts">
  import type { Snippet } from "svelte";
  import { Button } from "$lib/components/ui/button";

  // A latching chip: on or off, and it says which by outline rather than fill.
  //
  // Rows of these are how tier and enchantment get picked, because they are
  // picked in combinations and thirteen rows of checkbox would swallow a
  // panel. The outline matters: a whole row of filled chips reads as one solid
  // block, where an outline keeps each chip legible and the row scannable.
  //
  // Written once because the class that says "on" was written out five times,
  // three in the radar's filters and two in the route view, and a sixth was
  // about to be. A style stated five times is a style that drifts.
  let {
    active,
    onclick,
    title = "",
    children,
  }: {
    active: boolean;
    onclick: () => void;
    title?: string;
    children: Snippet;
  } = $props();
</script>

<Button
  size="xs"
  {active}
  {title}
  {onclick}
  class={active ? "border-accent bg-transparent text-accent hover:bg-accent/10" : ""}
>
  {@render children()}
</Button>
