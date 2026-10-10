<script lang="ts">
  import type { Snippet } from "svelte";
  import { DropdownMenu } from "bits-ui";
  import { cn } from "$lib/cx";

  // A menu's sub-menu: portalled to the body like the menu itself, placed by
  // floating-ui with room to flip and shift inside the window. Its layer is
  // `z-tip`, not `z-popover`: a sub-menu opened from a title-bar menu is
  // shifted up beside its trigger, and at `z-popover` (40) it slid under the
  // title bar (`z-titlebar`, 70) and was cut off.
  let {
    side = "left",
    align = "start",
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
  <DropdownMenu.SubContent
    {side}
    {align}
    sideOffset={4}
    avoidCollisions
    collisionPadding={8}
    class={cn(
      "z-tip max-h-[var(--bits-dropdown-menu-content-available-height)] min-w-44 overflow-y-auto rounded-md border",
      "border-border bg-overlay p-1 shadow-pop outline-none data-[state=open]:animate-pop-in data-[state=closed]:animate-pop-out",
      klass,
    )}
  >
    {@render children()}
  </DropdownMenu.SubContent>
</DropdownMenu.Portal>
