<script lang="ts">
  import { onMount } from "svelte";
  import { dict } from "$lib/i18n";
  import Users from "~icons/lucide/users";
  import RefreshCw from "~icons/lucide/refresh-cw";
  import MapPin from "~icons/lucide/map-pin";
  import { Empty, Figure, JoinButton, PageHeader } from "$lib/components/app";
  import { Button } from "$lib/components/ui/button";
  import { SectionLabel } from "$lib/components/ui/section-label";
  import { SkeletonRows } from "$lib/components/ui/skeleton";
  import { Tag } from "$lib/components/ui/tag";
  import { steamFriends } from "$lib/ipc/friends";
  import { errorText } from "$lib/ipc/core";
  import type { FriendDto, FriendsDto, FriendStatus } from "$lib/ipc/types";
  import { app } from "$lib/stores/app.svelte";
  import { relative } from "$lib/format";
  import { cn } from "$lib/cx";

  /**
   * The account's Steam friends: who plays DayZ and on which server (one
   * click to join them), who is in another game, online, offline. Asked
   * again every half minute while the view is open, every minute through
   * the Steam client (each ask shows DayZ running for a moment).
   */
  const fr = dict("friends");
  const nav = dict("nav");

  let data = $state<FriendsDto | null>(null);
  let loading = $state(false);
  let failed = $state<string | null>(null);

  async function load() {
    if (loading) return;
    loading = true;
    try {
      data = await steamFriends();
      failed = null;
    } catch (e) {
      failed = errorText(e);
    } finally {
      loading = false;
    }
  }

  // Once per mount, so nothing it reads is tracked.
  onMount(() => {
    let timer: ReturnType<typeof setTimeout> | undefined;
    let stopped = false;
    const next = () => {
      if (stopped) return;
      const every = data?.source === "steam_client" ? 60_000 : 30_000;
      timer = setTimeout(() => void load().then(next), every);
    };
    void load().then(next);
    return () => {
      stopped = true;
      clearTimeout(timer);
    };
  });

  const friends = $derived(data?.friends ?? []);
  const groups = $derived([
    { key: "dayz", label: $fr.groupDayz.value, rows: friends.filter((f) => f.in_dayz) },
    {
      key: "game",
      label: $fr.groupInGame.value,
      rows: friends.filter((f) => !f.in_dayz && f.game),
    },
    {
      key: "online",
      label: $fr.groupOnline.value,
      rows: friends.filter((f) => !f.game && f.status !== "offline"),
    },
    {
      key: "offline",
      label: $fr.groupOffline.value,
      rows: friends.filter((f) => !f.game && f.status === "offline"),
    },
  ]);
  const online = $derived(friends.filter((f) => f.status !== "offline" || f.game).length);
  const inDayz = $derived(friends.filter((f) => f.in_dayz).length);

  const statusWord = (s: FriendStatus) =>
    ({
      online: $fr.online.value,
      busy: $fr.busy.value,
      away: $fr.away.value,
      snooze: $fr.snooze.value,
      offline: $fr.offline.value,
    })[s];
  const dot = (f: FriendDto) =>
    f.in_dayz
      ? "bg-accent"
      : f.game
        ? "bg-ok"
        : f.status === "offline"
          ? "bg-fg-faint/40"
          : f.status === "online"
            ? "bg-info"
            : "bg-warn";
</script>

{#snippet row(f: FriendDto)}
  <li
    class={cn(
      "group flex min-h-row items-center gap-3 border-b border-border/50 px-pad py-1.5 transition-colors duration-fast hover:bg-raised/50",
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
          dot(f),
        )}
      ></span>
    </div>
    <div class="min-w-0 flex-1">
      <div class="truncate text-sm text-fg" data-selectable>{f.name}</div>
      <div class="truncate text-2xs text-fg-muted">
        {#if f.in_dayz && f.server}
          {#if f.server.name}
            <span class="text-fg">{f.server.name}</span>
          {:else}
            <span class="font-mono" data-selectable>{f.server.ip}:{f.server.port}</span>
            · {$fr.unlisted.value}
          {/if}
        {:else if f.in_dayz}
          {$fr.noServer.value}
        {:else if f.game}
          <span class="text-ok">{f.game}</span>
        {:else if f.status === "offline" && f.last_online}
          {$fr.lastOnline({ when: relative(f.last_online) }).value}
        {:else}
          {statusWord(f.status)}
        {/if}
      </div>
    </div>
    {#if f.in_dayz && f.server}
      {#if f.server.map}
        <span class="hidden shrink-0 items-center gap-1 text-2xs text-fg-muted @min-[560px]:flex">
          <MapPin class="size-3" />{f.server.map}
        </span>
      {/if}
      {#if f.server.players != null}
        <span class="num w-14 shrink-0 text-right font-mono text-xs text-fg">
          {f.server.players}/{f.server.max_players}
        </span>
      {/if}
      <JoinButton ip={f.server.ip} port={f.server.port} quiet />
    {/if}
  </li>
{/snippet}

<div class="flex min-h-0 flex-1 flex-col">
  <PageHeader title={$nav.friends.value}>
    {#snippet stats()}
      <Figure label={$fr.statInDayz.value} amount={inDayz} tone="text-accent" />
      <Figure label={$fr.statOnline.value} amount={online} tone="text-ok" />
      <Figure label={$fr.statTotal.value} amount={friends.length} />
    {/snippet}
    {#snippet actions()}
      {#if data?.source}
        <Tag title={data.source === "steam_client" ? $fr.sourceClientHint.value : ""}>
          {data.source === "web_api" ? $fr.sourceWeb.value : $fr.sourceClient.value}
        </Tag>
      {/if}
      <!-- Refresh, as the server browser has it. -->
      <Button onclick={() => void load()} disabled={loading}>
        <RefreshCw class={cn("size-icon-sm", loading && "animate-spin")} />{$fr.refresh.value}
      </Button>
    {/snippet}
  </PageHeader>

  {#if !data && loading}
    <div class="min-h-0 flex-1 overflow-hidden bg-panel"><SkeletonRows media count={10} /></div>
  {:else if failed || data?.error}
    <Empty icon={Users} title={$fr.unavailable.value}>
      <span data-selectable>{failed ?? data?.error}</span>
      <br />{$fr.unavailableHint.value}
      {#snippet action()}
        <Button variant="accent" onclick={() => app.go("settings", "apis")}
          >{$fr.openSettings.value}</Button
        >
      {/snippet}
    </Empty>
  {:else if friends.length === 0}
    <Empty icon={Users} title={$fr.empty.value}>
      {#snippet action()}
        <Button onclick={() => void load()} disabled={loading}>
          <RefreshCw class={cn("size-icon-sm", loading && "animate-spin")} />{$fr.refresh.value}
        </Button>
      {/snippet}
    </Empty>
  {:else}
    <div class="relative min-h-0 flex-1 bg-panel">
      <div class="@container absolute inset-0 overflow-y-auto">
        {#each groups as g (g.key)}
          {#if g.rows.length}
            <section>
              <div class="sticky top-0 z-raised border-b border-border bg-bg px-pad py-1.5">
                <SectionLabel>
                  {g.label}
                  {#snippet actions()}
                    <span class="num font-mono text-2xs text-fg-faint">{g.rows.length}</span>
                  {/snippet}
                </SectionLabel>
              </div>
              <ul class="m-0 list-none p-0">
                {#each g.rows as f (f.steam_id)}{@render row(f)}{/each}
              </ul>
            </section>
          {/if}
        {/each}
      </div>
    </div>
  {/if}
</div>
