<script lang="ts">
  import { useIntlayer } from "svelte-intlayer";

  /**
   * A day of player counts, drawn as an area under a line, with the minimum,
   * average and peak marked and the value under the pointer read out. Points
   * are [unix seconds, players].
   */
  let { points, max: capacity = null }: { points: [number, number][]; max?: number | null } =
    $props();
  const c = useIntlayer("detail");

  const W = 300;
  const H = 64;
  const sorted = $derived([...points].sort((a, b) => a[0] - b[0]));
  const stats = $derived.by(() => {
    let min = Infinity;
    let peak = -Infinity;
    let sum = 0;
    for (const [, v] of sorted) {
      if (v < min) min = v;
      if (v > peak) peak = v;
      sum += v;
    }
    return {
      min,
      peak,
      avg: Math.round(sum / Math.max(1, sorted.length)),
      now: sorted.at(-1)?.[1] ?? 0,
    };
  });
  const top = $derived(Math.max(1, capacity ?? 0, stats.peak));
  const t0 = $derived(sorted[0]?.[0] ?? 0);
  const t1 = $derived(sorted.at(-1)?.[0] ?? 1);
  const x = (t: number) => ((t - t0) / Math.max(1, t1 - t0)) * W;
  const y = (v: number) => H - (v / top) * (H - 4) - 2;
  const line = $derived(
    sorted.map(([t, v], i) => `${i ? "L" : "M"}${x(t).toFixed(1)},${y(v).toFixed(1)}`).join(""),
  );
  const area = $derived(sorted.length ? `${line}L${W},${H}L0,${H}Z` : "");

  let hover = $state<number | null>(null);
  function onmove(e: PointerEvent) {
    const r = (e.currentTarget as SVGElement).getBoundingClientRect();
    const t = t0 + ((e.clientX - r.left) / r.width) * (t1 - t0);
    let best = 0;
    for (let i = 1; i < sorted.length; i++)
      if (Math.abs(sorted[i]![0] - t) < Math.abs(sorted[best]![0] - t)) best = i;
    hover = best;
  }
  const hovered = $derived(hover !== null ? sorted[hover] : undefined);
  const hhmm = (ts: number) =>
    new Date(ts * 1000).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
</script>

<div class="flex flex-col gap-1">
  <div class="flex items-baseline justify-between font-mono text-3xs text-fg-faint">
    <span>{$c.bmPlayerHistory.value}</span>
    {#if hovered}
      <span class="text-fg"><span class="num">{hovered[1]}</span> · {hhmm(hovered[0])}</span>
    {:else}
      <span>
        {$c.bmMin.value} <span class="num text-fg-muted">{stats.min}</span> ·
        {$c.bmAvg({ count: stats.avg }).value} ·
        {$c.bmPeak.value} <span class="num text-fg-muted">{stats.peak}</span>
      </span>
    {/if}
  </div>
  <svg
    viewBox="0 0 {W} {H}"
    preserveAspectRatio="none"
    class="h-16 w-full touch-none rounded-xs bg-plot text-accent"
    onpointermove={onmove}
    onpointerleave={() => (hover = null)}
    role="img"
    aria-label={$c.bmPlayerHistory.value}
  >
    <line
      x1="0"
      x2={W}
      y1={y(stats.avg)}
      y2={y(stats.avg)}
      class="stroke-border-strong"
      stroke-dasharray="3 3"
      vector-effect="non-scaling-stroke"
    />
    {#if capacity}
      <line
        x1="0"
        x2={W}
        y1={y(capacity)}
        y2={y(capacity)}
        class="stroke-err/40"
        vector-effect="non-scaling-stroke"
      />
    {/if}
    <path d={area} fill="currentColor" fill-opacity="0.14" />
    <path
      d={line}
      fill="none"
      stroke="currentColor"
      stroke-width="1.5"
      vector-effect="non-scaling-stroke"
      stroke-linejoin="round"
    />
    {#if hovered}
      <line
        x1={x(hovered[0])}
        x2={x(hovered[0])}
        y1="0"
        y2={H}
        class="stroke-fg-faint"
        vector-effect="non-scaling-stroke"
      />
      <circle cx={x(hovered[0])} cy={y(hovered[1])} r="2.5" fill="currentColor" />
    {/if}
  </svg>
  <div class="flex justify-between font-mono text-3xs text-fg-faint">
    <span>{$c.bm24hAgo.value}</span>
    <span>{$c.bmNow.value} · <span class="num text-fg-muted">{stats.now}</span></span>
  </div>
</div>
