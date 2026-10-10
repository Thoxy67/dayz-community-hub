<script lang="ts" module>
  import type { PadKind } from "$lib/ipc/gamepad";
  import type { PadButton } from "$lib/shortcuts/registry";

  export type { PadButton };

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
  /** The sticks, pressed in or pushed. */
  const STICK: Record<"l3" | "r3" | "rs", Record<"xbox" | "sony" | "nintendo", string>> = {
    l3: { xbox: "LS", sony: "L3", nintendo: "LS" },
    r3: { xbox: "RS", sony: "R3", nintendo: "RS" },
    rs: { xbox: "R", sony: "R", nintendo: "R" },
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
  import Move from "~icons/lucide/move";
  import MoveVertical from "~icons/lucide/move-vertical";
  import { cn } from "$lib/cx";

  /**
   * A controller button as the pad in hand prints it: Xbox letters by
   * default, PlayStation shapes on a Sony pad, Nintendo's swapped letters
   * on a Switch pad. The Steam Deck prints Xbox's letters and L1/R1, L3/R3.
   * Nintendo's Menu and View are its + and − buttons.
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
{:else if (button === "menu" || button === "view") && kind === "nintendo"}
  <span
    class={cn(
      "inline-grid size-4.5 shrink-0 place-items-center rounded-full border border-border-strong bg-raised font-mono text-3xs font-bold text-fg-muted",
      klass,
    )}
    aria-hidden="true">{button === "menu" ? "+" : "−"}</span
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
{:else if button === "dpad"}
  <span
    class={cn(
      "inline-grid size-4.5 shrink-0 place-items-center rounded-sm border border-border-strong bg-raised text-fg-muted",
      klass,
    )}
    aria-hidden="true"><Move class="size-3" /></span
  >
{:else if button === "l3" || button === "r3" || button === "rs"}
  <span
    class={cn(
      "inline-flex h-4.5 min-w-4.5 shrink-0 items-center justify-center rounded-full border border-dashed border-border-strong bg-raised px-1 font-mono text-3xs font-bold text-fg-muted",
      klass,
    )}
    aria-hidden="true"
    >{STICK[button][shoulder]}{#if button === "rs"}<MoveVertical class="size-2.5" />{/if}</span
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
