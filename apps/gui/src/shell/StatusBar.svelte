<script lang="ts">
  import { getVersion } from "@tauri-apps/api/app";
  import { dict } from "$lib/i18n";
  import Pause from "~icons/lucide/pause";
  import PlayIcon from "~icons/lucide/play";
  import RotateCw from "~icons/lucide/rotate-cw";
  import ServerIcon from "~icons/lucide/server";
  import CloudOff from "~icons/lucide/cloud-off";
  import Radar from "~icons/lucide/radar";
  import Terminal from "~icons/lucide/square-terminal";
  import HardDriveDownload from "~icons/lucide/hard-drive-download";
  import Puzzle from "~icons/lucide/puzzle";
  import Download from "~icons/lucide/download";
  import { Tooltip } from "$lib/components/ui/tooltip";
  import { inTauri } from "$lib/ipc/core";
  import { app } from "$lib/stores/app.svelte";
  import { servers, STALE_MS } from "$lib/stores/servers.svelte";
  import { mods } from "$lib/stores/mods.svelte";
  import { updater } from "$lib/stores/updater.svelte";
  import { num, relative } from "$lib/format";
  import { cn } from "$lib/cx";

  /**
   * What the app knows and is doing in the background, always in the same
   * place, one segment per concern: the server list, the ping scan, SteamCMD,
   * the mods, the launcher's own update. Each says its state in a word and a
   * colour, and a click goes where that state is dealt with.
   */
  const n = dict("nav");
  let version = $state("");
  if (inTauri) getVersion().then((v) => (version = v)).catch(() => {});

  // The list's age is re-read every 30 s; nothing else changes it.
  let now = $state(Date.now());
  $effect(() => {
    const t = setInterval(() => (now = Date.now()), 30_000);
    return () => clearInterval(t);
  });
  const stale = $derived(servers.lastRefreshed > 0 && now - servers.lastRefreshed > STALE_MS);
  const age = $derived(
    (void now, servers.lastRefreshed > 0 ? relative(Math.floor(servers.lastRefreshed / 1000)) : ""),
  );
  const scan = $derived(servers.scan);
  const pct = $derived(scan && scan.total > 0 ? Math.floor((scan.done / scan.total) * 100) : 0);
  const op = $derived(mods.op.active && mods.op.phase !== "finished" ? mods.op : null);
  const opPct = $derived(op && op.total > 0 ? (op.current / op.total) * 100 : 0);
  const hasSteamcmd = $derived(servers.stats?.has_steamcmd ?? true);

  const seg = "flex h-full items-center gap-1.5 px-2.5 transition-colors";
  const btn = `${seg} hover:bg-raised hover:text-fg`;
</script>

{#snippet bar(value: number, tone: string)}
  <span class="relative h-1 w-20 overflow-hidden rounded-full bg-raised" aria-hidden="true">
    <span class={cn("absolute inset-y-0 left-0 rounded-full transition-[width]", tone)} style="width: {value}%"></span>
  </span>
{/snippet}

<footer
  class="flex h-7 shrink-0 items-stretch divide-x divide-border border-t border-border bg-bg text-2xs text-fg-faint"
>
  <!-- The server list: how many, how fresh; a click fetches it again. -->
  <Tooltip text={servers.listError ?? $n.sbRefreshList.value} side="top" class="flex">
    <button
      class={cn(btn, servers.listError ? "text-err" : stale && "text-warn")}
      disabled={servers.refreshing}
      onclick={() => servers.refresh()}
    >
      {#if servers.refreshing}
        <RotateCw class="size-3 animate-spin text-accent" />
        <span>{$n.sbRefreshing.value}</span>
      {:else if servers.listError && servers.total === 0}
        <CloudOff class="size-3" />
        <span>{$n.sbListDown.value}</span>
      {:else}
        <ServerIcon class="size-3" />
        <span class="num font-mono text-fg-muted">{$n.sbServers({ count: num(servers.total) }).value}</span>
        {#if age}<span class={cn(!stale && "text-fg-faint")}>· {age}</span>{/if}
      {/if}
    </button>
  </Tooltip>

  <!-- The ping scan: its progress while it runs, a way to run it again after. -->
  {#if scan}
    <span class={cn(seg, "gap-2")}>
      <Radar class={cn("size-3", servers.scanPaused ? "text-warn" : "animate-pulse text-accent")} />
      <span class="num font-mono text-fg-muted">
        {servers.scanPaused ? $n.scanPaused.value : $n.scanning({ done: num(scan.done), total: num(scan.total) }).value}
      </span>
      {@render bar(pct, servers.scanPaused ? "bg-warn" : "bg-accent")}
      <span class="num w-8 font-mono">{pct}%</span>
      <Tooltip text={servers.scanPaused ? $n.resumeScan.value : $n.pauseScan.value} side="top">
        <button
          class="grid size-5 place-items-center rounded-xs hover:bg-raised hover:text-fg"
          aria-label={servers.scanPaused ? $n.resumeScan.value : $n.pauseScan.value}
          onclick={() => servers.toggleScanPause()}
        >
          {#if servers.scanPaused}<PlayIcon class="size-3" />{:else}<Pause class="size-3" />{/if}
        </button>
      </Tooltip>
    </span>
  {:else if servers.total > 0}
    <Tooltip text={$n.sbRescan.value} side="top" class="flex">
      <button class={btn} onclick={() => servers.startScan()} aria-label={$n.sbRescan.value}>
        <Radar class="size-3" /><span>{$n.sbPing.value}</span><RotateCw class="size-3 opacity-60" />
      </button>
    </Tooltip>
  {/if}

  <!-- SteamCMD: a job running in the background comes back from here. -->
  {#if op}
    <Tooltip text={$n.showProgress.value} side="top" class="flex min-w-0">
      <button class={cn(btn, "min-w-0 text-info")} onclick={() => (mods.op.minimised = false)}>
        <HardDriveDownload class="size-3 shrink-0 animate-pulse" />
        <span class="num shrink-0 font-mono">{$n.modOpRunning({ current: op.current, total: op.total }).value}</span>
        {@render bar(opPct, "bg-info")}
        <span class="max-w-48 min-w-0 truncate text-fg-muted">{op.currentName}</span>
      </button>
    </Tooltip>
  {:else}
    <button
      class={cn(btn, !hasSteamcmd && "text-warn")}
      onclick={() => app.go("settings", "steam")}
    >
      <Terminal class="size-3" />
      <span>{hasSteamcmd ? $n.sbSteamcmdReady.value : $n.sbSteamcmdMissing.value}</span>
    </button>
  {/if}

  <!-- Mods: out of date ones lead to the mods view. -->
  {#if mods.installed.length > 0 || mods.checking}
    <button class={cn(btn, mods.stale.length > 0 && "text-warn")} onclick={() => app.go("mods")}>
      <Puzzle class={cn("size-3", mods.checking && "animate-pulse")} />
      <span class="num">
        {#if mods.checking}
          {$n.sbModsChecking.value}
        {:else if mods.stale.length > 0}
          {$n.sbModsStale({ count: mods.stale.length }).value}
        {:else}
          {$n.sbModsOk({ count: mods.installed.length }).value}
        {/if}
      </span>
    </button>
  {/if}

  <span class="flex-1"></span>

  {#if updater.state === "available" && updater.info}
    <button class={cn(btn, "font-semibold text-ok")} onclick={() => app.go("about", "update")}>
      <Download class="size-3" />{$n.sbUpdate({ version: updater.info.version }).value}
    </button>
  {/if}
  <span class={cn(seg, "font-mono")}>{version ? $n.version({ version }).value : ""}</span>
</footer>
