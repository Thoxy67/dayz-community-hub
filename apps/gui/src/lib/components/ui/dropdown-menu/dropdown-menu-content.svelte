<script lang="ts">
  import type { Snippet } from "svelte";
  import { DropdownMenu } from "bits-ui";
  import { cn } from "$lib/cx";

  // The list a menu opens: bits-ui's, placed by floating-ui and portalled to
  // the body, so a menu opened at the window's edge turns inward.
  let {
    side = "bottom",
    align = "end",
    class: klass = "",
    children,
  }: {
    side?: "top" | "right" | "bottom" | "left";
    align?: "start" | "center" | "end";
    class?: string;
    children: Snippet;
  } = $props();
</script>

<DropdownMenu.Portal>
  <DropdownMenu.Content
    {side}
    {align}
    sideOffset={4}
    collisionPadding={4}
    class={cn(
      "z-popover max-h-[var(--bits-dropdown-menu-content-available-height)] min-w-44 overflow-y-auto rounded-md border",
      "border-border bg-overlay p-1 shadow-pop outline-none",
      klass,
    )}
  >
    {@render children()}
  </DropdownMenu.Content>
</DropdownMenu.Portal>
