<script lang="ts">
  import type { Component, Snippet } from "svelte";
  import X from "~icons/lucide/x";
  import { cn } from "$lib/cx";

  /**
   * Something the reader has to know, and usually something to do about it.
   *
   * What it is in a few words, why in a sentence, and the way out at the far
   * end: the button that fixes it, or the command that does. It was a single
   * line of coloured text across the top of the window with a button at the
   * end, which had room to say that something was wrong and none to say what
   * it cost to leave it or what pressing the button would do.
   *
   * `banner` runs edge to edge under the window's top, for something about the
   * whole app; without it, it is a card for inside a panel.
   */
  let {
    tone = "warn",
    icon,
    title,
    banner = false,
    dismissLabel,
    ondismiss,
    class: klass = "",
    children,
    actions,
  }: {
    tone?: "warn" | "err" | "ok" | "info";
    icon: Component<{ class?: string }>;
    title: string;
    banner?: boolean;
    /** Given together: the alert can be put away, and what the button is called. */
    dismissLabel?: string;
    ondismiss?: () => void;
    class?: string;
    /** The sentence under the title. */
    children?: Snippet;
    /** Buttons, and a command to copy, at the far end. */
    actions?: Snippet;
  } = $props();

  const Icon = $derived(icon);

  const tones = {
    warn: { edge: "border-warn/35", ground: "bg-warn/8", ink: "text-warn", well: "bg-warn/15" },
    err: { edge: "border-err/40", ground: "bg-err/8", ink: "text-err", well: "bg-err/15" },
    ok: { edge: "border-ok/35", ground: "bg-ok/8", ink: "text-ok", well: "bg-ok/15" },
    info: {
      edge: "border-border-strong",
      ground: "bg-raised/40",
      ink: "text-accent",
      well: "bg-accent/12",
    },
  } as const;
  const look = $derived(tones[tone]);
</script>

<!-- A size container, so it lays itself out by the room it has and not by the
     window's: the same alert is wide across a maximised window and narrow
     beside an opened sidebar or in a window snapped to a third of a screen.

     Wide, it is one line: the words, then the way out at the far end. Narrow,
     the words go on top and the way out is one row under them, where a long
     command gives way (it truncates, and can still be copied whole) rather
     than pushing the button onto a line of its own. Only when there is no
     room even for that does the button drop under the command. -->
<div
  role={tone === "err" || tone === "warn" ? "alert" : "status"}
  class={cn(
    "@container shrink-0 px-2.5 py-2",
    look.ground,
    banner ? cn("border-b", look.edge) : cn("rounded-md border", look.edge),
    klass,
  )}
>
  <div class="flex items-start gap-2.5">
    <span
      class={cn("grid size-control shrink-0 place-items-center rounded-sm", look.well, look.ink)}
    >
      <Icon class="size-icon-lg" />
    </span>
    <div
      class="flex min-w-0 flex-1 flex-col gap-1.5 @[60rem]:flex-row @[60rem]:items-center @[60rem]:gap-3"
    >
      <div class="min-w-0 flex-1">
        <p class={cn("m-0 text-xs font-semibold", look.ink)}>{title}</p>
        {#if children}
          <p class="m-0 mt-0.5 text-2xs leading-relaxed text-fg-muted">{@render children()}</p>
        {/if}
      </div>
      {#if actions}
        <div
          class="flex min-w-0 flex-wrap items-center justify-end gap-1.5 @[60rem]:max-w-[55%] @[60rem]:flex-nowrap"
        >
          {@render actions()}
        </div>
      {/if}
    </div>
    {#if ondismiss}
      <button
        type="button"
        aria-label={dismissLabel}
        title={dismissLabel}
        class="grid size-control-sm shrink-0 place-items-center rounded-sm text-fg-faint hover:bg-raised hover:text-fg"
        onclick={ondismiss}
      >
        <X class="size-icon" />
      </button>
    {/if}
  </div>
</div>
