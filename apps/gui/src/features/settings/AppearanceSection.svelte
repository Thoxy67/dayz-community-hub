<script lang="ts">
  import { dict } from "$lib/i18n";
  import Palette from "~icons/lucide/palette";
  import Monitor from "~icons/lucide/monitor";
  import Moon from "~icons/lucide/moon";
  import Sun from "~icons/lucide/sun";
  import Check from "~icons/lucide/check";
  import Copy from "~icons/lucide/copy";
  import Upload from "~icons/lucide/upload";
  import { Button } from "$lib/components/ui/button";
  import { Select } from "$lib/components/ui/select";
  import { Tag } from "$lib/components/ui/tag";
  import { cn } from "$lib/cx";
  import { theme, PRESETS } from "$lib/theme/theme.svelte";
  import { presetName } from "$lib/theme/names";
  import type { Preset, Scheme, ThemeTokens } from "$lib/theme/presets";
  import { say } from "$lib/stores/say";
  import { SettingsSection as Section } from "$lib/components/app";
  import ThemePreview from "$lib/components/app/ThemePreview.svelte";
  import ThemeEditor from "./ThemeEditor.svelte";

  /**
   * Appearance: "follow the system" and the pair it switches between, every
   * preset drawn in its own colours (dark, light), the player's own themes,
   * and the editor for the one worn.
   */
  const s = dict("settings");
  const t = dict("theme");

  const dark = PRESETS.filter((p) => p.scheme === "dark");
  const light = PRESETS.filter((p) => p.scheme === "light");
  const blurb = (p: Preset) =>
    ($t as unknown as Record<string, { value: string } | undefined>)[p.blurb]?.value ?? "";
  const pairOptions = (scheme: Scheme) =>
    (scheme === "dark" ? dark : light).map((p) => ({ value: p.id, label: presetName(p.id, $t) }));

  const wornName = $derived(
    theme.selected === null
      ? $t.followSystem.value
      : (theme.custom?.name ?? presetName(theme.selected, $t)),
  );

  let fileInput: HTMLInputElement | undefined = $state();
  async function importFile(e: Event) {
    const input = e.currentTarget as HTMLInputElement;
    const file = input.files?.[0];
    if (!file) return;
    const fallback = file.name.replace(/(\.dzch-theme)?\.json$/i, "") || $t.untitled.value;
    if (theme.importJson(await file.text(), fallback)) say.ok($t.imported.value);
    else say.err($t.importError.value);
    input.value = "";
  }
</script>

