<script lang="ts">
  import { dict } from "$lib/i18n";
  import X from "~icons/lucide/x";
  import PlugZap from "~icons/lucide/plug-zap";
  import RefreshCw from "~icons/lucide/refresh-cw";
  import MapIcon from "~icons/lucide/map";
  import Users from "~icons/lucide/users";
  import Info from "~icons/lucide/info";
  import TriangleAlert from "~icons/lucide/triangle-alert";
  import { IconButton } from "$lib/components/ui/button";
  import { Copy } from "$lib/components/ui/copy";
  import { Disclosure } from "$lib/components/ui/disclosure";
  import { Spinner } from "$lib/components/ui/spinner";
  import {
    ExcludeButton,
    Facts,
    FavoriteButton,
    JoinButton,
    ModsCount,
    OsIcon,
    PingButton,
    PlayersButton,
    Section,
    ServerFlags,
    TimeOfDay,
    type Fact,
  } from "$lib/components/app";
  import { servers } from "$lib/stores/servers.svelte";
  import { serverData } from "$lib/stores/server-data.svelte";
  import { profile } from "$lib/stores/profile.svelte";
  import { connect } from "$lib/stores/connect.svelte";
  import { duration } from "$lib/format";
  import ModsSection from "./ModsSection.svelte";
  import BattleMetrics from "./BattleMetrics.svelte";

  /**
   * Everything known about one server, in one column: its line and head-count
   * live, how to join it, what it runs against what is installed, who is on
   * it, its rules, and BattleMetrics' long view. Shared by the server list,
   * the favourites and the history; works for a server that is not in the
   * public list too, from what the server answers directly.
   */
  let {
    ip,
    port,
    name = "",
    onclose,
    focusMods = false,
  }: {
    ip: string;
    port: number;
    name?: string;
    onclose?: () => void;
    focusMods?: boolean;
  } = $props();

  const c = dict("detail");

  const listed = $derived(servers.find(ip, port));
  const live = $derived(serverData.a2s(ip, port));
  const a2s = $derived(live.data);
  const queryPort = $derived(listed?.query_port ?? a2s?.query_port ?? port);
  const gamePort = $derived(listed?.game_port ?? a2s?.game_port ?? port);
  const title = $derived(listed?.name || a2s?.server_name || name || `${ip}:${port}`);
  const count = $derived(
    listed
      ? servers.count(listed)
      : a2s
        ? { players: a2s.players, max: a2s.max_players, bots: a2s.bots }
        : null,
  );
  const map = $derived(listed?.map || a2s?.map || "");
  const excluded = $derived(profile.excludedIps.has(ip));
  const address = $derived(`${ip}:${gamePort}`);
  let showMods = $state(false);

  // Ask the server itself whenever another one is shown (debounced, so
  // arrowing through a list does not query every row it passes).
  $effect(() => {
    const [i, p] = [ip, port];
    showMods = false;
    const t = setTimeout(() => void serverData.refreshA2s(i, p), 150);
    return () => clearTimeout(t);
  });

  const yesNo = (v: boolean | null | undefined) =>
    v == null ? "—" : v ? $c.yes.value : $c.no.value;
  const fill = $derived(count && count.max > 0 ? Math.round((count.players / count.max) * 100) : 0);
  const facts = $derived.by((): Fact[] => {
    const f: Fact[] = [
      { label: $c.gamePort.value, value: String(gamePort) },
      { label: $c.queryPort.value, value: String(queryPort) },
    ];
    if (listed) {
      f.push(
        {
          label: $c.platform.value,
          value: listed.environment === "w" ? $c.windows.value : $c.linux.value,
        },
        { label: $c.version.value, value: listed.version || "—", tone: "text-fg-muted" },
        {
          label: $c.battleye.value,
          value: yesNo(listed.battl_eye),
          tone: listed.battl_eye ? "text-ok" : "text-fg-muted",
        },
        {
          label: $c.vac.value,
          value: yesNo(listed.vac),
          tone: listed.vac ? "text-ok" : "text-fg-muted",
        },
        {
          label: $c.firstPerson.value,
          value: yesNo(listed.first_person_only),
          tone: listed.first_person_only ? "text-warn" : "text-fg-muted",
        },
        {
          label: $c.password.value,
          value: yesNo(listed.password),
          tone: listed.password ? "text-err" : "text-fg-muted",
        },
      );
    }
    if (count) {
      f.push(
        {
          label: $c.fill.value,
          value: `${fill}%`,
          tone: fill >= 100 ? "text-err" : fill > 50 ? "text-warn" : "text-ok",
        },
        { label: $c.realPlayers.value, value: String(Math.max(0, count.players - count.bots)) },
        {
          label: $c.bots.value,
          value: String(count.bots),
          tone: count.bots > 0 ? "text-warn" : "text-fg-muted",
        },
      );
    }
    if (a2s?.game) f.push({ label: $c.a2sGame.value, value: a2s.game, tone: "text-fg-muted" });
    return f;
  });

  const playersList = $derived(
    [...(a2s?.players_list ?? [])].sort((a, b) => (b.duration ?? 0) - (a.duration ?? 0)),
  );
