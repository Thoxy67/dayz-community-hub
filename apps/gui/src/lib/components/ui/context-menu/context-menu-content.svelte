<script lang="ts">
  import type { Snippet } from "svelte";
  import { ContextMenu } from "bits-ui";
  import { cn } from "$lib/cx";

  // The menu a right-click opens, at the pointer. The same plate, spacing and
  // layer as the dropdown menus, so the two read as one family.
  let { class: klass = "", children }: { class?: string; children: Snippet } = $props();
</script>

<ContextMenu.Portal>
  <ContextMenu.Content
    collisionPadding={4}
    class={cn(
      "z-popover max-h-[var(--bits-context-menu-content-available-height)] min-w-52 overflow-y-auto rounded-md border",
      "border-border bg-overlay p-1 shadow-pop outline-none",
      klass,
    )}
  >
    {@render children()}
  </ContextMenu.Content>
</ContextMenu.Portal>
