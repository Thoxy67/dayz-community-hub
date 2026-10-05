<script lang="ts">
  import { dict } from "$lib/i18n";
  import Gamepad from "~icons/lucide/gamepad-2";
  import Keyboard from "~icons/lucide/keyboard";
  import { Field } from "$lib/components/ui/field";
  import { Segmented } from "$lib/components/ui/segmented";
  import { Tag } from "$lib/components/ui/tag";
  import { PadLegend, SettingsSection as Section } from "$lib/components/app";
  import { pad } from "$lib/gamepad";
  import { prefs, type PadMode } from "$lib/stores/prefs.svelte";

  /**
   * A controller: when the interface switches to it, how large it is drawn
   * then, what is connected, and what the buttons do.
   */
  const p = dict("pad");

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
  <Field label={$p.legend.value} stacked>
    <PadLegend />
  </Field>
</Section>
