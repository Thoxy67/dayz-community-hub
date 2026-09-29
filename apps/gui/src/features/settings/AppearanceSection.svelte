<script lang="ts">
  import { useIntlayer } from "svelte-intlayer";
  import Palette from "~icons/lucide/palette";
  import Monitor from "~icons/lucide/monitor";
  import Check from "~icons/lucide/check";
  import Upload from "~icons/lucide/upload";
  import Download from "~icons/lucide/download";
  import ClipboardCopy from "~icons/lucide/clipboard-copy";
  import Pencil from "~icons/lucide/pencil";
  import { Button } from "$lib/components/ui/button";
  import { Select } from "$lib/components/ui/select";
  import { Tag } from "$lib/components/ui/tag";
  import { cn } from "$lib/cx";
  import { copyText } from "$lib/ipc/native";
  import { theme } from "$lib/theme/theme.svelte";
  import { PRESETS, TOKEN_NAMES, presetById, type Preset, type TokenName } from "$lib/theme/presets";
  import { say } from "$lib/stores/say";
  import { SettingsSection as Section } from "$lib/components/app";
  import { ColorToken } from "$lib/components/app";

  const s = useIntlayer("settings");
  const t = useIntlayer("theme");

  function label(id: string): string {
    const key = `preset${id.replace(/(^|_)(\w)/g, (_, __, c: string) => c.toUpperCase())}` as keyof typeof $t;
    return ($t[key] as { value?: string } | undefined)?.value ?? id;
  }

  const TOKEN_LABEL: Record<TokenName, () => string> = {
    bg: () => $s.tokenBg.value,
    panel: () => $s.tokenPanel.value,
    raised: () => $s.tokenRaised.value,
    fg: () => $s.tokenFg.value,
    accent: () => $s.tokenAccent.value,
    accentFg: () => $s.tokenAccentFg.value,
    ok: () => $s.tokenOk.value,
    warn: () => $s.tokenWarn.value,
    err: () => $s.tokenErr.value,
    info: () => $s.tokenInfo.value,
    map: () => $s.tokenMap.value,
    mods: () => $s.tokenMods.value,
  };

  const editing = $derived(theme.selected === "custom");
  let from = $state<string | null>(null);

  function startFrom(id: string | null) {
    const p = id ? presetById(id) : null;
    if (!p) return;
    theme.setCustom({ scheme: p.scheme, tokens: { ...p.tokens } });
    from = null;
  }

  async function copyJson() {
    await copyText(theme.exportJson());
    say.ok($s.themeCopied.value);
  }

  function download() {
    const url = URL.createObjectURL(new Blob([theme.exportJson()], { type: "application/json" }));
    const a = document.createElement("a");
    a.href = url;
    a.download = "dayz-community-hub-theme.json";
    a.click();
    URL.revokeObjectURL(url);
    say.ok($t.exported.value);
  }

  let fileInput: HTMLInputElement | undefined = $state();
  async function importFile(e: Event) {
    const file = (e.currentTarget as HTMLInputElement).files?.[0];
    if (!file) return;
    if (theme.importJson(await file.text())) say.ok($t.imported.value);
    else say.err($t.importError.value);
    (e.currentTarget as HTMLInputElement).value = "";
  }

  const presetOptions = PRESETS.map((p) => ({
    value: p.id,
    label: label(p.id),
    group: p.scheme === "dark" ? $t.dark.value : $t.light.value,
  }));
</script>

