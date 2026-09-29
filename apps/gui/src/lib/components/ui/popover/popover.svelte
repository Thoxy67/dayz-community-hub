<script lang="ts">
  import type { Snippet } from "svelte";
  import { Popover } from "bits-ui";
  import { cn } from "$lib/cx";

  // A button that opens a small panel beside itself: bits-ui's Popover.
  //
  // It was written by hand, twice and then once, and each copy had to get
  // "click outside closes it", Escape and the focus coming back right, or the
  // control silently stopped feeling usable. Those are bits-ui's now, and the
  // panel is placed by floating-ui, so one opened near the window's edge
  // turns to the side that has room instead of running off it.
  type Placement = "bottom-end" | "bottom-start" | "right-end";
  type Kind = "menu" | "dialog";

  let {
    label,
    placement = "bottom-end",
    kind = "dialog",
    open = $bindable(false),
    triggerClass = "",
    panelClass = "",
    trigger,
    children,
  }: {
    /** Names both the button and the panel. */
    label: string;
    placement?: Placement;
    kind?: Kind;
    open?: boolean;
    triggerClass?: string;
    panelClass?: string;
    trigger: Snippet;
    children: Snippet;
  } = $props();

  const WHERE: Record<Placement, { side: "bottom" | "right"; align: "start" | "end" }> = {
    "bottom-end": { side: "bottom", align: "end" },
    "bottom-start": { side: "bottom", align: "start" },
    // The rail opens to its right: it sits against the window's edge.
    "right-end": { side: "right", align: "end" },
  };
</script>

<Popover.Root bind:open>
  <Popover.Trigger
    aria-label={label}
    aria-haspopup={kind}
    title={label}
    class={cn(
      "grid shrink-0 place-items-center rounded-md text-fg-faint hover:bg-raised hover:text-fg",
      "data-[state=open]:bg-raised data-[state=open]:text-fg",
      triggerClass,
    )}
  >
    {@render trigger()}
  </Popover.Trigger>
  <Popover.Portal>
    <Popover.Content
      side={WHERE[placement].side}
      align={WHERE[placement].align}
      sideOffset={placement === "right-end" ? 8 : 4}
      collisionPadding={4}
      role={kind === "menu" ? "menu" : undefined}
      aria-label={label}
      class={cn(
        "z-popover overflow-hidden rounded-md border border-border bg-overlay shadow-pop outline-none",
        panelClass,
      )}
    >
      {@render children()}
    </Popover.Content>
  </Popover.Portal>
</Popover.Root>
