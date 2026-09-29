<script lang="ts">
  import { dict } from "$lib/i18n";
  import Play from "~icons/lucide/play";
  import X from "~icons/lucide/x";
  import MapIcon from "~icons/lucide/map";
  import { Signal } from "$lib/components/ui/signal";
  import { Players } from "$lib/components/ui/players";
  import { Tooltip } from "$lib/components/ui/tooltip";
  import { profile } from "$lib/stores/profile.svelte";
  import { servers, keyOf } from "$lib/stores/servers.svelte";
  import { connect } from "$lib/stores/connect.svelte";
  import { prefs } from "$lib/stores/prefs.svelte";
  import { app } from "$lib/stores/app.svelte";
  import { relative, dateTime } from "$lib/format";

  /**
   * The server played last, at the foot of the rail: one click back in. Its
   * line and head-count are live, so the player sees whether it is worth it
   * before joining. Put away until another server becomes the last one.
   */
  let { collapsed = false }: { collapsed?: boolean } = $props();
  const n = dict("nav");
  const common = dict("common");

  const last = $derived(profile.data?.history?.[0] ?? null);
  const addr = $derived(last ? `${last.ip}:${last.port}` : "");
  const listed = $derived(last ? servers.find(last.ip, last.port) : undefined);
  const key = $derived(listed ? keyOf(listed) : addr);
  const count = $derived(listed ? servers.count(listed) : null);
  const hidden = $derived(!last || prefs.dismissedRejoin === addr);
</script>

{#if !hidden && last}
  {#if collapsed}
    <!-- Same frame as the rail's entries, so the icon lines up with theirs. -->
    <div class="px-1.5">
      <Tooltip text={`${$n.rejoin.value}: ${last.name}`} class="flex">
        <button
          class="grid h-control-lg w-full place-items-center rounded-sm bg-accent/15 text-accent hover:bg-accent/25"
          onclick={() => connect.rejoin()}
          aria-label={$n.rejoin.value}><Play class="size-icon" /></button
        >
      </Tooltip>
    </div>
  {:else}
    <!-- A short window gets one line: the full card would cover the menu. -->
    <div class="mx-2 hidden [@media(max-height:760px)]:block">
      <Tooltip text={`${$n.rejoin.value}: ${last.name}`} class="flex">
        <button
          class="flex h-control-lg w-full min-w-0 items-center gap-2 rounded-sm bg-accent/15 px-2 text-left text-accent hover:bg-accent/25"
          onclick={() => connect.rejoin()}
        >
          <Play class="size-icon shrink-0" />
          <span class="min-w-0 flex-1 truncate text-xs font-semibold">{last.name}</span>
        </button>
      </Tooltip>
    </div>
    <div
      class="relative mx-2 overflow-hidden rounded-md border border-border bg-panel [@media(max-height:760px)]:hidden"
    >
      <div class="flex items-center gap-1.5 border-b border-border/70 px-2 py-1">
        <span class="label-stencil whitespace-nowrap text-fg-faint">{$n.lastPlayed.value}</span>
        <span
          class="ml-auto truncate font-mono text-3xs whitespace-nowrap text-fg-faint"
          title={dateTime(last.ts)}>{relative(last.ts)}</span
        >
        <button
          class="grid size-4 place-items-center rounded-xs text-fg-faint hover:bg-raised hover:text-fg"
          aria-label={$common.close.value}
          title={$common.close.value}
          onclick={() => (prefs.dismissedRejoin = addr)}><X class="size-3" /></button
        >
      </div>
      <div class="space-y-1.5 px-2 py-1.5">
        <button
          class="block w-full truncate text-left text-xs font-semibold text-fg hover:text-accent"
          title={last.name}
          onclick={() => app.go("history", "last")}>{last.name}</button
        >
        {#if listed && count}
          <div class="flex items-center justify-between gap-2">
            <Players players={count.players} max={count.max} bots={count.bots} compact />
            <button onclick={() => servers.pingOne(listed.ip, listed.query_port)} class="shrink-0">
              <Signal ms={servers.ping.get(key)} pending={servers.pending.has(key)} size="xs" />
            </button>
          </div>
          <div class="flex items-center gap-1 text-2xs text-map">
            <MapIcon class="size-3 shrink-0" /><span class="truncate">{listed.map}</span>
            {#if listed.time}<span class="ml-auto font-mono text-fg-faint">{listed.time}</span>{/if}
          </div>
        {:else}
          <p class="m-0 text-2xs text-warn">{$n.notListed.value}</p>
        {/if}
        <button
          class="flex h-control w-full items-center justify-center gap-1.5 rounded-sm bg-accent font-display text-sm font-extrabold tracking-[0.08em] text-accent-fg uppercase hover:brightness-110"
          onclick={() => connect.rejoin()}
        >
          <Play class="size-icon-sm" />{$n.rejoin.value}
        </button>
      </div>
    </div>
  {/if}
{/if}
