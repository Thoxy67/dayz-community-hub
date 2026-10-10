<script lang="ts">
  import { dict } from "$lib/i18n";
  import Users from "~icons/lucide/users";
  import UserX from "~icons/lucide/user-x";
  import SearchX from "~icons/lucide/search-x";
  import RefreshCw from "~icons/lucide/refresh-cw";
  import Server from "~icons/lucide/server";
  import List from "~icons/lucide/list";
  import ScanSearch from "~icons/lucide/scan-search";
  import PlugZap from "~icons/lucide/plug-zap";
  import UserRound from "~icons/lucide/user-round";
  import MessageSquare from "~icons/lucide/message-square";
  import {
    Empty,
    Figure,
    LIST_ROW_PX,
    MasterDetail,
    PageHeader,
    ServerListRow,
  } from "$lib/components/app";
  import { Button, IconButton } from "$lib/components/ui/button";
  import { Input } from "$lib/components/ui/input";
  import { SectionLabel } from "$lib/components/ui/section-label";
  import { SkeletonRows } from "$lib/components/ui/skeleton";
  import { Tag } from "$lib/components/ui/tag";
  import ServerDetail from "$features/servers/detail/ServerDetail.svelte";
  import { openUrl } from "$lib/ipc/native";
  import type { FriendDto, FriendStatus } from "$lib/ipc/types";
  import { app } from "$lib/stores/app.svelte";
  import { connect } from "$lib/stores/connect.svelte";
  import { friends } from "$lib/stores/friends.svelte";
  import { servers, type ServerRow } from "$lib/stores/servers.svelte";
  import { say, errorText } from "$lib/stores/say";
  import { relative } from "$lib/format";
  import { cn } from "$lib/cx";

  /**
   * The account's Steam friends: who plays DayZ and where (the server as
   * every server list shows it, one click to join, show it in the list or
   * ask it for its details), who is in another game, online, away, offline.
   * Kept fresh by the friends store while the view shows.
   */
  const fr = dict("friends");
  const nav = dict("nav");
  const sv = dict("servers");

  const data = $derived(friends.data);
  const failed = $derived(friends.failed ?? data?.error ?? null);

  // ── the clock for "updated x ago" and "joined x ago" ───────────────────
  let now = $state(Date.now());
  $effect(() => {
    if (app.view !== "friends") return;
    now = Date.now();
    const t = setInterval(() => (now = Date.now()), 15_000);
    return () => clearInterval(t);
  });
  const ago = (ms: number) => (void now, relative(Math.floor(ms / 1000)));

  // ── search ──────────────────────────────────────────────────────────────
  let query = $state("");
  /** The listed row of a friend's server, if the list has it. */
  const rowOf = (f: FriendDto): ServerRow | undefined =>
    f.server ? servers.find(f.server.ip, f.server.port) : undefined;
  const matches = (f: FriendDto, q: string) => {
    if (!q) return true;
    const r = rowOf(f);
    return [f.name, f.game, r?.name ?? f.server?.name, r?.map ?? f.server?.map].some((s) =>
      s?.toLowerCase().includes(q),
    );
  };
  const shown = $derived.by(() => {
    const q = query.trim().toLowerCase();
    return friends.list.filter((f) => matches(f, q));
  });

  // ── groups: DayZ (by server), another game, online, away, offline ──────
  const byName = (a: FriendDto, b: FriendDto) =>
    a.name.localeCompare(b.name, undefined, { sensitivity: "base" });
  type Block = { key: string; ip: string; port: number; listed: boolean; who: FriendDto[] };
  const dayz = $derived.by(() => {
    const blocks = new Map<string, Block>();
    const lost: FriendDto[] = [];
    for (const f of shown) {
      if (!f.in_dayz) continue;
      if (!f.server) {
        lost.push(f);
        continue;
      }
      const key = `${f.server.ip}:${f.server.port}`;
      const b = blocks.get(key);
      if (b) b.who.push(f);
      else
        blocks.set(key, {
          key,
          ip: f.server.ip,
          port: f.server.port,
          listed: f.server.name != null,
          who: [f],
        });
    }
    const list = [...blocks.values()];
    for (const b of list) b.who.sort(byName);
    list.sort((a, b) => b.who.length - a.who.length || byName(a.who[0]!, b.who[0]!));
    return { blocks: list, lost: lost.sort(byName), count: shown.filter((f) => f.in_dayz).length };
  });
  const rest = $derived.by(() => {
    const others = shown.filter((f) => !f.in_dayz);
    const away = (s: FriendStatus) => s === "away" || s === "busy" || s === "snooze";
    return [
      {
        key: "game",
        label: $fr.groupInGame.value,
        rows: others.filter((f) => f.game).sort(byName),
      },
      {
        key: "online",
        label: $fr.groupOnline.value,
        rows: others.filter((f) => !f.game && f.status === "online").sort(byName),
      },
      {
        key: "away",
        label: $fr.groupAway.value,
        rows: others.filter((f) => !f.game && away(f.status)).sort(byName),
      },
      {
        key: "offline",
        label: $fr.groupOffline.value,
        rows: others
          .filter((f) => !f.game && f.status === "offline")
          .sort((a, b) => (b.last_online ?? 0) - (a.last_online ?? 0) || byName(a, b)),
      },
    ];
  });

  // ── the server whose details show ───────────────────────────────────────
  let selected = $state<{ ip: string; port: number; name: string } | null>(null);
  const selectedKey = $derived(selected ? `${selected.ip}:${selected.port}` : null);
  function pick(b: Block, row: ServerRow | undefined) {
    selected = { ip: b.ip, port: b.port, name: row?.name ?? b.who[0]?.server?.name ?? "" };
  }

  // ── persona ─────────────────────────────────────────────────────────────
  const statusWord = (s: FriendStatus) =>
    ({
      online: $fr.online.value,
      busy: $fr.busy.value,
      away: $fr.away.value,
      snooze: $fr.snooze.value,
      offline: $fr.offline.value,
    })[s];
  /** Online `ok`, away and snoozing `warn`, busy `err`, offline faint. */
  const dot = (s: FriendStatus) =>
    ({
      online: "bg-ok",
      away: "bg-warn",
      snooze: "bg-warn",
      busy: "bg-err",
      offline: "bg-fg-faint/40",
    })[s];

  /** Steam's own pages for a friend, through the Steam client. Valve's
   *  "Steam browser protocol": `steam://url/SteamIDPage/<id>` opens the
   *  profile, `steam://friends/message/<id>` a chat with them
   *  (https://developer.valvesoftware.com/wiki/Steam_browser_protocol). */
  async function steam(url: string) {
    try {
      await openUrl(url);
    } catch (e) {
      say.err(errorText(e));
    }
  }
