<script lang="ts">
  import { getVersion } from "@tauri-apps/api/app";
  import { useIntlayer } from "svelte-intlayer";
  import Pause from "~icons/lucide/pause";
  import PlayIcon from "~icons/lucide/play";
  import RefreshCw from "~icons/lucide/refresh-cw";
  import Clock from "~icons/lucide/clock";
  import Radar from "~icons/lucide/radar";
  import HardDriveDownload from "~icons/lucide/hard-drive-download";
  import { Tooltip } from "$lib/components/ui/tooltip";
  import { inTauri } from "$lib/ipc/core";
  import { servers, STALE_MS } from "$lib/stores/servers.svelte";
  import { mods } from "$lib/stores/mods.svelte";
  import { num, relative } from "$lib/format";
  import { cn } from "$lib/cx";

  /**
   * What the app is doing in the background, always in the same place: the
   * ping scan and its progress, how old the server list is, a SteamCMD job
   * running behind a closed window, and the build.
   */
  const n = useIntlayer("nav");
  let version = $state("");
  if (inTauri) getVersion().then((v) => (version = v)).catch(() => {});

  // The age is re-read every 30 s; nothing else changes it.
  let now = $state(Date.now());
  $effect(() => {
    const t = setInterval(() => (now = Date.now()), 30_000);
    return () => clearInterval(t);
  });
  const stale = $derived(servers.lastRefreshed > 0 && now - servers.lastRefreshed > STALE_MS);
  const scan = $derived(servers.scan);
  const pct = $derived(scan && scan.total > 0 ? (scan.done / scan.total) * 100 : 0);
</script>

<footer class="flex h-6 shrink-0 items-center gap-3 border-t border-border bg-bg px-2.5 text-2xs text-fg-faint">
  {#if scan}
    <span class="flex items-center gap-1.5">
      <Radar class={cn("size-3", servers.scanPaused ? "text-warn" : "animate-pulse text-accent")} />
      <span class="num font-mono">
        {servers.scanPaused ? $n.scanPaused.value : $n.scanning({ done: num(scan.done), total: num(scan.total) }).value}
      </span>
      <span class="relative h-1 w-28 overflow-hidden rounded-full bg-raised">
        <span class="absolute inset-y-0 left-0 rounded-full bg-accent transition-[width]" style="width: {pct}%"></span>
      </span>
      <Tooltip text={servers.scanPaused ? $n.resumeScan.value : $n.pauseScan.value} side="top">
        <button
          class="grid size-4 place-items-center rounded-xs hover:bg-raised hover:text-fg"
          aria-label={servers.scanPaused ? $n.resumeScan.value : $n.pauseScan.value}
          onclick={() => servers.toggleScanPause()}
        >
          {#if servers.scanPaused}<PlayIcon class="size-3" />{:else}<Pause class="size-3" />{/if}
        </button>
      </Tooltip>
    </span>
  {/if}

  {#if servers.lastRefreshed > 0}
    <span class={cn("flex items-center gap-1", stale && "text-warn")} title={stale ? $n.listStale.value : ""}>
      <Clock class="size-3" />
      {(void now, $n.listAge({ age: relative(Math.floor(servers.lastRefreshed / 1000)) }).value)}
      {#if stale}
        <button class="ml-0.5 rounded-xs px-1 hover:bg-warn/15" onclick={() => servers.refresh()}>
          <RefreshCw class="size-3" />
        </button>
      {/if}
    </span>
  {/if}

  {#if mods.op.active && mods.op.phase !== "finished"}
    <span class="flex items-center gap-1.5 text-info">
      <HardDriveDownload class="size-3 animate-pulse" />
      <span class="num font-mono">{$n.modOpRunning({ current: mods.op.current, total: mods.op.total }).value}</span>
      <span class="max-w-48 truncate text-fg-muted">{mods.op.currentName}</span>
    </span>
  {/if}

  <span class="ml-auto font-mono">{version ? $n.version({ version }).value : ""}</span>
</footer>
