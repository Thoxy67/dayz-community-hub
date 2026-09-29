<script lang="ts" module>
  import type { PadKind } from "$lib/ipc/gamepad";

  export type PadButton = "a" | "b" | "x" | "y" | "lb" | "rb" | "lt" | "rt" | "menu" | "view";

  /** The face buttons by position (a is the bottom one), as each maker prints them. */
  const FACE: Record<"a" | "b" | "x" | "y", Record<"letters" | "shapes" | "nintendo", string>> = {
    a: { letters: "A", shapes: "✕", nintendo: "B" },
    b: { letters: "B", shapes: "○", nintendo: "A" },
    x: { letters: "X", shapes: "□", nintendo: "Y" },
    y: { letters: "Y", shapes: "△", nintendo: "X" },
  };
  const SHOULDER: Record<
    "lb" | "rb" | "lt" | "rt",
    Record<"xbox" | "sony" | "nintendo", string>
  > = {
    lb: { xbox: "LB", sony: "L1", nintendo: "L" },
    rb: { xbox: "RB", sony: "R1", nintendo: "R" },
    lt: { xbox: "LT", sony: "L2", nintendo: "ZL" },
    rt: { xbox: "RT", sony: "R2", nintendo: "ZR" },
  };
  /** Xbox's colours by letter; PlayStation's by shape. */
  const TONE: Record<"a" | "b" | "x" | "y", { letters: string; shapes: string }> = {
    a: { letters: "text-ok", shapes: "text-info" },
    b: { letters: "text-err", shapes: "text-err" },
    x: { letters: "text-info", shapes: "text-mods" },
    y: { letters: "text-warn", shapes: "text-ok" },
  };
</script>

<script lang="ts">
  import Menu from "~icons/lucide/menu";
  import Copy from "~icons/lucide/copy";
  import { cn } from "$lib/cx";

  /**
   * A controller button as the pad in hand prints it: Xbox letters by
   * default, PlayStation shapes on a Sony pad, Nintendo's swapped letters
   * on a Switch pad. The Steam Deck prints Xbox's letters.
   */
  let {
    button,
    kind = "xbox",
    class: klass = "",
  }: { button: PadButton; kind?: PadKind; class?: string } = $props();

  const face = $derived(
    kind === "playStation" ? "shapes" : kind === "nintendo" ? "nintendo" : "letters",
  );
  const shoulder = $derived(
    kind === "playStation" || kind === "steamDeck"
      ? "sony"
      : kind === "nintendo"
        ? "nintendo"
        : "xbox",
  );
</script>

{#if button === "a" || button === "b" || button === "x" || button === "y"}
  <span
    class={cn(
      "inline-grid size-4.5 shrink-0 place-items-center rounded-full border border-border-strong bg-raised font-mono text-3xs font-bold",
      face === "nintendo" ? "text-fg" : TONE[button][face === "shapes" ? "shapes" : "letters"],
      klass,
    )}
    aria-hidden="true">{FACE[button][face]}</span
  >
{:else if button === "menu" || button === "view"}
  {@const I = button === "menu" ? Menu : Copy}
  <span
    class={cn(
      "inline-grid size-4.5 shrink-0 place-items-center rounded-full border border-border-strong bg-raised text-fg-muted",
      klass,
    )}
    aria-hidden="true"><I class="size-2.5" /></span
  >
{:else}
  <span
    class={cn(
      "inline-grid h-4.5 min-w-6 shrink-0 place-items-center rounded-sm border border-border-strong bg-raised px-1 font-mono text-3xs font-bold text-fg-muted",
      klass,
    )}
    aria-hidden="true">{SHOULDER[button][shoulder]}</span
  >
{/if}
