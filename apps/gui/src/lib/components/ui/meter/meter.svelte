<script lang="ts">
  import { Meter } from "bits-ui";
  import { cn } from "$lib/cx";

  /**
   * A bar that is some fraction full: bits-ui's Meter.
   *
   * Eighteen of these were drawn in place as a `div` with a percentage width,
   * in three shapes, and none of them said anything to a screen reader.
   *
   * The fill is scaled, not resized: a width change is a layout of everything
   * beside the bar on every tick of a countdown, and a transform is not.
   */
  let {
    value,
    max = 1,
    tone = "accent",
    size = "sm",
    label,
    animate = true,
    class: klass = "",
  }: {
    value: number;
    max?: number;
    tone?: "accent" | "ok" | "warn" | "err" | "muted" | "in" | "out";
    size?: "xs" | "sm" | "md";
    /** What is being measured. Without it the bar is decoration and says so. */
    label?: string;
    animate?: boolean;
    class?: string;
  } = $props();

  const held = $derived(Math.min(max, Math.max(0, value)));
  const fraction = $derived(max > 0 ? held / max : 0);

  const tones = {
    accent: "bg-accent",
    ok: "bg-ok",
    warn: "bg-warn",
    err: "bg-err",
    muted: "bg-fg-faint",
    in: "bg-info",
    out: "bg-warn",
  };
  const sizes = { xs: "h-0.5", sm: "h-1", md: "h-1.5" };
</script>

<Meter.Root
  value={held}
  min={0}
  {max}
  aria-label={label}
  aria-hidden={label ? undefined : true}
  class={cn("w-full overflow-hidden rounded-full bg-raised", sizes[size], klass)}
>
  <div
    class={cn(
      "h-full w-full origin-left rounded-full",
      tones[tone],
      animate && "transition-transform duration-(--duration-base) ease-out",
    )}
    style:transform="scaleX({fraction})"
  ></div>
</Meter.Root>
