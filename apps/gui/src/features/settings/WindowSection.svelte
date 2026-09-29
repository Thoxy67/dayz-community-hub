<script lang="ts">
  import { useIntlayer } from "svelte-intlayer";
  import AppWindow from "~icons/lucide/app-window";
  import { Field } from "$lib/components/ui/field";
  import { Slider } from "$lib/components/ui/slider";
  import { theme } from "$lib/theme/theme.svelte";
  import { SettingsSection as Section } from "$lib/components/app";
  import { ColorToken } from "$lib/components/app";

  const s = useIntlayer("settings");
  const frame = $derived(theme.frame);
</script>

<Section id="window" title={$s.sectionWindow.value} description={$s.windowHint.value} icon={AppWindow}>
  <Field label={$s.windowRadius.value}>
    <Slider
      value={frame.radius}
      onchange={(v) => theme.setFrame({ radius: v })}
      min={0}
      max={16}
      step={1}
      aria-label={$s.windowRadius.value}
      class="flex-1"
    />
    <span class="num w-12 text-right font-mono text-xs text-fg">{frame.radius} px</span>
  </Field>
  <Field label={$s.windowBorder.value}>
    <Slider
      value={frame.border}
      onchange={(v) => theme.setFrame({ border: v })}
      min={0}
      max={4}
      step={1}
      aria-label={$s.windowBorder.value}
      class="flex-1"
    />
    <span class="num w-12 text-right font-mono text-xs text-fg">{frame.border} px</span>
  </Field>
  {#if frame.border > 0}
    <div class="grid grid-cols-2 gap-px bg-border/60">
      <ColorToken value={frame.borderFocus} label={$s.windowBorderFocus.value} onchange={(v) => theme.setFrame({ borderFocus: v })} />
      <ColorToken value={frame.borderBlur} label={$s.windowBorderBlur.value} onchange={(v) => theme.setFrame({ borderBlur: v })} />
    </div>
  {/if}
</Section>
