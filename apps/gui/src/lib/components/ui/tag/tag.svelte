<script lang="ts">
  import type { Snippet } from "svelte";
  import { cn } from "$lib/cx";

  let {
    tone = "neutral",
    class: klass = "",
    title = "",
    filled = false,
    children,
  }: {
    tone?: "neutral" | "ok" | "warn" | "err" | "accent";
    class?: string;
    title?: string;
    /** Fill the tone rather than tinting it. See `filledTones`. */
    filled?: boolean;
    children: Snippet;
  } = $props();

  const tones = {
    neutral: "bg-raised/70 text-fg-muted ring-1 ring-border/60",
    ok: "bg-ok/12 text-ok ring-1 ring-ok/25",
    warn: "bg-warn/12 text-warn ring-1 ring-warn/25",
    err: "bg-err/12 text-err ring-1 ring-err/25",
    accent: "bg-accent/12 text-accent ring-1 ring-accent/25",
  };

  /**
   * The same tones, filled.
   *
   * For the one place a pair of tags has to be told apart without relying on
   * hue: filled against outlined survives greyscale, and survives the
   * reduced-motion rule, where a colour difference plus a pulse does not.
   */
  const filledTones = {
    neutral: "bg-fg-muted text-bg",
    ok: "bg-ok text-bg",
    warn: "bg-warn text-bg",
    err: "bg-err text-bg",
    accent: "bg-accent text-accent-fg",
  };
</script>

<span
  {title}
  class={cn(
    "inline-flex items-center gap-1 rounded-sm px-1.5 py-px font-mono text-2xs leading-4 whitespace-nowrap",
    filled ? filledTones[tone] : tones[tone],
    klass,
  )}
>
  {@render children()}
</span>