</script>

<aside class="flex h-full min-h-0 flex-col bg-panel" aria-label={$c.title.value}>
  <!-- ── who ───────────────────────────────────────────────────────────── -->
  <header class="flex shrink-0 flex-col gap-1 border-b border-border px-pad pt-2.5 pb-2">
    <div class="flex items-start gap-2">
      <h2
        class="m-0 min-w-0 flex-1 text-sm leading-snug font-semibold break-words text-fg"
        data-selectable
      >
        {title}
      </h2>
      {#if onclose}<IconButton
          icon={X}
          label={$c.close.value}
          size="icon-xs"
          onclick={onclose}
        />{/if}
    </div>
    <div class="flex flex-wrap items-center gap-x-2 gap-y-1 text-2xs text-fg-faint">
      <Copy text={address} title={$c.copyIp.value} />
      {#if listed?.version || a2s?.version}<span class="font-mono"
          >v{listed?.version || a2s?.version}</span
        >{/if}
      {#if listed}
        <ServerFlags
          password={listed.password}
          firstPerson={listed.first_person_only}
          battleye={listed.battl_eye}
        />
        <OsIcon environment={listed.environment} class="size-3" />
      {/if}
      {#if excluded}<span class="text-err">{$c.excluded.value}</span>{/if}
    </div>
  </header>

  <div class="min-h-0 flex-1 overflow-y-auto">
    {#if !listed}
      <p
        class="m-0 flex items-start gap-2 border-b border-border bg-warn/8 px-pad py-2 text-2xs leading-snug text-warn"
      >
        <TriangleAlert class="mt-px size-3.5 shrink-0" />{$c.notListed.value}
      </p>
    {/if}

    <!-- ── the line and the head-count, live ─────────────────────────────── -->
    <div class="grid grid-cols-2 gap-px border-b border-border bg-border">
      <div class="flex flex-col gap-1 bg-panel px-pad py-2">
        <span class="label-stencil text-fg-faint">{$c.ping.value}</span>
        <PingButton {ip} {queryPort} size="md" />
      </div>
      <div class="flex flex-col gap-1 bg-panel px-pad py-2">
        <div class="flex items-center justify-between">
          <span class="label-stencil text-fg-faint">{$c.players.value}</span>
          <button
            class="grid size-5 place-items-center rounded-xs text-fg-faint hover:bg-raised hover:text-fg disabled:opacity-40"
            title={$c.refreshLive.value}
            aria-label={$c.refreshLive.value}
            disabled={live.loading}
            onclick={() => serverData.refreshA2s(ip, port)}
          >
            {#if live.loading}<Spinner class="size-3" />{:else}<RefreshCw class="size-3" />{/if}
          </button>
        </div>
        <PlayersButton {ip} {queryPort} />
      </div>
      <div class="flex items-center gap-1.5 bg-panel px-pad py-1.5 text-2xs text-map">
        <MapIcon class="size-3.5 shrink-0" /><span class="truncate font-medium">{map || "—"}</span>
      </div>
      <div class="flex items-center gap-2 bg-panel px-pad py-1.5">
        {#if listed?.time}<TimeOfDay time={listed.time} />{/if}
        {#if listed}
          <span class="ml-auto"
            ><ModsCount count={listed.mods_count} onclick={() => (showMods = true)} /></span
          >
        {/if}
      </div>
    </div>

    <!-- ── actions ───────────────────────────────────────────────────────── -->
    <div class="flex items-center gap-1.5 border-b border-border px-pad py-2">
      <div class="min-w-0 flex-1 [&>button]:w-full">
        <JoinButton {ip} port={listed ? port : gamePort} size="lg" />
      </div>
      <FavoriteButton name={title} {ip} port={queryPort} size="md" />
      <IconButton
        icon={PlugZap}
        label={$c.openDirect.value}
        variant="default"
        onclick={() => connect.openInDirect(ip, gamePort, queryPort)}
      />
      <span
        class="grid size-control place-items-center rounded-sm border border-border [&>button]:size-full"
      >
        <ExcludeButton {ip} always />
      </span>
    </div>

    <Section icon={Info} title={$c.general.value}>
      <Facts items={facts} />
      {#if live.error && !a2s}
        <div
          class="flex items-center gap-2 rounded-sm border border-err/30 bg-err/10 px-2 py-1.5 text-2xs text-err"
        >
          <span class="min-w-0 flex-1">{$c.liveFailed.value}</span>
          <button class="shrink-0 underline" onclick={() => serverData.refreshA2s(ip, port)}
            >{$c.retry.value}</button
          >
        </div>
      {/if}
    </Section>

    <ModsSection {listed} a2sMods={a2s?.mods_from_a2s ?? []} focus={focusMods || showMods} />

    <Section icon={Users} title={$c.playersOnline({ count: count?.players ?? 0 }).value}>
      {#if live.loading && !a2s}
        <div class="flex items-center gap-2 text-2xs text-fg-faint">
          <Spinner class="size-3.5" />{$c.queryingLive.value}
        </div>
      {:else if playersList.length > 0}
        <ol
          class="m-0 flex max-h-56 list-none flex-col overflow-y-auto rounded-sm border border-border bg-bg p-0"
        >
          {#each playersList as p, i (i)}
            <li
              class="flex h-6 items-center gap-2 border-b border-border/50 px-2 text-2xs last:border-b-0"
            >
              <span class="num w-5 shrink-0 text-right font-mono text-fg-faint">{i + 1}</span>
              <span class="min-w-0 flex-1 truncate text-fg" data-selectable>{p.name || "—"}</span>
              <span class="num shrink-0 font-mono text-fg-muted">{duration(p.duration ?? 0)}</span>
            </li>
          {/each}
        </ol>
      {:else if a2s && a2s.players === 0}
        <p class="m-0 text-2xs text-fg-faint italic">{$c.a2sNoPlayers.value}</p>
      {:else if a2s}
        <p class="m-0 text-2xs text-fg-faint italic">{$c.a2sNamesNotReported.value}</p>
      {:else}
        <p class="m-0 text-2xs text-fg-faint italic">{$c.a2sClickRefresh.value}</p>
      {/if}
    </Section>

    {#if a2s?.rules && a2s.rules.length > 0}
      <section class="border-t border-border py-1">
        <Disclosure label={$c.rules({ count: a2s.rules.length }).value}>
          <dl
            class="m-0 grid grid-cols-[minmax(0,auto)_1fr] gap-x-3 gap-y-0.5 px-pad pb-2 font-mono text-3xs"
          >
            {#each a2s.rules as r (r.name)}
              <dt class="truncate text-fg-faint" title={r.name}>{r.name}</dt>
              <dd class="m-0 break-all text-fg-muted" data-selectable>{r.value}</dd>
            {/each}
          </dl>
        </Disclosure>
      </section>
    {/if}

    <BattleMetrics {ip} port={gamePort} {queryPort} name={title} />
  </div>
</aside>