<!-- A preset as a tiny window in its own colours: rail, panel, a row, the accent. -->
{#snippet thumb(p: { tokens: Preset["tokens"] })}
  <span class="relative block h-16 overflow-hidden rounded-sm" style="background: {p.tokens.bg}" aria-hidden="true">
    <span class="absolute inset-y-0 left-0 w-1/4 border-r" style="border-color: {p.tokens.raised}">
      <span class="absolute top-2 left-1.5 h-1 w-2/3 rounded-full" style="background: {p.tokens.accent}"></span>
      <span class="absolute top-4.5 left-1.5 h-1 w-1/2 rounded-full opacity-50" style="background: {p.tokens.fg}"></span>
      <span class="absolute top-7 left-1.5 h-1 w-1/2 rounded-full opacity-50" style="background: {p.tokens.fg}"></span>
    </span>
    <span class="absolute inset-y-0 right-0 left-1/4" style="background: {p.tokens.panel}">
      {#each [0, 1, 2] as r (r)}
        <span class="absolute right-2 left-2 flex items-center gap-1" style="top: {8 + r * 14}px">
          <span class="h-1 flex-1 rounded-full opacity-70" style="background: {p.tokens.fg}"></span>
          <span class="h-1 w-3 rounded-full" style="background: {[p.tokens.ok, p.tokens.warn, p.tokens.err][r]}"></span>
        </span>
      {/each}
      <span class="absolute right-2 bottom-2 h-2.5 w-8 rounded-xs" style="background: {p.tokens.accent}"></span>
    </span>
  </span>
{/snippet}

{#snippet card(id: string | null, name: string, tokens: Preset["tokens"] | null, scheme: string, icon?: typeof Monitor)}
  {@const on = theme.selected === id}
  <button
    type="button"
    class={cn(
      "group flex flex-col gap-1.5 rounded-md border p-1.5 text-left transition-colors",
      on ? "border-accent bg-accent/8 shadow-glow" : "border-border hover:border-border-strong hover:bg-raised/40",
    )}
    onclick={() => theme.use(id)}
    aria-pressed={on}
  >
    {#if tokens}
      {@render thumb({ tokens })}
    {:else}
      <span class="grid h-16 grid-cols-2 overflow-hidden rounded-sm">
        {@render thumb({ tokens: presetById("chernarus")!.tokens })}
        {@render thumb({ tokens: presetById("topo")!.tokens })}
      </span>
    {/if}
    <span class="flex items-center gap-1 px-0.5">
      {#if icon}{@const I = icon}<I class="size-3 shrink-0 text-fg-faint" />{/if}
      <span class="min-w-0 flex-1 truncate text-2xs font-medium text-fg">{name}</span>
      {#if on}<Check class="size-3 shrink-0 text-accent" />{:else}<span class="font-mono text-3xs text-fg-faint">{scheme}</span>{/if}
    </span>
  </button>
{/snippet}

<Section id="appearance" title={$s.sectionAppearance.value} description={$s.followSystemHint.value} icon={Palette}>
  {#snippet aside()}
    <Tag tone="accent">{theme.selected === null ? $s.followSystem.value : theme.selected === "custom" ? $s.yourTheme.value : label(theme.selected)}</Tag>
  {/snippet}

  <div class="grid grid-cols-[repeat(auto-fill,minmax(9.5rem,1fr))] gap-2 p-2.5">
    {@render card(null, $s.followSystem.value, null, "auto", Monitor)}
    {#if theme.custom}{@render card("custom", $s.yourTheme.value, theme.custom.tokens, theme.custom.scheme, Pencil)}{/if}
    {#each PRESETS as p (p.id)}{@render card(p.id, label(p.id), p.tokens, p.scheme === "dark" ? $t.dark.value : $t.light.value)}{/each}
  </div>

  <!-- The editor: the twelve colours of the player's own theme. -->
  <div class="border-t border-border">
    <div class="flex flex-wrap items-center gap-2 px-pad py-2">
      <div class="min-w-0 flex-1">
        <p class="m-0 title-display text-lg text-fg">{$s.yourTheme.value}</p>
        <p class="m-0 text-2xs text-fg-faint">{$s.yourThemeHint.value}</p>
      </div>
      {#if !editing}
        <Button variant="accent" onclick={() => theme.beginCustom()}><Pencil class="size-icon-sm" />{$s.customize.value}</Button>
      {/if}
      <span class="flex items-center gap-1.5">
        <span class="text-2xs text-fg-faint">{$s.startFrom.value}</span>
        <Select bind:value={() => from, (v) => startFrom(v)} options={presetOptions} placeholder="—" aria-label={$s.startFrom.value} class="w-40" />
      </span>
      <Button onclick={copyJson}><ClipboardCopy class="size-icon-sm" />{$t.export.value}</Button>
      <Button variant="ghost" onclick={download}><Download class="size-icon-sm" />.json</Button>
      <Button variant="ghost" onclick={() => fileInput?.click()}><Upload class="size-icon-sm" />{$t.import.value}</Button>
      <input bind:this={fileInput} type="file" accept=".json,application/json" class="hidden" onchange={importFile} />
    </div>
    <div class={cn("grid grid-cols-2 gap-px border-t border-border bg-border/60", !editing && "pointer-events-none opacity-50")}>
      {#each TOKEN_NAMES as name (name)}
        <ColorToken value={theme.current.tokens[name]} label={TOKEN_LABEL[name]()} onchange={(v) => theme.setToken(name, v)} />
      {/each}
    </div>
  </div>
</Section>
