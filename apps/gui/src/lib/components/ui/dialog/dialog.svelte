<script lang="ts">
  import type { Snippet } from "svelte";
  import { Dialog } from "bits-ui";
  import X from "~icons/lucide/x";
  import { cn } from "$lib/cx";

  /**
   * A question that has to be answered before anything else happens.
   *
   * The app had none, so the things that needed one (wipe every setting,
   * delete a saved route) grew a second button in place, which moved the
   * layout under the pointer and could be confirmed by a double click meant
   * for the first. bits-ui holds the focus inside, gives it back afterwards,
   * closes on Escape and names the thing for a screen reader; what is here is
   * what it looks like.
   *
   * No `name` is ever passed to a bits-ui primitive in this app: see the
   * README beside this folder. The veil is a flat colour, not a blur: a
   * backdrop filter over the radar is a full-window resample on every frame
   * the plot draws.
   */
  let {
    open = $bindable(false),
    title,
    description = "",
    size = "sm",
    closeLabel = "Close",
    class: klass = "",
    children,
    footer,
  }: {
    open?: boolean;
    title: string;
    description?: string;
    size?: "sm" | "md" | "lg";
    closeLabel?: string;
    class?: string;
    children?: Snippet;
    /** The buttons, at the end: the safe one first, the one that acts last. */
    footer?: Snippet;
  } = $props();

  // Never wider than the window: a dialog in a window snapped to a third of
  // the screen keeps a margin rather than running off both edges.
  const sizes = {
    sm: "w-[min(22rem,calc(100vw-1rem))]",
    md: "w-[min(32rem,calc(100vw-1rem))]",
    lg: "w-[min(44rem,calc(100vw-1rem))]",
  };
</script>

<Dialog.Root bind:open>
  <Dialog.Portal>
    <Dialog.Overlay class="fixed inset-0 z-dialog bg-plot/75" />
    <Dialog.Content
      class={cn(
        "fixed top-1/2 left-1/2 z-dialog flex max-h-[calc(100vh-2rem)] max-w-[calc(100vw-2rem)]",
        "-translate-x-1/2 -translate-y-1/2 flex-col rounded-md border border-border-strong bg-panel shadow-pop",
        sizes[size],
        klass,
      )}
    >
      <header class="flex shrink-0 items-start gap-2 border-b border-border px-3 py-2">
        <span class="mt-1 h-3 w-0.5 shrink-0 rounded-full bg-accent"></span>
        <div class="min-w-0 flex-1">
          <Dialog.Title class="m-0 text-sm font-semibold text-fg">{title}</Dialog.Title>
          {#if description}
            <Dialog.Description class="m-0 mt-0.5 text-xs leading-relaxed text-fg-muted">
              {description}
            </Dialog.Description>
          {/if}
        </div>
        <Dialog.Close
          aria-label={closeLabel}
          class="grid size-control-sm shrink-0 place-items-center rounded-sm text-fg-faint hover:bg-raised hover:text-fg"
        >
          <X class="size-icon" />
        </Dialog.Close>
      </header>
      {#if children}
        <div class="min-h-0 flex-1 overflow-y-auto px-3 py-2.5 text-xs text-fg">
          {@render children()}
        </div>
      {/if}
      {#if footer}
        <footer
          class="flex shrink-0 items-center justify-end gap-1.5 border-t border-border px-3 py-2"
        >
          {@render footer()}
        </footer>
      {/if}
    </Dialog.Content>
  </Dialog.Portal>
</Dialog.Root>
