<script lang="ts" module>
  /** Past this, a reply is a timeout (the backend reports 9999 for one). */
  export const PING_TIMEOUT_MS = 5000;

  export type SignalLevel = 0 | 1 | 2 | 3 | 4;

  /** Bars lit for a round trip: a radio's reading of the line to the server. */
  export function signalLevel(ms: number | null | undefined): SignalLevel {
    if (ms == null || ms >= PING_TIMEOUT_MS) return 0;
    if (ms < 50) return 4;
    if (ms < 100) return 3;
    if (ms < 180) return 2;
    return 1;
  }

  export function signalTone(level: SignalLevel): string {
    return level >= 4
      ? "text-ok"
      : level >= 2
        ? "text-warn"
        : level === 1
          ? "text-err"
          : "text-fg-faint";
  }
</script>

<script lang="ts">
  import { cn } from "$lib/cx";

  /**
   * The ping, read the way a field radio reads a signal: four bars, lit by
   * how quickly the server answered, and the figure beside them. It is the
   * one mark every server in the app wears, so a list can be scanned for a
   * good line without reading a number.
   *
   * `pending` sweeps the bars while a query is out; a timeout shows four
   * dark bars struck through.
   */
  let {
    ms,
    pending = false,
    label = true,
    size = "sm",
    class: klass = "",
  }: {
    ms: number | null | undefined;
    pending?: boolean;
    /** The figure beside the bars. */
    label?: boolean;
    size?: "xs" | "sm" | "md";
    class?: string;
  } = $props();

  const level = $derived(signalLevel(ms));
  const timedOut = $derived(!pending && ms != null && ms >= PING_TIMEOUT_MS);
  const tone = $derived(pending ? "text-fg-faint" : signalTone(level));
  const dims = { xs: "h-2.5 gap-px", sm: "h-3 gap-[2px]", md: "h-4 gap-[2px]" } as const;
  const bar = { xs: "w-[2px]", sm: "w-[3px]", md: "w-1" } as const;
</script>

<span class={cn("inline-flex items-center gap-1.5", tone, klass)}>
  <span class={cn("relative inline-flex items-end", dims[size])} aria-hidden="true">
    {#each [1, 2, 3, 4] as b (b)}
      <span
        class={cn(
          "rounded-[1px]",
          bar[size],
          pending
            ? "animate-pulse bg-current opacity-40"
            : b <= level
              ? "bg-current"
              : "bg-current opacity-20",
        )}
        style="height: {b * 25}%; {pending ? `animation-delay: ${b * 120}ms` : ''}"
      ></span>
    {/each}
    {#if timedOut}
      <span class="absolute inset-x-[-1px] top-1/2 h-px -rotate-[25deg] bg-err"></span>
    {/if}
  </span>
  {#if label}
    <span class="num font-mono text-2xs">
      {#if pending}…{:else if ms == null}—{:else if timedOut}<span class="text-err">×</span
        >{:else}{ms}<span class="text-fg-faint">ms</span>{/if}
    </span>
  {/if}
</span>
