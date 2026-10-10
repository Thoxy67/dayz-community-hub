<script lang="ts">
  import { dict } from "$lib/i18n";
  import HardDriveDownload from "~icons/lucide/hard-drive-download";
  import KeyRound from "~icons/lucide/key-round";
  import TriangleAlert from "~icons/lucide/triangle-alert";
  import Puzzle from "~icons/lucide/puzzle";
  import { app } from "$lib/stores/app.svelte";
  import { mods } from "$lib/stores/mods.svelte";
  import { servers } from "$lib/stores/servers.svelte";
  import { profile } from "$lib/stores/profile.svelte";
  import { cn } from "$lib/cx";
  import Segment, { WORD } from "./Segment.svelte";

  /**
   * The mods' side of the work. A running download (SteamCMD or Steam): what
   * it does, the mod in hand, n of total and the whole batch as a bar; a click
   * brings its dialog back. One waiting for a password or Steam Guard says so
   * in amber. With nothing running: SteamCMD missing (when it is what
   * downloads), then the mods behind, each leading where it is dealt with.
   */
  let { armed }: { armed: boolean } = $props();
  const s = dict("shell");
  const p = dict("progress");
  const n = dict("nav");

  const op = $derived(mods.busy ? mods.op : null);
  const waiting = $derived(op?.phase === "steam_guard_mobile" || op?.phase === "password_required");
  const kind = $derived(
    !op
      ? ""
      : op.kind === "login"
        ? $p.kindLogin.value
        : op.kind === "repair"
          ? $p.kindRepair.value
          : op.kind === "install_server" || op.kind === "install_manual"
            ? $p.kindInstall.value
            : $p.kindUpdate.value,
  );
  const done = $derived(op ? op.ok + op.failed : 0);
  const missing = $derived(
    !op && !profile.viaSteam && servers.stats != null && !servers.stats.has_steamcmd,
  );
  const behind = $derived(mods.stale.length);
  const reopen = () => (mods.op.minimised = false);
</script>

{#if op}
  {@const via = op.via === "steamworks" ? "Steam" : "SteamCMD"}
  <Segment
    icon={waiting ? KeyRound : HardDriveDownload}
    tone={waiting ? "warn" : "info"}
    pulse={!waiting && op.total === 0}
    tip={waiting
      ? $s.sbModWaitingTip.value
      : $s.sbModOpTip({
          kind,
          via,
          name: op.currentName,
          current: op.current,
          total: op.total,
        }).value}
    label={waiting ? $s.sbModWaitingTip.value : kind}
    onclick={reopen}
    progress={waiting ? undefined : op.total > 0 ? (done / op.total) * 100 : null}
    class="min-w-0 shrink"
  >
    {#if waiting}
      <span>{$s.sbModWaiting.value}</span>
    {:else}
      <span class={WORD}>{kind}</span>
      {#if op.total > 0}<span class="num font-mono">{op.current}/{op.total}</span>{/if}
      <span class="max-w-40 min-w-0 truncate text-fg-faint">{op.currentName}</span>
    {/if}
  </Segment>
{:else if missing}
  <Segment
    icon={TriangleAlert}
    tone="warn"
    tip={$s.sbSteamcmdMissingTip.value}
    label={$s.sbSteamcmdMissingTip.value}
    onclick={() => app.go("settings", "steam")}
  >
    <span class={WORD}>{$n.sbSteamcmdMissing.value}</span>
  </Segment>
{/if}

<!-- Mods behind: worth a look whatever is running. The third to go when
     the bar is narrow; the rail's Mods entry carries the same mark. -->
{#if mods.checking}
  <Segment
    icon={Puzzle}
    tone="info"
    tip={$s.sbModsCheckingTip.value}
    progress={null}
    class="@max-[800px]:hidden"
  >
    <span class={WORD}>{$n.sbModsChecking.value}</span>
  </Segment>
{:else if behind > 0}
  <Segment
    icon={Puzzle}
    tone="warn"
    tip={$s.sbModsBehindTip({ count: behind }).value}
    label={$s.sbModsBehindTip({ count: behind }).value}
    onclick={() => app.go("mods")}
    class="@max-[800px]:hidden"
  >
    {#key behind}
      <span class={cn("num rounded-xs font-mono", armed && "animate-flash")}>{behind}</span>
    {/key}
    <span class={WORD}>{$s.sbModsBehind.value}</span>
  </Segment>
{/if}
