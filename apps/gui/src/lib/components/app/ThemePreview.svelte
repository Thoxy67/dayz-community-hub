<script lang="ts">
  import type { ThemeTokens } from "$lib/theme/presets";
  import { cn } from "$lib/cx";

  /**
   * A theme drawn small in its own colours, so choosing one is looking, not
   * reading: a title bar with the accent mark, two server rows (the second
   * selected) each with a ping signal, a players bar and a map name, and the
   * accent's play button. Every colour comes from `tokens`; the derived ones
   * (muted text, borders, the selected tint) are mixed the way
   * `styles/tokens.css` mixes them.
   */
  let { tokens, class: klass = "" }: { tokens: ThemeTokens; class?: string } = $props();

  const mix = (a: string, p: number, b: string) => `color-mix(in oklch, ${a} ${p}%, ${b})`;
  const border = $derived(mix(tokens.raised, 72, `${tokens.fg} 12%`));
  const muted = $derived(mix(tokens.fg, 68, tokens.bg));
  const faint = $derived(mix(tokens.fg, 50, tokens.bg));
  const rows = $derived([
    { ping: tokens.ok, bars: 3, fill: 70, map: tokens.map, mods: false },
    { ping: tokens.warn, bars: 2, fill: 35, map: tokens.map, mods: true },
  ]);
</script>

<span
  class={cn("flex h-18 flex-col overflow-hidden rounded-sm", klass)}
  style="background: {tokens.bg}; box-shadow: inset 0 0 0 1px {border}"
  aria-hidden="true"
>
  <!-- title bar -->
  <span
    class="flex h-3.5 shrink-0 items-center gap-1 px-1.5"
    style="border-bottom: 1px solid {border}"
  >
    <span class="h-1.5 w-1.5 rounded-xs" style="background: {tokens.accent}"></span>
    <span class="h-1 w-8 rounded-full" style="background: {tokens.fg}"></span>
    <span class="ml-auto h-1 w-4 rounded-full" style="background: {faint}"></span>
  </span>
  <span class="flex min-h-0 flex-1">
    <!-- rail -->
    <span
      class="flex w-1/5 shrink-0 flex-col gap-1 px-1 pt-1.5"
      style="border-right: 1px solid {border}"
    >
      <span class="h-1 w-full rounded-full" style="background: {tokens.accent}"></span>
      <span class="h-1 w-2/3 rounded-full" style="background: {muted}"></span>
      <span class="h-1 w-3/4 rounded-full" style="background: {muted}"></span>
    </span>
    <!-- list -->
    <span class="flex min-w-0 flex-1 flex-col" style="background: {tokens.panel}">
      {#each rows as r, i (i)}
        <span
          class="flex h-4 items-center gap-1 px-1"
          style="border-bottom: 1px solid {border}; {i === 1
            ? `background: ${mix(tokens.accent, 12, 'transparent')}; box-shadow: inset 2px 0 0 ${tokens.accent}`
            : ''}"
        >
          <span class="flex h-1.5 items-end gap-px">
            {#each [1, 2, 3] as b (b)}
              <span
                class="w-px"
                style="height: {b * 2}px; background: {b <= r.bars ? r.ping : border}"
              ></span>
            {/each}
          </span>
          <span class="h-1 w-4 overflow-hidden rounded-full" style="background: {tokens.raised}">
            <span class="block h-full" style="width: {r.fill}%; background: {tokens.accent}"></span>
          </span>
          <span class="h-1 min-w-0 flex-1 rounded-full" style="background: {tokens.fg}"></span>
          <span class="h-1 w-3 rounded-full" style="background: {r.map}"></span>
          {#if r.mods}<span class="h-1 w-1.5 rounded-full" style="background: {tokens.mods}"
            ></span>{/if}
        </span>
      {/each}
      <span class="mt-auto flex items-center gap-1 px-1 pb-1">
        <span class="h-1 w-2 rounded-full" style="background: {tokens.err}"></span>
        <span class="h-1 w-2 rounded-full" style="background: {tokens.info}"></span>
        <span
          class="ml-auto flex h-2.5 w-7 items-center justify-center rounded-xs"
          style="background: {tokens.accent}"
        >
          <span class="h-0.5 w-3 rounded-full" style="background: {tokens.accentFg}"></span>
        </span>
      </span>
    </span>
  </span>
</span>
