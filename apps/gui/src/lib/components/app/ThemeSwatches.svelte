<script lang="ts">
  import { useIntlayer } from "svelte-intlayer";
  import Monitor from "~icons/lucide/monitor";
  import Check from "~icons/lucide/check";
  import { theme, PRESETS } from "$lib/theme/theme.svelte";
  import type { Preset } from "$lib/theme/presets";
  import { cn } from "$lib/cx";

  /**
   * Every preset as a small card painted in its own colours (ground, panel,
   * text, accent, status), dark ones first, plus "follow the system".
   * Choosing one wears it at once.
   */
  let {
    systemLabel,
    columns = 4,
    class: klass = "",
  }: { systemLabel: string; columns?: 3 | 4 | 5 | 6; class?: string } = $props();

  const t = useIntlayer("theme");
  const ordered = [...PRESETS].sort((a, b) => (a.scheme === b.scheme ? 0 : a.scheme === "dark" ? -1 : 1));
  const cols = { 3: "grid-cols-3", 4: "grid-cols-4", 5: "grid-cols-5", 6: "grid-cols-6" } as const;

  function label(id: string): string {
    const key = `preset${id.replace(/(^|_)(\w)/g, (_m, _s, c: string) => c.toUpperCase())}`;
    const node = ($t as unknown as Record<string, { value?: string } | undefined>)[key];
    return node?.value ?? id.replace(/_/g, " ");
  }
</script>

{#snippet card(p: Preset)}
  {@const on = theme.selected === p.id}
  <button
    class={cn(
      "group relative overflow-hidden rounded-md border text-left transition-colors",
      on ? "border-accent shadow-[var(--glow-accent-soft)]" : "border-border hover:border-border-strong",
    )}
    style="background: {p.tokens.bg}"
    aria-pressed={on}
    onclick={() => theme.use(p.id)}
  >
    <span class="m-1.5 mb-0 flex h-7 flex-col justify-center gap-1 rounded-sm px-1.5" style="background: {p.tokens.panel}">
      <span class="h-1 w-3/4 rounded-full" style="background: {p.tokens.fg}; opacity: .8"></span>
      <span class="flex gap-1">
        <span class="h-1 w-4 rounded-full" style="background: {p.tokens.accent}"></span>
        <span class="h-1 w-2 rounded-full" style="background: {p.tokens.ok}"></span>
        <span class="h-1 w-2 rounded-full" style="background: {p.tokens.err}"></span>
      </span>
    </span>
    <span class="flex items-center gap-1 px-1.5 py-1 text-3xs capitalize" style="color: {p.tokens.fg}">
      <span class="truncate">{label(p.id)}</span>
      {#if on}<Check class="ml-auto size-3 shrink-0" style="color: {p.tokens.accent}" />{/if}
    </span>
  </button>
{/snippet}

<div class={cn("grid gap-1.5", cols[columns], klass)}>
  <button
    class={cn(
      "flex flex-col items-center justify-center gap-1 rounded-md border bg-raised/40 px-2 text-3xs text-fg-muted transition-colors",
      theme.selected === null ? "border-accent text-fg shadow-[var(--glow-accent-soft)]" : "border-border hover:border-border-strong",
    )}
    aria-pressed={theme.selected === null}
    onclick={() => theme.use(null)}
  >
    <Monitor class="size-icon" />
    <span class="text-center leading-tight">{systemLabel}</span>
  </button>
  {#each ordered as p (p.id)}{@render card(p)}{/each}
</div>
