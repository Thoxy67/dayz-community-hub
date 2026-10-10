<script lang="ts">
  import { getVersion } from "@tauri-apps/api/app";
  import { dict } from "$lib/i18n";
  import Download from "~icons/lucide/download";
  import RefreshCw from "~icons/lucide/refresh-cw";
  import MapIcon from "~icons/lucide/map";
  import Gamepad from "~icons/lucide/gamepad-2";
  import { inTauri } from "$lib/ipc/core";
  import { app } from "$lib/stores/app.svelte";
  import { updater } from "$lib/stores/updater.svelte";
  import { game } from "$lib/stores/game.svelte";
  import { sources } from "$lib/stores/sources.svelte";
  import { offline } from "$features/offline/offline.svelte";
  import { pad } from "$lib/gamepad/state.svelte";
  import { duration } from "$lib/format";
  import Segment, { WORD } from "./status/Segment.svelte";
  import ListSegment from "./status/ListSegment.svelte";
  import ScanSegment from "./status/ScanSegment.svelte";
  import ModSegments from "./status/ModSegments.svelte";
  import SteamSegment from "./status/SteamSegment.svelte";

  /**
   * The window's instrument panel, in two groups. On the left, what is
   * happening: the server list, the ping scan, the mods' downloads and
   * their state, the offline mode's install, the launcher's own download;
   * each segment shows only while it has something to say, except the list.
   * On the right, the state of things: the game, Steam, an update, the
   * version. Every segment has the same anatomy (`status/Segment.svelte`);
   * a narrow bar drops the words, then the least needed segments, by its
   * own width (container queries), not the window's.
   */
  const n = dict("nav");
  const s = dict("shell");
  const o = dict("offline");

  let version = $state("");
  if (inTauri)
    getVersion()
      .then((v) => (version = v))
      .catch(() => {});

  // Ages and the game's time are re-read every 30 s; nothing else moves them.
  let now = $state(Date.now());
  $effect(() => {
    const t = setInterval(() => (now = Date.now()), 30_000);
    return () => clearInterval(t);
  });
  /** Not while the window opens: a flash says "this just changed". */
  let armed = $state(false);
  $effect(() => {
    const t = setTimeout(() => (armed = true), 1500);
    return () => clearTimeout(t);
  });
  // Steam's listing reports through the list's sources.
  $effect(() => void sources.start());

  const played = $derived(game.since > 0 ? duration((now - game.since) / 1000) : "");
</script>

<footer
  data-pad-region
  aria-label={$s.sbBar.value}
  class="@container flex h-7 shrink-0 items-center gap-0.5 border-t border-border bg-bg px-1 text-2xs text-fg-faint"
>
  <!-- What is happening. -->
  <div class="flex min-w-0 flex-1 items-center gap-0.5 overflow-hidden">
    <ListSegment {now} {armed} />
    <ScanSegment />
    <ModSegments {armed} />
    {#if offline.installing}
      <Segment
        icon={MapIcon}
        tone="info"
        tip={$s.sbOfflineTip.value}
        label={$s.sbOfflineTip.value}
        onclick={() => app.go("offline")}
        progress={null}
      >
        <span class={WORD}>{$s.sbOffline.value}</span>
        <span>{$o.installing.value}</span>
      </Segment>
    {/if}
    {#if updater.state === "downloading"}
      {@const v = updater.info?.version ?? ""}
      <Segment
        icon={Download}
        tone="info"
        tip={$s.sbUpdatingTip({ version: v, percent: updater.percent }).value}
        label={$s.sbUpdatingTip({ version: v, percent: updater.percent }).value}
        onclick={() => app.go("about", "update")}
        progress={updater.total > 0 ? updater.percent : null}
      >
        <span class={WORD}>{$s.sbUpdating.value}</span>
        {#if updater.total > 0}<span class="num font-mono">{updater.percent}%</span>{/if}
      </Segment>
    {/if}
  </div>

  <span class="mx-1 h-3.5 w-px shrink-0 bg-border" aria-hidden="true"></span>

  <!-- The state of things. -->
  <div class="flex shrink-0 items-center gap-0.5">
    {#if game.running}
      <!-- An indicator only: closing the game is the rail's (at its foot),
           and with a controller Steam's overlay's. -->
      <Segment
        icon={Gamepad}
        tone="ok"
        tip={`${played ? $s.sbGameFor({ time: played }).value : $s.sbGameOn.value} ${
          pad.mode === "gamepad" ? $s.sbGameQuitPad.value : $s.sbGameQuit.value
        }`}
      >
        <span class="size-1.5 animate-pulse rounded-full bg-ok" aria-hidden="true"></span>
        <span class={WORD}>DayZ</span>
        {#if played}<span class="num font-mono font-normal text-fg-muted">{played}</span>{/if}
      </Segment>
    {/if}
    <SteamSegment {now} />
    {#if updater.state === "available" && updater.info}
      <Segment
        icon={Download}
        tone="ok"
        tip={$s.sbUpdateTip({ version: updater.info.version }).value}
        label={$s.sbUpdateTip({ version: updater.info.version }).value}
        onclick={() => app.go("about", "update")}
      >
        <span class="num font-mono">{$n.sbUpdate({ version: updater.info.version }).value}</span>
      </Segment>
    {:else if updater.state === "done"}
      <Segment
        icon={RefreshCw}
        tone="ok"
        tip={$s.sbUpdateReadyTip.value}
        label={$s.sbUpdateReadyTip.value}
        onclick={() => app.go("about", "update")}
      >
        <span class={WORD}>{$s.sbUpdateReady.value}</span>
      </Segment>
    {/if}
    {#if version}
      <span class="px-2 font-mono text-fg-faint @max-[1000px]:hidden"
        >{$n.version({ version }).value}</span
      >
    {/if}
  </div>
</footer>
