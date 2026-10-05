<script lang="ts">
  import { dict } from "$lib/i18n";
  import { pad } from "$lib/gamepad";
  import PadGlyph, { type PadButton } from "./PadGlyph.svelte";

  /**
   * What a controller's buttons do, in the glyphs of the one plugged in:
   * the moves everywhere, then `extra` rows (a view's own X and Y); with
   * `only`, the extra rows alone.
   */
  let {
    extra = [],
    only = false,
  }: { extra?: { buttons: PadButton[]; text: string }[]; only?: boolean } = $props();
  const p = dict("pad");

  const base = (): { buttons: PadButton[] | "dpad"; text: string }[] => [
    { buttons: "dpad", text: $p.legendMove.value },
    { buttons: ["a"], text: $p.legendSelect.value },
    { buttons: ["b"], text: $p.legendBack.value },
    { buttons: ["x", "y"], text: $p.legendXY.value },
    { buttons: ["lb", "rb"], text: $p.legendTabs.value },
    { buttons: ["lt", "rt"], text: $p.legendPage.value },
    { buttons: ["menu", "view"], text: $p.legendMenu.value },
    ...extra,
  ];
  const rows = $derived<{ buttons: PadButton[] | "dpad"; text: string }[]>(only ? extra : base());
</script>

<dl class="m-0 grid w-full grid-cols-[auto_minmax(0,1fr)] items-center gap-x-3 gap-y-1.5">
  {#each rows as l (l.text)}
    <dt class="flex items-center gap-1">
      {#if l.buttons === "dpad"}
        <span
          class="inline-grid h-4.5 min-w-6 place-items-center rounded-sm border border-border-strong bg-raised px-1 font-mono text-3xs font-bold text-fg-muted"
          aria-hidden="true">✚</span
        >
      {:else}
        {#each l.buttons as b (b)}<PadGlyph button={b} kind={pad.kind} />{/each}
      {/if}
    </dt>
    <dd class="m-0 text-xs text-fg-muted">{l.text}</dd>
  {/each}
</dl>
{#if !only}
  <p class="m-0 mt-2 text-2xs leading-snug text-fg-faint">{$p.legendSticks.value}</p>
{/if}
