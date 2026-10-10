<script lang="ts">
  import { dict } from "$lib/i18n";
  import Monitor from "~icons/lucide/monitor";
  import Check from "~icons/lucide/check";
  import { theme, PRESETS } from "$lib/theme/theme.svelte";
  import { presetName } from "$lib/theme/names";
  import type { Preset, ThemeTokens } from "$lib/theme/presets";
  import { cn } from "$lib/cx";
  import ThemePreview from "./ThemePreview.svelte";

  /**
   * Every preset as a small window in its own colours, dark ones first, plus
   * "follow the system" (drawn as its dark and light halves). Choosing one
   * wears it at once.
   */
  let {
    systemLabel,
    columns = 4,
    class: klass = "",
  }: { systemLabel: string; columns?: 3 | 4 | 5 | 6; class?: string } = $props();

  const t = dict("theme");
  const ordered = [...PRESETS].sort((a, b) =>
    a.scheme === b.scheme ? 0 : a.scheme === "dark" ? -1 : 1,
  );
  const cols = { 3: "grid-cols-3", 4: "grid-cols-4", 5: "grid-cols-5", 6: "grid-cols-6" } as const;
  const blurb = (p: Preset) =>
    ($t as unknown as Record<string, { value: string } | undefined>)[p.blurb]?.value ?? "";
  const sys = $derived({
    dark: theme.look(theme.pair.dark).tokens,
    light: theme.look(theme.pair.light).tokens,
  });
</script>

{#snippet card(
  on: boolean,
  label: string,
  onclick: () => void,
  title: string,
  tokens: ThemeTokens | null,
)}
  <button
    type="button"
    class={cn(
      "flex min-w-0 flex-col gap-1 rounded-md border p-1 text-left transition-colors duration-fast",
      on
        ? "border-accent bg-accent/8 shadow-[var(--glow-accent-soft)]"
        : "border-border hover:border-border-strong hover:bg-raised/40",
    )}
    aria-pressed={on}
    {title}
    {onclick}
  >
    {#if tokens}
      <ThemePreview {tokens} />
    {:else}
      <span class="relative block">
        <span class="grid grid-cols-2 gap-px overflow-hidden rounded-sm">
          <ThemePreview tokens={sys.dark} class="rounded-none" />
          <ThemePreview tokens={sys.light} class="rounded-none" />
        </span>
        <Monitor
          class="absolute top-1/2 left-1/2 size-icon -translate-1/2 rounded-xs bg-bg p-0.5 text-fg"
        />
      </span>
    {/if}
    <span class="flex items-center gap-1 px-0.5 text-3xs text-fg-muted">
      <span class="min-w-0 flex-1 truncate">{label}</span>
      {#if on}<Check class="size-3 shrink-0 text-accent" />{/if}
    </span>
  </button>
{/snippet}

<div class={cn("grid gap-1.5", cols[columns], klass)}>
  {@render card(theme.selected === null, systemLabel, () => theme.use(null), "", null)}
  {#each ordered as p (p.id)}
    {@render card(
      theme.selected === p.id,
      presetName(p.id, $t),
      () => theme.use(p.id),
      blurb(p),
      p.tokens,
    )}
  {/each}
</div>
