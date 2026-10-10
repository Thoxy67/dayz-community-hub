<script lang="ts">
  import { dict } from "$lib/i18n";
  import Moon from "~icons/lucide/moon";
  import Sun from "~icons/lucide/sun";
  import WandSparkles from "~icons/lucide/wand-sparkles";
  import RotateCcw from "~icons/lucide/rotate-ccw";
  import Trash from "~icons/lucide/trash-2";
  import ClipboardCopy from "~icons/lucide/clipboard-copy";
  import Download from "~icons/lucide/download";
  import TriangleAlert from "~icons/lucide/triangle-alert";
  import Check from "~icons/lucide/check";
  import { Button, IconButton } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import { Segmented } from "$lib/components/ui/segmented";
  import { Tooltip } from "$lib/components/ui/tooltip";
  import { ColorToken } from "$lib/components/app";
  import { cn } from "$lib/cx";
  import { copyText } from "$lib/ipc/native";
  import { say } from "$lib/stores/say";
  import { theme, type CustomTheme } from "$lib/theme/theme.svelte";
  import { TOKEN_NAMES, type Scheme, type TokenName } from "$lib/theme/presets";
  import { audit, confusable } from "$lib/theme/contrast";
  import { deriveTheme } from "$lib/theme/derive";

  /**
   * The worn custom theme, open for editing: its name and scheme, the twelve
   * colours, and beside them the legibility checks the shipped presets are
   * tested against (`lib/theme/contrast.ts`), live.
   */
  let { custom }: { custom: CustomTheme } = $props();

  const s = dict("settings");
  const t = dict("theme");

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

  /** What each check is called: what is drawn, and on what. */
  function checkLabel(id: string): string {
    const on = id.endsWith("Panel")
      ? $t.onPanel.value
      : id === "fgRaised"
        ? $t.onControl.value
        : $t.onFloor.value;
    if (id.startsWith("fg")) return `${$t.checkText.value} ${on}`;
    if (id.startsWith("muted")) return `${$t.checkMuted.value} ${on}`;
    if (id.startsWith("faint")) return `${$t.checkFaint.value} ${on}`;
    if (id === "accentFg") return $t.checkAccentLabel.value;
    return `${TOKEN_LABEL[id as TokenName]?.() ?? id} ${$t.onPanel.value}`;
  }

  const checks = $derived(audit(custom.tokens));
  const alike = $derived(confusable(custom.tokens));
  const failing = $derived(checks.filter((c) => !c.pass).length + alike.length);

  let name = $state("");
  $effect(() => {
    name = custom.name;
  });

  function derive() {
    const d = deriveTheme(custom.tokens.bg, custom.tokens.accent);
    theme.setCustom({ scheme: d.scheme, tokens: d.tokens });
  }

  async function copyJson() {
    await copyText(theme.exportJson(custom.id));
    say.ok($s.themeCopied.value);
  }

  function download() {
    const url = URL.createObjectURL(
      new Blob([theme.exportJson(custom.id)], { type: "application/json" }),
    );
    const a = document.createElement("a");
    a.href = url;
    a.download = `${custom.name.replace(/[^\w-]+/g, "-").toLowerCase() || "theme"}.dzch-theme.json`;
    a.click();
    URL.revokeObjectURL(url);
    say.ok($t.exported.value);
  }
</script>

<div class="border-t border-border">
  <div class="flex flex-wrap items-center gap-2 px-pad py-2">
    <Input
      bind:value={name}
      onchange={() => theme.rename(custom.id, name)}
      aria-label={$t.name.value}
      placeholder={$t.untitled.value}
      class="w-44"
    />
    <Segmented
      value={custom.scheme}
      onchange={(v: Scheme) => theme.setScheme(v)}
      options={[
        { value: "dark", label: $t.dark.value, icon: Moon },
        { value: "light", label: $t.light.value, icon: Sun },
      ]}
      aria-label={$t.scheme.value}
    />
    <span class="flex-1"></span>
    <Tooltip text={$t.deriveHint.value} side="bottom">
      <Button onclick={derive}><WandSparkles class="size-icon-sm" />{$t.derive.value}</Button>
    </Tooltip>
    {#if custom.base}
      <IconButton
        icon={RotateCcw}
        label={$t.resetHint.value}
        onclick={() => theme.reset(custom.id)}
      />
    {/if}
    <IconButton icon={ClipboardCopy} label={$t.export.value} onclick={copyJson} />
    <IconButton icon={Download} label="{$t.export.value} (.json)" onclick={download} />
    <IconButton
      icon={Trash}
      label={$t.remove.value}
      hoverTone="danger"
      onclick={() => theme.remove(custom.id)}
    />
  </div>

  <div class="border-t border-border">
    <div class="grid grid-cols-2 gap-px bg-border/60">
      {#each TOKEN_NAMES as tk (tk)}
        <ColorToken
          value={custom.tokens[tk]}
          label={TOKEN_LABEL[tk]()}
          onchange={(v) => theme.setToken(tk, v)}
        />
      {/each}
    </div>

    <!-- The checks: each pair drawn in its own colours, its ratio, its floor. -->
    <div class="border-t border-border bg-bg px-pad py-2" aria-live="polite">
      <p class="m-0 flex items-center gap-1.5 text-2xs font-medium text-fg">
        {$t.legibility.value}
        <span class="flex-1"></span>
        {#if failing}
          <span class="flex items-center gap-1 text-warn"
            ><TriangleAlert class="size-icon-sm" />{failing}</span
          >
        {:else}
          <Check class="size-icon-sm text-ok" />
        {/if}
      </p>
      <p class="m-0 mb-1 text-3xs text-fg-faint">{$t.legibilityHint.value}</p>
      <ul class="m-0 grid list-none grid-cols-1 gap-x-6 p-0 sm:grid-cols-2">
        {#each checks as c (c.id)}
          <li class="flex h-5 items-center gap-1.5 text-3xs">
            <span
              class="grid h-4 w-6 shrink-0 place-items-center rounded-xs font-semibold"
              style="background: {c.bg}; color: {c.fg}">Aa</span
            >
            <span class={cn("min-w-0 flex-1 truncate", c.pass ? "text-fg-muted" : "text-warn")}
              >{checkLabel(c.id)}</span
            >
            <span class={cn("num font-mono", c.pass ? "text-fg-muted" : "text-warn")}
              >{(Math.floor(c.ratio * 10) / 10).toFixed(1)}</span
            >
            <span class="num w-7 font-mono text-fg-faint">/{c.min}</span>
          </li>
        {/each}
        {#each alike as [a, b] (a + b)}
          <li class="flex h-5 items-center gap-1.5 text-3xs text-warn">
            <span class="flex h-4 w-6 shrink-0 overflow-hidden rounded-xs">
              <span class="flex-1" style="background: {custom.tokens[a as TokenName]}"></span>
              <span class="flex-1" style="background: {custom.tokens[b as TokenName]}"></span>
            </span>
            <span class="min-w-0 flex-1 truncate"
              >{$t.tooAlike.value}: {TOKEN_LABEL[a as TokenName]()} · {TOKEN_LABEL[
                b as TokenName
              ]()}</span
            >
          </li>
        {/each}
      </ul>
    </div>
  </div>
</div>