</script>

{#snippet friendRow(f: FriendDto)}
  {@const since = friends.since.get(f.steam_id)}
  <li
    class={cn(
      "group flex h-row items-center gap-3 border-b border-border/50 px-pad transition-colors duration-fast hover:bg-raised/50",
      f.status === "offline" && !f.game && "opacity-60",
    )}
  >
    <div class="relative shrink-0">
      {#if f.avatar}
        <img src={f.avatar} alt="" class="size-8 rounded-sm object-cover" loading="lazy" />
      {:else}
        <div
          class="grid size-8 place-items-center rounded-sm bg-raised font-display text-sm text-fg-muted"
        >
          {f.name.slice(0, 1).toUpperCase() || "?"}
        </div>
      {/if}
      <span
        class={cn(
          "absolute -right-0.5 -bottom-0.5 size-2.5 rounded-full ring-2 ring-panel",
          dot(f.status),
        )}
        title={statusWord(f.status)}
      ></span>
    </div>
    <div class="min-w-0 flex-1">
      <div class="truncate text-sm text-fg" data-selectable>{f.name}</div>
      <div class="truncate text-2xs text-fg-muted">
        {#if f.in_dayz}
          <span class="text-accent">DayZ</span>
          {#if !f.server}· {$fr.noServer.value}{/if}
          {#if since}· {$fr.inDayzSince({ when: ago(since) }).value}{/if}
        {:else if f.game}
          <span class="text-ok">{f.game}</span>
        {:else if f.status === "offline" && f.last_online}
          {$fr.lastOnline({ when: (void now, relative(f.last_online)) }).value}
        {:else}
          <span class={f.status === "busy" ? "text-err" : f.status === "online" ? "" : "text-warn"}
            >{statusWord(f.status)}</span
          >
        {/if}
      </div>
    </div>
    <div
      class="flex items-center gap-px rounded-sm border border-border bg-panel p-px opacity-0 transition-opacity group-hover:opacity-100 focus-within:opacity-100"
    >
      <IconButton
        icon={MessageSquare}
        size="icon-xs"
        label={$fr.sendMessage.value}
        onclick={() => steam(`steam://friends/message/${f.steam_id}`)}
      />
      <IconButton
        icon={UserRound}
        size="icon-xs"
        label={$fr.openProfile.value}
        onclick={() => steam(`steam://url/SteamIDPage/${f.steam_id}`)}
      />
    </div>
  </li>
{/snippet}

{#snippet groupHead(label: string, count: number)}
  <div class="sticky top-0 z-raised border-b border-border bg-bg px-pad py-1.5">
    <SectionLabel>
      {label}
      {#snippet actions()}
        <span class="num font-mono text-2xs text-fg-faint">{count}</span>
      {/snippet}
    </SectionLabel>
  </div>
{/snippet}

{#snippet serverRow(b: Block)}
  {@const r = servers.find(b.ip, b.port)}
  {@const first = b.who[0]?.server}
  <div style:height="{LIST_ROW_PX}px" class="border-b border-border bg-bg/40 pl-6">
    <ServerListRow
      ip={b.ip}
      queryPort={r?.query_port ?? b.port}
      gamePort={r?.game_port ?? b.port}
      joinPort={b.port}
      name={r?.name ?? first?.name ?? `${b.ip}:${b.port}`}
      map={r?.map ?? first?.map}
      time={r?.time}
      version={r?.version}
      environment={r?.environment}
      modsCount={r?.mods_count ?? 0}
      modsMissing={r?.mods_missing}
      modsStale={r?.mods_stale}
      password={r?.password}
      firstPerson={r?.first_person_only}
      battleye={r?.battl_eye}
      official={r?.official}
      mimicsOfficial={r?.mimics_official}
      listed={b.listed}
      selected={selectedKey === b.key}
      onselect={() => pick(b, r)}
      onjoin={() => void connect.address(b.ip, b.port)}
    >
      {#snippet tag()}
        {#if !b.listed}<Tag tone="warn">{$fr.unlisted.value}</Tag>{/if}
      {/snippet}
      {#snippet actions()}
        {#if b.listed}
          <IconButton
            icon={List}
            size="icon-xs"
            label={$fr.showInList.value}
            onclick={(e) => {
              e.stopPropagation();
              app.go("servers", `server:${b.ip}:${b.port}`);
            }}
          />
        {:else}
          <IconButton
            icon={ScanSearch}
            size="icon-xs"
            label={$fr.details.value}
            onclick={(e) => {
              e.stopPropagation();
              pick(b, r);
            }}
          />
        {/if}
        <IconButton
          icon={PlugZap}
          size="icon-xs"
          label={$sv.directConnect.value}
          onclick={(e) => {
            e.stopPropagation();
            connect.openInDirect(b.ip, r?.game_port ?? b.port, r?.query_port ?? b.port);
          }}
        />
      {/snippet}
    </ServerListRow>
  </div>
{/snippet}

{#snippet listPane()}
  <div class="relative min-h-0 flex-1 bg-panel">
    <div class="@container absolute inset-0 overflow-y-auto">
      {#if shown.length === 0}
        <Empty icon={SearchX} title={$fr.noMatch.value} />
      {/if}
      {#if dayz.count}
        <section>
          {@render groupHead($fr.groupDayz.value, dayz.count)}
          {#each dayz.blocks as b (b.key)}
            <ul class="m-0 list-none p-0">
              {#each b.who as f (f.steam_id)}{@render friendRow(f)}{/each}
            </ul>
            {@render serverRow(b)}
          {/each}
          <ul class="m-0 list-none p-0">
            {#each dayz.lost as f (f.steam_id)}{@render friendRow(f)}{/each}
          </ul>
        </section>
      {/if}
      {#each rest as g (g.key)}
        {#if g.rows.length}
          <section>
            {@render groupHead(g.label, g.rows.length)}
            <ul class="m-0 list-none p-0">
              {#each g.rows as f (f.steam_id)}{@render friendRow(f)}{/each}
            </ul>
          </section>
        {/if}
      {/each}
    </div>
  </div>
{/snippet}

{#snippet detailPane()}
  {#if selected}
    <ServerDetail
      ip={selected.ip}
      port={selected.port}
      name={selected.name}
      onclose={() => (selected = null)}
    />
  {:else}
    <Empty icon={Server} title={$sv.selectServer.value} />
  {/if}
{/snippet}

{#snippet refresh(variant: "default" | "accent" = "default")}
  <Button {variant} onclick={() => void friends.load()} disabled={friends.loading}>
    <RefreshCw class={cn("size-icon-sm", friends.loading && "animate-spin")} />{$fr.refresh.value}
  </Button>
{/snippet}

<div class="flex min-h-0 flex-1 flex-col">
  <PageHeader title={$nav.friends.value}>
    {#snippet stats()}
      <Figure label={$fr.statInDayz.value} amount={friends.inDayz} tone="text-accent" />
      <Figure label={$fr.statOnline.value} amount={friends.online} tone="text-ok" />
      <Figure label={$fr.statTotal.value} amount={friends.list.length} />
    {/snippet}
    {#snippet actions()}
      {#if data?.source === "steam_client"}
        <Tag title={$fr.sourceClientHint.value}>{$fr.sourceClient.value}</Tag>
      {/if}
      {#if friends.updatedAt}
        <span
          class="px-1 text-2xs text-fg-faint"
          title={data?.source === "web_api" ? $fr.sourceWeb.value : ""}
          >{$fr.updated({ when: ago(friends.updatedAt) }).value}</span
        >
      {/if}
      {@render refresh()}
    {/snippet}
    {#if friends.list.length}
      <div class="flex items-center gap-2">
        <Input
          class="w-44 min-w-32 flex-1 xl:max-w-80"
          bind:value={query}
          type="search"
          placeholder={$fr.search.value}
          clearLabel={$sv.clearSearch.value}
        />
      </div>
    {/if}
  </PageHeader>

  {#if !data && friends.loading}
    <div class="min-h-0 flex-1 overflow-hidden bg-panel"><SkeletonRows media count={10} /></div>
  {:else if failed && friends.list.length === 0}
    <Empty icon={UserX} title={$fr.unavailable.value} class="flex-1">
      <span class="text-err" data-selectable>{failed}</span>
      <br />{$fr.unavailableHint.value}
      {#snippet action()}
        <div class="flex gap-2">
          <Button onclick={() => void friends.load()} disabled={friends.loading}>
            <RefreshCw class={cn("size-icon-sm", friends.loading && "animate-spin")} />{$fr.retry
              .value}
          </Button>
          <Button variant="accent" onclick={() => app.go("settings", "apis")}
            >{$fr.openSettings.value}</Button
          >
        </div>
      {/snippet}
    </Empty>
  {:else if friends.list.length === 0}
    <Empty icon={Users} title={$fr.empty.value} class="flex-1">
      {#if data}{$fr.emptyHint.value}{/if}
      {#snippet action()}{@render refresh()}{/snippet}
    </Empty>
  {:else}
    <MasterDetail
      id="friends-detail"
      selected={selected !== null}
      initial={380}
      min={300}
      main={listPane}
      detail={detailPane}
    />
  {/if}
</div>
