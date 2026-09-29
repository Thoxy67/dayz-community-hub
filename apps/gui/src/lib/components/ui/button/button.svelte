<script lang="ts" module>
  import { tv } from "tailwind-variants/lite";

  /**
   * What a button looks like, exported so that something which is not a
   * `<button>` (a link, a menu's trigger) can wear the same clothes.
   *
   * Heights are the theme's measures, not pixels, so density reaches every
   * button there is. `icon` and `icon-xs` are the square ones.
   */
  export const button = tv({
    base: [
      "inline-flex shrink-0 items-center justify-center gap-1.5 rounded-md font-medium select-none",
      "disabled:cursor-not-allowed disabled:opacity-40",
    ],
    variants: {
      variant: {
        default:
          "border border-border bg-raised/80 text-fg hover:border-border-strong hover:bg-raised active:bg-raised/60",
        accent:
          "border border-accent/50 bg-accent text-accent-fg shadow-[var(--glow-accent-soft)] hover:bg-accent/90 active:bg-accent/80",
        ghost:
          "border border-transparent text-fg-muted hover:bg-raised hover:text-fg active:bg-raised/60",
        danger: "border border-err/40 text-err hover:bg-err/10 active:bg-err/15",
        // The one action a screen exists for: joining a server. Stencilled
        // capitals on the accent, so it reads from across the room.
        play: [
          "border border-accent bg-accent text-accent-fg shadow-[var(--glow-accent-soft)]",
          "font-display font-extrabold uppercase tracking-[0.08em] hover:brightness-110 active:brightness-95",
        ],
      },
      size: {
        lg: "h-control-lg px-4 text-sm",
        sm: "h-control px-2.5 text-xs",
        xs: "h-control-sm px-1.5 text-2xs",
        icon: "size-control",
        "icon-xs": "size-control-sm",
      },
    },
    defaultVariants: { variant: "default", size: "sm" },
  });
</script>

<script lang="ts">
  import type { Snippet } from "svelte";
  import { Button } from "bits-ui";
  import { cn } from "$lib/cx";

  let {
    variant = "default",
    size = "sm",
    active = false,
    disabled = false,
    title = "",
    type = "button",
    class: klass = "",
    onclick,
    children,
    "aria-label": ariaLabel,
  }: {
    variant?: "default" | "accent" | "ghost" | "danger" | "play";
    size?: "lg" | "sm" | "xs" | "icon" | "icon-xs";
    /** Toggle state. Rendered as a filled accent so a latched control is
        distinguishable from a hovered one. */
    active?: boolean;
    disabled?: boolean;
    title?: string;
    /**
     * The accessible name, for a button whose whole content is an icon.
     *
     * `title` is a hint and not a name: a screen reader may read it, may
     * read it after the content, or may ignore it, and an icon-only button
     * has no content to fall back on. Wanted here rather than sprinkled
     * because every icon button in the app is one of these.
     */
    "aria-label"?: string;
    type?: "button" | "submit";
    class?: string;
    onclick?: (e: MouseEvent) => void;
    children: Snippet;
  } = $props();
</script>

<!-- bits-ui's Button: a `<button>`, or an `<a>` wearing the same clothes when
     it is given an `href`, so a link that should look like a button is not a
     second component. -->
<Button.Root
  {type}
  {title}
  {disabled}
  aria-label={ariaLabel}
  aria-pressed={active ? true : undefined}
  class={cn(button({ variant: active ? "accent" : variant, size }), klass)}
  {onclick}
>
  {@render children()}
</Button.Root>
