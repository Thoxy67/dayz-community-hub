<script lang="ts">
  import { dict } from "$lib/i18n";
  import Pause from "~icons/lucide/pause";
  import PlayIcon from "~icons/lucide/play";
  import RotateCcw from "~icons/lucide/rotate-ccw";
  import Radar from "~icons/lucide/radar";
  import { Tooltip } from "$lib/components/ui/tooltip";
  import { servers } from "$lib/stores/servers.svelte";
  import { num } from "$lib/format";
  import Segment, { WORD } from "./Segment.svelte";

  /**
   * The ping scan of the whole list. While it runs: how far, a bar, and
   * small buttons to pause (or resume) and to start over. Idle, a quiet
   * "Ping" that pings every server again. Hidden with no list to ping.
   */
  const n = dict("nav");
  const s = dict("shell");
  const scan = $derived(servers.scan);
  const paused = $derived(servers.scanPaused);
  const pct = $derived(scan && scan.total > 0 ? (scan.done / scan.total) * 100 : null);
  const mini =
    "grid size-5 place-items-center rounded-xs text-fg-faint transition-colors duration-fast hover:bg-raised hover:text-fg";
</script>

{#snippet act(text: string, Icon: typeof Pause, run: () => void)}
  <Tooltip {text} side="top">
    <button type="button" class={mini} aria-label={text} onclick={run}>
      <Icon class="size-3" />
    </button>
  </Tooltip>
{/snippet}

{#if scan}
  <Segment
    icon={Radar}
    tone={paused ? "warn" : "accent"}
    pulse={!paused}
    tip={(paused ? $s.sbPingPausedTip : $s.sbPingTip)({
      done: num(scan.done),
      total: num(scan.total),
    }).value}
    progress={pct}
  >
    <span class={WORD}>{paused ? $n.scanPaused.value : $n.sbPing.value}</span>
    <span class="num font-mono">{num(scan.done)}/{num(scan.total)}</span>
    {#snippet actions()}
      <span class="flex items-center pr-1">
        {#if paused}
          {@render act($n.resumeScan.value, PlayIcon, () => void servers.toggleScanPause())}
        {:else}
          {@render act($n.pauseScan.value, Pause, () => void servers.toggleScanPause())}
        {/if}
        {@render act($n.sbRescan.value, RotateCcw, () => void servers.startScan())}
      </span>
    {/snippet}
  </Segment>
{:else if servers.total > 0 && !servers.refreshing}
  <Segment
    icon={Radar}
    tip={$n.sbRescan.value}
    label={$n.sbRescan.value}
    onclick={() => void servers.startScan()}
  >
    <span class={WORD}>{$n.sbPing.value}</span>
  </Segment>
{/if}
