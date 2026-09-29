<script lang="ts">
  import type { Snippet } from "svelte";
  import { cn } from "$lib/cx";

  // A run of controls about one thing, with a word saying what. The word is
  // the point: seven icons in a row are seven guesses, and "LAYERS" in front
  // of five of them is one answer. It gives way first when the bar is narrow.
  let {
    label,
    class: klass = "",
    children,
  }: {
    label: string;
    class?: string;
    children: Snippet;
  } = $props();
</script>

<div role="group" aria-label={label} class={cn("flex shrink-0 items-center gap-1.5", klass)}>
  <span
    aria-hidden="true"
    class="hidden font-mono text-3xs tracking-[0.1em] text-fg-faint uppercase select-none @[980px]:inline"
  >
    {label}
  </span>
  {@render children()}
</div>
