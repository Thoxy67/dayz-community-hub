<script lang="ts" module>
  /** How full a server is, as a tone: empty is quiet, full is a warning. */
  export function fillTone(players: number, max: number): "muted" | "ok" | "warn" | "err" {
    if (players <= 0 || max <= 0) return "muted";
    if (players >= max) return "err";
    if (players / max > 0.5) return "warn";
    return "ok";
  }
</script>

<script lang="ts">
  import Bot from "~icons/lucide/bot";
  import { cn } from "$lib/cx";

  /**
   * Players on a server: the count, the capacity, and a bar underneath. Bots
   * are told apart, because many servers pad their count with them to look
   * busy; `unverified` marks a "full" the query could not confirm.
   */
  let {
    players,
    max,
    bots = 0,
    queue = 0,
    loading = false,
    compact = false,
    botsLabel = "",
    class: klass = "",
  }: {
    players: number;
    max: number;
    bots?: number;
    queue?: number;
    loading?: boolean;
    compact?: boolean;
    botsLabel?: string;
    class?: string;
  } = $props();

  const tone = $derived(fillTone(players, max));
  const ink = { muted: "text-fg-faint", ok: "text-ok", warn: "text-warn", err: "text-err" } as const;
  const fill = { muted: "bg-fg-faint/40", ok: "bg-ok", warn: "bg-warn", err: "bg-err" } as const;
  const pct = $derived(max > 0 ? Math.min(100, (players / max) * 100) : 0);
  const botPct = $derived(max > 0 ? Math.min(pct, (bots / max) * 100) : 0);
</script>

<span class={cn("inline-flex min-w-0 flex-col gap-0.5", compact ? "w-16" : "w-24", klass)}>
  <span class="flex items-baseline gap-1 font-mono text-2xs">
    <span class={cn("num font-semibold", ink[tone], loading && "opacity-50")}>{players}</span>
    <span class="num text-fg-faint">/{max}</span>
    {#if queue > 0}<span class="num text-warn">+{queue}</span>{/if}
    {#if bots > 0}
      <span class="ml-auto inline-flex items-center gap-0.5 text-fg-faint" title={botsLabel}>
        <Bot class="size-3" /><span class="num">{bots}</span>
      </span>
    {/if}
  </span>
  <span class="relative h-1 overflow-hidden rounded-full bg-raised">
    <span
      class={cn("absolute inset-y-0 left-0 rounded-full transition-[width] duration-500", fill[tone])}
      style="width: {pct}%"
    ></span>
    {#if botPct > 0}
      <span
        class="absolute inset-y-0 rounded-r-full bg-[repeating-linear-gradient(135deg,var(--color-fg-faint)_0_2px,transparent_2px_4px)]"
        style="left: {pct - botPct}%; width: {botPct}%"
      ></span>
    {/if}
  </span>
</span>
