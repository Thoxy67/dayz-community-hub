<script lang="ts">
  import type { Snippet } from "svelte";
  import { Tooltip } from "bits-ui";
  import { Kbd } from "$lib/components/ui/kbd";

  // A hint beside a control: bits-ui's Tooltip, which places it with
  // floating-ui (flips to the side that has room, stays inside the window),
  // waits a moment unless one was just up, and takes it down on Escape.
  //
  // It needs a `TooltipProvider` somewhere above it, which each window's root
  // component supplies; that is also where the delays are set.
  //
  // The wrapper is a span around whatever is passed in, because what is
  // passed in is usually one of this app's own buttons and a button cannot go
  // inside the button bits-ui would otherwise render. The pointer finds a span
  // as well as it finds a button. The keyboard does not: focus lands on the
  // control inside and `focus` does not bubble, so `focusin` is listened for
  // here and opens the hint by hand, and only for a focus the keyboard gave.
  // A plain `:focus` keeps a tooltip up after its control is *clicked*, since
  // a clicked button stays focused, which is the "tooltip that will not go
  // away" the view tabs once had.
  //
  // It wraps. It used not to, which cut a sentence off at the edge of a pane
  // 180 pixels wide, and a second mechanism (the native `title`, unreachable
  // from the keyboard) had grown up beside it for prose.
  let {
    text,
    kbd = "",
    side = "right",
    class: klass = "inline-flex",
    children,
  }: {
    text: string;
    kbd?: string;
    side?: "right" | "top" | "bottom" | "left";
    /** On the wrapper. `inline-flex` suits a button; a row wants `flex`. */
    class?: string;
    children: Snippet;
  } = $props();

  let open = $state(false);
</script>

<Tooltip.Root bind:open disabled={!text}>
  <!-- Not a tab stop of its own: the control inside it is. -->
  <Tooltip.Trigger tabindex={-1}>
    {#snippet child({ props })}
      <span
        {...props}
        class={klass}
        onfocusin={(e) => {
          if (e.target instanceof Element && e.target.matches(":focus-visible")) open = true;
        }}
        onfocusout={() => (open = false)}
      >
        {@render children()}
      </span>
    {/snippet}
  </Tooltip.Trigger>
  <Tooltip.Portal>
    <Tooltip.Content
      {side}
      sideOffset={6}
      collisionPadding={4}
      class="z-tip flex w-max max-w-[min(20rem,var(--bits-floating-available-width,20rem))] items-center gap-1.5
             rounded-md border border-border bg-overlay px-2 py-1 text-2xs leading-snug text-fg shadow-pop"
    >
      <span class="min-w-0">{text}</span>
      {#if kbd}<Kbd>{kbd}</Kbd>{/if}
    </Tooltip.Content>
  </Tooltip.Portal>
</Tooltip.Root>
