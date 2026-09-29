<script lang="ts">
  import Info from "~icons/lucide/info";
  import { Tooltip } from "$lib/components/ui/tooltip";

  // A paragraph's worth of explanation, folded into one glyph.
  //
  // What goes in here is the *why*: the sentence that earns a panel its place
  // and is worth reading once. It has no business taking a third of the panel
  // for the hundredth time somebody looks at it.
  //
  // It was the browser's own `title`, because the app's tooltip could not
  // wrap. It can now, so this is that tooltip, and a button, which means the
  // explanation can be reached with the keyboard: a `title` on a span could
  // only ever be read by somebody with a mouse.
  let { text, side = "top" }: { text: string; side?: "right" | "top" | "bottom" | "left" } =
    $props();
</script>

<Tooltip {text} {side}>
  <button
    type="button"
    aria-label={text}
    class="inline-grid cursor-help place-items-center rounded-xs text-fg-faint hover:text-fg-muted"
  >
    <Info class="size-icon-sm" />
  </button>
</Tooltip>
