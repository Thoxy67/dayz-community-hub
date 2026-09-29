<script lang="ts">
  import { hexToOklch, oklchToHex, parseOklch } from "$lib/theme/oklch";

  /**
   * One of the theme's twelve colours: the system colour picker on the
   * swatch, and the exact OKLCH value beside it for whoever wants to type it.
   */
  let {
    value,
    label,
    onchange,
  }: { value: string; label: string; onchange: (value: string) => void } = $props();

  const hex = $derived.by(() => {
    const c = parseOklch(value);
    return c ? oklchToHex(c) : value.startsWith("#") ? value : "#000000";
  });

  function fromHex(h: string) {
    const c = hexToOklch(h);
    onchange(`oklch(${(c.l * 100).toFixed(1)}% ${c.c.toFixed(3)} ${Math.round(c.h)})`);
  }

  let draft = $state("");
  $effect(() => {
    draft = value;
  });
</script>

<label class="flex items-center gap-2 bg-bg px-2 py-1.5">
  <span
    class="relative size-7 shrink-0 overflow-hidden rounded-sm border border-border-strong"
    style="background: {value}"
  >
    <input
      type="color"
      value={hex}
      oninput={(e) => fromHex(e.currentTarget.value)}
      class="absolute inset-0 size-full cursor-pointer opacity-0"
      aria-label={label}
    />
  </span>
  <span class="w-28 shrink-0 truncate text-2xs text-fg-muted">{label}</span>
  <input
    bind:value={draft}
    onchange={() => (CSS.supports("color", draft) ? onchange(draft.trim()) : (draft = value))}
    spellcheck="false"
    class="h-control-sm min-w-0 flex-1 rounded-xs border border-transparent bg-transparent px-1 font-mono text-2xs text-fg
           hover:border-border focus:border-accent/60 focus:outline-none"
  />
</label>
