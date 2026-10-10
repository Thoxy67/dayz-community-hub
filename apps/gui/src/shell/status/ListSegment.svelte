<script lang="ts">
  import { dict } from "$lib/i18n";
  import ServerIcon from "~icons/lucide/server";
  import CloudOff from "~icons/lucide/cloud-off";
  import { app } from "$lib/stores/app.svelte";
  import { servers, STALE_MS } from "$lib/stores/servers.svelte";
  import { sources } from "$lib/stores/sources.svelte";
  import { num, relative } from "$lib/format";
  import { rolling } from "$lib/components/app/rolling.svelte";
  import { cn } from "$lib/cx";
  import Segment, { WORD, type Tone } from "./Segment.svelte";

  /**
   * The server list, always there: how many, how fresh, and what is being
   * done to it. A refresh says so with a sweep; Steam's listing, which goes
   * on after it, with its requests as a bar; a failed fetch in red with its
   * reason, and a click tries again.
   */
  let { now, armed }: { now: number; armed: boolean } = $props();
  const n = dict("nav");
  const s = dict("shell");

  const total = rolling(() => servers.total);
  const age = $derived(
    (void now, servers.lastRefreshed > 0 ? relative(Math.floor(servers.lastRefreshed / 1000)) : ""),
  );
  const stale = $derived(servers.lastRefreshed > 0 && now - servers.lastRefreshed > STALE_MS);
  const failed = $derived(
    !servers.refreshing && servers.lastRefresh?.phase === "failed"
      ? (servers.lastRefresh.error ?? "")
      : null,
  );
  const steam = $derived(sources.now?.steam.phase === "running" ? sources.now.steam : null);

  type View = {
    tone: Tone;
    tip: string;
    act?: () => void;
    progress?: number | null;
    note?: string;
  };
  const refresh = () => void servers.refresh();
  const v = $derived.by((): View => {
    if (servers.refreshing)
      return {
        tone: "accent",
        tip: $s.sbFetchingTip.value,
        progress: null,
        note: $s.sbFetching.value,
      };
    if (servers.listError && servers.total === 0)
      return {
        tone: "err",
        tip: $s.sbListFailedTip({ error: servers.listError }).value,
        act: refresh,
        note: $n.sbListDown.value,
      };
    if (failed !== null)
      return {
        tone: "err",
        tip: $s.sbListFailedTip({ error: failed }).value,
        act: refresh,
        note: $s.sbListFailed.value,
      };
    if (steam)
      return {
        tone: "accent",
        tip: $s.sbSteamListingTip({
          done: num(steam.requests_done),
          total: num(steam.requests_total),
          added: num(steam.added),
        }).value,
        act: () => app.go("servers"),
        progress:
          steam.requests_total > 0 ? (steam.requests_done / steam.requests_total) * 100 : null,
        note:
          steam.requests_total > 0
            ? `Steam ${Math.floor((steam.requests_done / steam.requests_total) * 100)}%`
            : "Steam",
      };
    return {
      tone: stale ? "warn" : "idle",
      tip: (stale ? $s.sbListStaleTip : $s.sbListTip)({ count: num(servers.total), age }).value,
      act: refresh,
      note: age,
    };
  });
</script>

<Segment
  icon={servers.listError && servers.total === 0 ? CloudOff : ServerIcon}
  tone={v.tone}
  tip={v.tip}
  label={v.act ? v.tip : undefined}
  onclick={v.act}
  progress={v.progress}
>
  {#if !(servers.listError && servers.total === 0)}
    <!-- Flashes once when the list's size changes. -->
    {#key servers.total}
      <span class={cn("num rounded-xs font-mono", armed && "animate-flash")}
        >{num(total.current)}</span
      >
    {/key}
    <span class={WORD}>{$s.sbServersWord.value}</span>
  {/if}
  {#if v.note}
    <span
      class={cn(
        "truncate",
        v.tone === "idle" ? "text-fg-faint" : "",
        // A sentence of its own when the list is down; a note otherwise.
        servers.listError && servers.total === 0 ? "" : WORD,
      )}>{servers.listError && servers.total === 0 ? "" : "· "}{v.note}</span
    >
  {/if}
</Segment>
