<script lang="ts">
  import { dict } from "$lib/i18n";
  import Gamepad from "~icons/lucide/gamepad-2";
  import Keyboard from "~icons/lucide/keyboard";
  import { Field } from "$lib/components/ui/field";
  import { Segmented } from "$lib/components/ui/segmented";
  import { Tag } from "$lib/components/ui/tag";
  import { PadGlyph, PadLegend, SettingsSection as Section } from "$lib/components/app";
  import { cn } from "$lib/cx";
  import { pad } from "$lib/gamepad";
  import type { GlyphChoice } from "$lib/gamepad/state.svelte";
  import type { PadAction } from "$lib/ipc/gamepad";
  import type { PadButton } from "$lib/shortcuts/registry";
  import { prefs, type PadMode } from "$lib/stores/prefs.svelte";

  /**
   * A controller: when the interface switches to it, how large it is drawn
   * then, what is connected, which labels to draw, a live test of the
   * buttons, and what they do (the shortcut registry's controller list).
   */
  const p = dict("pad");

  const GLYPHS: { value: GlyphChoice; label: string }[] = $derived([
    { value: "auto", label: $p.modeAuto.value },
    { value: "xbox", label: "Xbox" },
    { value: "playStation", label: "PlayStation" },
    { value: "nintendo", label: "Nintendo" },
    { value: "steamDeck", label: "Steam Deck" },
  ]);

  // ── the live test: the button behind the last action lights a moment ──
  const FROM: Record<PadAction, PadButton> = {
    up: "dpad",
    down: "dpad",
    left: "dpad",
    right: "dpad",
    accept: "a",
    back: "b",
    primary: "x",
    secondary: "y",
    menu: "menu",
    view: "view",
    prevTab: "lb",
    nextTab: "rb",
    pageUp: "lt",
    pageDown: "rt",
    scrollUp: "rs",
    scrollDown: "rs",
    leftStick: "l3",
    rightStick: "r3",
  };
  const TEST: PadButton[] = [
    "dpad",
    "a",
    "b",
    "x",
    "y",
    "lb",
    "rb",
    "lt",
    "rt",
    "menu",
    "view",
    "l3",
    "r3",
    "rs",
  ];
  let lit = $state<PadButton | null>(null);
  $effect(() => {
    const last = pad.last;
    if (!last) return;
    lit = FROM[last.action];
    const t = setTimeout(() => (lit = null), 450);
    return () => clearTimeout(t);
  });

  const MODES = $derived([
    { value: "auto" as PadMode, label: $p.modeAuto.value },
    { value: "always" as PadMode, label: $p.modeAlways.value },
    { value: "never" as PadMode, label: $p.modeNever.value },
  ]);
  const SCALES = [
    { value: "1", label: "100 %" },
    { value: "1.15", label: "115 %" },
    { value: "1.3", label: "130 %" },
  ] as const;
</script>

<Section id="controller" title={$p.section.value} description={$p.sectionHint.value} icon={Gamepad}>
  {#snippet aside()}
    <Tag tone={pad.mode === "gamepad" ? "accent" : "neutral"}
      >{pad.pads.length > 0 ? $p.connected.value : $p.noPad.value}</Tag
    >
  {/snippet}

  <Field label={$p.mode.value} hint={$p.modeHint.value}>
    <Segmented
      value={prefs.padMode}
      options={MODES}
      onchange={(v) => (prefs.padMode = v)}
      aria-label={$p.mode.value}
    />
  </Field>
  <Field label={$p.scale.value}>
    <Segmented
      value={String(prefs.padScale)}
      options={SCALES}
      onchange={(v) => (prefs.padScale = Number(v))}
      aria-label={$p.scale.value}
    />
  </Field>
  <Field label={$p.glyphs.value} hint={$p.glyphsHint.value}>
    <Segmented
      value={pad.glyphs}
      options={GLYPHS}
      onchange={(v) => (pad.glyphs = v)}
      aria-label={$p.glyphs.value}
    />
  </Field>
  <Field label={$p.connected.value}>
    <div class="flex min-w-0 flex-col gap-1">
      {#each pad.pads as d, i (i)}
        <span class="flex min-w-0 items-center gap-1.5 text-xs text-fg">
          <Gamepad class="size-icon-sm shrink-0 text-accent" />
          <span class="truncate">{d.name}</span>
        </span>
      {:else}
        <span class="text-xs text-fg-faint"
          >{pad.available ? $p.noPad.value : $p.unavailable.value}</span
        >
      {/each}
      {#if pad.steamUi}
        <span class="flex items-center gap-1.5 text-2xs text-fg-muted">
          <Keyboard class="size-icon-sm shrink-0" />{$p.steamKeyboard.value}
        </span>
      {/if}
    </div>
  </Field>
  <Field label={$p.test.value} hint={$p.testHint.value}>
    <div class="flex flex-wrap items-center gap-1.5" aria-live="polite">
      {#each TEST as b (b)}
        <span
          class={cn(
            "rounded-full p-0.5 transition-colors",
            lit === b ? "bg-accent/40" : "bg-transparent",
          )}
        >
          <PadGlyph button={b} kind={pad.kind} />
        </span>
      {/each}
    </div>
  </Field>
  <Field label={$p.legend.value} stacked>
    <PadLegend class="block columns-2 gap-x-6 space-y-3 max-[900px]:columns-1" />
  </Field>
</Section>
