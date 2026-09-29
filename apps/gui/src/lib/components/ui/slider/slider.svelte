<script lang="ts">
  import { Slider } from "bits-ui";
  import { cn } from "$lib/cx";

  /**
   * A number chosen along a line: bits-ui's Slider.
   *
   * In place of the engine's own range input, which is drawn by the platform
   * and ignored most of the theme: WebKitGTK and WebView2 disagreed about its
   * height, and the stylesheet carried eight rules to make the two agree.
   * This one is made of elements, so it is the same everywhere and the theme
   * reaches all of it. Arrows, Page Up/Down, Home and End are bits-ui's.
   */
  let {
    value = $bindable(),
    min = 0,
    max = 1,
    step = 0.01,
    disabled = false,
    onchange,
    oncommit,
    class: klass = "",
    "aria-label": ariaLabel,
  }: {
    value: number;
    min?: number;
    max?: number;
    step?: number;
    disabled?: boolean;
    /** Told on every move, for a caller that owns the writing. */
    onchange?: (value: number) => void;
    /** Told once, when the thumb is let go: for work too heavy for every move. */
    oncommit?: (value: number) => void;
    class?: string;
    "aria-label"?: string;
  } = $props();
</script>

<Slider.Root
  type="single"
  bind:value={
    () => value,
    (v) => {
      if (v === value) return;
      if (onchange) onchange(v);
      else value = v;
    }
  }
  onValueCommit={(v: number) => oncommit?.(v)}
  {min}
  {max}
  {step}
  {disabled}
  class={cn(
    "relative flex h-control-sm w-full min-w-0 touch-none items-center select-none",
    disabled && "cursor-not-allowed opacity-40",
    klass,
  )}
>
  {#snippet children({ thumbItems })}
    <span class="relative h-1 w-full grow overflow-hidden rounded-full bg-raised">
      <Slider.Range class="absolute h-full rounded-full bg-accent/70" />
    </span>
    {#each thumbItems as { index } (index)}
      <Slider.Thumb
        {index}
        aria-label={ariaLabel}
        class="block size-3 cursor-grab rounded-full border border-accent bg-accent shadow-[var(--glow-accent-soft)]
               focus-visible:ring-2 focus-visible:ring-accent/50 focus-visible:outline-none
               active:cursor-grabbing data-[disabled]:cursor-not-allowed"
      />
    {/each}
  {/snippet}
</Slider.Root>