{#snippet card(id: string, name: string, tokens: ThemeTokens, sub: string)}
  {@const on = theme.worn === id}
  <button
    type="button"
    class={cn(
      "flex min-w-0 flex-col gap-1.5 rounded-md border p-1.5 text-left transition-colors duration-fast",
      on
        ? "border-accent bg-accent/8 shadow-glow"
        : "border-border hover:border-border-strong hover:bg-raised/40",
    )}
    onclick={() => theme.use(id)}
    aria-pressed={on}
  >
    <ThemePreview {tokens} />
    <span class="flex min-w-0 flex-col px-0.5">
      <span class="flex items-center gap-1">
        <span class="min-w-0 flex-1 truncate text-2xs font-medium text-fg">{name}</span>
        {#if on}<Check class="size-3 shrink-0 text-accent" />{/if}
      </span>
      <span class="line-clamp-2 min-h-[2.4em] text-3xs leading-tight text-fg-faint">{sub}</span>
    </span>
  </button>
{/snippet}

{#snippet group(label: string, icon: typeof Moon)}
  {@const I = icon}
  <p
    class="m-0 flex items-center gap-1.5 px-pad pt-2.5 pb-1 font-mono text-2xs tracking-[0.08em] text-fg-faint uppercase"
  >
    <I class="size-icon-sm" />{label}
  </p>
{/snippet}

<Section
  id="appearance"
  title={$s.sectionAppearance.value}
  description={$t.followSystemHint.value}
  icon={Palette}
>
  {#snippet aside()}
    <Tag tone="accent">{wornName}</Tag>
  {/snippet}

  <!-- Follow the system, and which dark and light preset that means. -->
  <div class="flex flex-wrap items-center gap-x-4 gap-y-2 px-pad py-2.5">
    <button
      type="button"
      class={cn(
        "flex h-control items-center gap-1.5 rounded-md border px-2 text-xs transition-colors duration-fast",
        theme.selected === null
          ? "border-accent bg-accent/8 text-fg shadow-glow"
          : "border-border text-fg-muted hover:border-border-strong hover:text-fg",
      )}
      aria-pressed={theme.selected === null}
      onclick={() => theme.use(null)}
    >
      <Monitor class="size-icon-sm" />
      {$t.followSystem.value}
      {#if theme.selected === null}<Check class="size-3 text-accent" />{/if}
    </button>
    {#each ["dark", "light"] as const as scheme (scheme)}
      {@const I = scheme === "dark" ? Moon : Sun}
      <span class="flex items-center gap-1.5">
        <I class={cn("size-icon-sm", theme.os === scheme ? "text-accent" : "text-fg-faint")} />
        <span class="text-2xs text-fg-faint"
          >{scheme === "dark" ? $t.whenDark.value : $t.whenLight.value}</span
        >
        <Select
          bind:value={() => theme.pair[scheme], (v) => v && theme.setPair(scheme, v)}
          options={pairOptions(scheme)}
          aria-label={scheme === "dark" ? $t.whenDark.value : $t.whenLight.value}
          class="w-40"
        />
      </span>
    {/each}
  </div>

  <div class="border-t border-border pb-2.5">
    {@render group($t.dark.value, Moon)}
    <div class="grid grid-cols-[repeat(auto-fill,minmax(9.5rem,1fr))] gap-2 px-pad">
      {#each dark as p (p.id)}{@render card(p.id, presetName(p.id, $t), p.tokens, blurb(p))}{/each}
    </div>
    {@render group($t.light.value, Sun)}
    <div class="grid grid-cols-[repeat(auto-fill,minmax(9.5rem,1fr))] gap-2 px-pad">
      {#each light as p (p.id)}{@render card(p.id, presetName(p.id, $t), p.tokens, blurb(p))}{/each}
    </div>
  </div>

  <!-- The player's own themes. -->
  <div class="border-t border-border pb-2.5">
    <div class="flex flex-wrap items-center gap-2 px-pad pt-2.5 pb-1">
      <div class="min-w-0 flex-1">
        <p class="m-0 title-display text-lg text-fg">{$t.yourThemes.value}</p>
        <p class="m-0 text-2xs text-fg-faint">{$s.yourThemeHint.value}</p>
      </div>
      {#if !theme.custom}
        <Button variant="accent" onclick={() => theme.duplicate()}
          ><Copy class="size-icon-sm" />{$t.duplicate.value}</Button
        >
      {/if}
      <Button variant="ghost" onclick={() => fileInput?.click()}
        ><Upload class="size-icon-sm" />{$t.import.value}</Button
      >
      <input
        bind:this={fileInput}
        type="file"
        accept=".json,application/json"
        class="hidden"
        onchange={importFile}
      />
    </div>
    {#if theme.customs.length}
      <div class="grid grid-cols-[repeat(auto-fill,minmax(9.5rem,1fr))] gap-2 px-pad pt-1">
        {#each theme.customs as c (c.id)}{@render card(
            c.id,
            c.name,
            c.tokens,
            c.scheme === "dark" ? $t.dark.value : $t.light.value,
          )}{/each}
      </div>
    {:else}
      <p class="m-0 px-pad text-2xs text-fg-muted">{$t.yourThemesEmpty.value}</p>
    {/if}
  </div>

  {#if theme.custom}<ThemeEditor custom={theme.custom} />{/if}
</Section>
