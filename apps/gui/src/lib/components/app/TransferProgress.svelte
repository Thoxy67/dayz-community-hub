<script lang="ts">
  import { Meter } from "$lib/components/ui/meter";
  import { bytes } from "$lib/format";
  import { clock } from "./log";

  /**
   * One transfer as it runs: what it is, how far (percent and bytes), how
   * fast, how long left and how long so far. Any figure not known yet is a
   * dash rather than a guess.
   */
  let {
    label,
    name = "",
    percent,
    done = null,
    total = null,
    speed = null,
    eta = null,
    elapsed = null,
    labels,
  }: {
    /** The block's stencilled caption. */
    label: string;
    /** What is being transferred. */
    name?: string;
    percent: number | null;
    done?: number | null;
    total?: number | null;
    /** Bytes per second. */
    speed?: number | null;
    /** Milliseconds left. */
    eta?: number | null;
    /** Milliseconds so far. */
    elapsed?: number | null;
    /** Captions for the figures, in the current language. */
    labels: { size: string; speed: string; eta: string; elapsed: string };
  } = $props();
</script>

<section class="rounded-md border border-border bg-panel p-3">
  <div class="flex items-baseline justify-between gap-2">
    <span class="label-stencil text-fg-faint">{label}</span>
    <span class="num font-mono text-xs text-fg">{percent != null ? `${percent.toFixed(1)}%` : "—"}</span>
  </div>
  {#if name}<p class="m-0 mt-1 truncate text-sm font-semibold text-fg" title={name}>{name}</p>{/if}
  <Meter class="mt-2" value={percent ?? 0} max={100} size="md" tone="in" {label} />
  <dl class="m-0 mt-2 grid grid-cols-2 gap-x-3 gap-y-1 font-mono text-2xs">
    <dt class="text-fg-faint">{labels.size}</dt>
    <dd class="m-0 text-right text-fg">{done != null && total ? `${bytes(done)} / ${bytes(total)}` : "—"}</dd>
    <dt class="text-fg-faint">{labels.speed}</dt>
    <dd class="m-0 text-right text-fg">{speed ? `${bytes(speed)}/s` : "—"}</dd>
    <dt class="text-fg-faint">{labels.eta}</dt>
    <dd class="m-0 text-right text-fg">{eta != null ? clock(eta) : "—"}</dd>
    <dt class="text-fg-faint">{labels.elapsed}</dt>
    <dd class="m-0 text-right text-fg">{elapsed != null ? clock(elapsed) : "—"}</dd>
  </dl>
</section>
