<script lang="ts">
  import { dict } from "$lib/i18n";
  import Plug from "~icons/lucide/plug";
  import Info from "~icons/lucide/info";
  import Puzzle from "~icons/lucide/puzzle";
  import Users from "~icons/lucide/users";
  import History from "~icons/lucide/history";
  import Download from "~icons/lucide/download";
  import PlugZap from "~icons/lucide/plug-zap";
  import CircleCheck from "~icons/lucide/circle-check";
  import Star from "~icons/lucide/star";
  import { Button } from "$lib/components/ui/button";
  import { Copy } from "$lib/components/ui/copy";
  import { Spinner } from "$lib/components/ui/spinner";
  import { Facts, Section, type Fact } from "$lib/components/app";
  import { mapName } from "$lib/components/app/map-name";
  import { dzchLink } from "$lib/dzch";
  import { connect } from "$lib/stores/connect.svelte";
  import { profile } from "$lib/stores/profile.svelte";
  import { duration, num, relative, dateTime } from "$lib/format";
  import type { DetailModel } from "./model.svelte";
  import type { DetailTab } from "./tab.svelte";
  import PopulationWarning from "./PopulationWarning.svelte";
  import { untilText } from "./until";

  /** The panel's first page: how to join, what the server is, what it will cost, who is on, and you. */
  let { m, go }: { m: DetailModel; go: (tab: DetailTab) => void } = $props();
  const c = dict("detail");

  const yesNo = (v: boolean | null | undefined) =>
    v == null ? "—" : v ? $c.yes.value : $c.no.value;

  /** The server as a dzch:// link, which opens this app straight on it. */
  const shareLink = $derived(
    dzchLink({
      ip: m.ip,
      gamePort: m.gamePort,
      queryPort: m.queryPort,
      name: m.title,
      modIds: m.modRows.map((r) => r.id),
    }),
  );

  const facts = $derived.by((): Fact[] => {
    const s = m.listed;
    const f: Fact[] = [{ label: $c.map.value, value: mapName(m.map) || "—", tone: "text-map" }];
    if (s) {
      f.push(
        { label: $c.version.value, value: s.version || "—", tone: "text-fg-muted" },
        {
          label: $c.platform.value,
          value: s.environment === "w" ? $c.windows.value : $c.linux.value,
        },
        {
          label: $c.battleye.value,
          value: yesNo(s.battl_eye),
          tone: s.battl_eye ? "text-ok" : "text-fg-muted",
        },
        {
          label: $c.firstPerson.value,
          value: yesNo(s.first_person_only),
          tone: s.first_person_only ? "text-warn" : "text-fg-muted",
        },
        { label: $c.vac.value, value: yesNo(s.vac), tone: s.vac ? "text-ok" : "text-fg-muted" },
      );
    } else if (m.a2s?.version) {
      f.push({ label: $c.version.value, value: m.a2s.version, tone: "text-fg-muted" });
    }
    // What the server states about itself over A2S; the list already says
    // BattlEye and first person for a listed one.
    const d = m.a2s?.dayz;
    if (d) {
      if (!s) {
        f.push(
          {
            label: $c.battleye.value,
            value: yesNo(d.battleye),
            tone: d.battleye ? "text-ok" : "text-fg-muted",
          },
          {
            label: $c.firstPerson.value,
            value: yesNo(d.first_person_only),
            tone: d.first_person_only ? "text-warn" : "text-fg-muted",
          },
        );
      }
      f.push({
        label: $c.hive.value,
        value: d.official
          ? $c.hiveOfficial.value
          : d.private_hive
            ? $c.hivePrivate.value
            : $c.hivePublic.value,
        tone: d.official ? "text-accent" : "text-fg-muted",
        title: $c.hiveHint.value,
      });
      if (d.login_queue != null) {
        f.push({
          label: $c.loginQueue.value,
          value:
            d.login_queue > 0
              ? $c.queueWaiting({ count: d.login_queue }).value
              : $c.queueEmpty.value,
          tone: d.login_queue > 0 ? "text-warn" : "text-fg-muted",
        });
      }
      if (d.time_accel != null) {
        const x = (v: number) => num(Math.round(v * 10) / 10);
        f.push({
          label: $c.dmTimeSpeed.value,
          value: $c.dmTimeSpeedValue({
            day: x(d.time_accel),
            night: x(d.night_time_accel ?? d.time_accel),
          }).value,
        });
      }
      if (d.whitelisted)
        f.push({ label: $c.whitelist.value, value: $c.yes.value, tone: "text-warn" });
    }
    if (m.count) {
      f.push(
        { label: $c.maxPlayers.value, value: num(m.count.max) },
        { label: $c.realPlayers.value, value: num(Math.max(0, m.count.players - m.count.bots)) },
      );
      if (m.count.bots > 0)
        f.push({ label: $c.bots.value, value: num(m.count.bots), tone: "text-warn" });
    }
    const x = m.metrics;
    const rank = x?.rank_pos ?? null;
    if (rank != null) f.push({ label: $c.rank.value, value: `#${num(rank)}`, tone: "text-accent" });
    const up = x?.uptime_7d ?? null;
    if (up != null) {
      f.push({
        label: $c.uptime.value,
        value: `${up.toFixed(1)}%`,
        tone: up >= 90 ? "text-ok" : up >= 70 ? "text-warn" : "text-err",
      });
    }
    const restart = x?.restart?.next_restart ? untilText(x.restart.next_restart) : null;
    if (restart) f.push({ label: $c.dmNextRestart.value, value: $c.dmIn({ time: restart }).value });
    const wipe = x?.wipe;
    if (wipe?.next) {
      f.push({
        label: $c.dmNextWipe.value,
        value:
          wipe.days_until != null
            ? $c.dmInDays({ days: Math.round(wipe.days_until) }).value
            : wipe.next,
        title: `${wipe.next} · ${wipe.next_source === "announced" ? $c.dmAnnounced.value : $c.dmPredicted.value}`,
      });
    }
    return f;
  });

  const missing = $derived(m.modTotals.missing.length);
  const stale = $derived(m.modTotals.stale.length);
  const top = $derived(m.players.slice(0, 5));
  const lastPlayed = $derived(m.history[0]?.ts ?? null);
  const fav = $derived(
    profile.isFavorite(m.ip, m.queryPort) || profile.isFavorite(m.ip, m.gamePort),
  );
</script>

{#if m.population === "fake" || m.population === "suspect"}
  <div class="px-pad pt-2.5"><PopulationWarning {m} /></div>
{/if}

<Section icon={Plug} title={$c.connection.value}>
  <dl class="m-0 grid grid-cols-[auto_minmax(0,1fr)] items-center gap-x-3 gap-y-1.5 text-2xs">
    <dt class="text-fg-faint">{$c.address.value}</dt>
    <dd class="m-0 min-w-0">
      <Copy text={m.address} title={$c.copyIp.value} class="text-xs text-fg" />
    </dd>
    <dt class="text-fg-faint">{$c.gamePort.value} · {$c.queryPort.value}</dt>
    <dd class="m-0 font-mono text-fg-muted">{m.gamePort} · {m.queryPort}</dd>
    {#if m.listed?.password}
      <dt class="text-fg-faint">{$c.password.value}</dt>
      <dd class="m-0 text-err">{$c.passwordRequired.value}</dd>
    {/if}
    <dt class="text-fg-faint">{$c.shareLink.value}</dt>
    <dd class="m-0 min-w-0"><Copy text={shareLink} title={$c.shareLinkHint.value} /></dd>
  </dl>
  <div>
    <Button size="xs" onclick={() => connect.openInDirect(m.ip, m.gamePort, m.queryPort)}>
      <PlugZap class="size-3" />{$c.openDirect.value}
    </Button>
  </div>
</Section>

<Section icon={Info} title={$c.details.value}>
  <Facts items={facts} />
  {#if m.live.error && !m.a2s}
    <div
      class="flex items-center gap-2 rounded-sm border border-err/30 bg-err/10 px-2 py-1.5 text-2xs text-err"
    >
      <span class="min-w-0 flex-1">{$c.liveFailed.value}</span>
    </div>
  {/if}
</Section>

<Section icon={Puzzle} title={$c.modsState.value}>
  {#if m.listed && m.listed.mods_count > 0}
    {#if m.modsEntry?.loading && !m.modsEntry.mods}
      <div class="flex items-center gap-2 text-2xs text-fg-faint">
        <Spinner class="size-3.5" />{$c.loadingMods({ count: m.listed.mods_count }).value}
      </div>
    {:else if m.modRows.length > 0}
      <div class="flex flex-wrap items-center gap-1.5 text-2xs">
        <span class="rounded-xs bg-ok/12 px-1.5 py-0.5 font-mono text-ok"
          >{m.modTotals.installed} ✓</span
        >
        {#if missing}<span class="rounded-xs bg-err/12 px-1.5 py-0.5 font-mono text-err"
            >{missing} {$c.modMissing.value}</span
          >{/if}
        {#if stale}<span class="rounded-xs bg-warn/12 px-1.5 py-0.5 font-mono text-warn"
            >{stale} {$c.modStale.value}</span
          >{/if}
        <button
          class="ml-auto text-fg-faint underline-offset-2 hover:text-fg hover:underline"
          onclick={() => go("mods")}
        >
          {$c.seeAll({ count: m.modRows.length }).value}
        </button>
      </div>
      {#if missing > 0 || stale > 0}
        <Button variant="accent" onclick={() => m.listed && connect.server(m.listed)}>
          <Download class="size-icon-sm" />
          {missing > 0
            ? $c.downloadAndJoin({ count: missing }).value
            : $c.updateAndJoin({ count: stale }).value}
        </Button>
      {:else}
        <p class="m-0 flex items-center gap-1.5 text-2xs text-ok">
          <CircleCheck class="size-3.5" />{$c.allInstalled.value}
        </p>
      {/if}
    {:else if m.modsEntry?.error}
      <p class="m-0 text-2xs text-warn">{$c.modsFailed({ count: m.listed.mods_count }).value}</p>
    {/if}
  {:else if m.modsCount > 0}
    <p class="m-0 text-2xs text-fg-muted">{$c.modsFromA2s.value}</p>
  {:else}
    <p class="m-0 text-2xs text-fg-faint italic">{$c.noMods.value}</p>
  {/if}
</Section>

<Section icon={Users} title={$c.playersNow.value}>
  {#if m.live.loading && !m.a2s}
    <div class="flex flex-col gap-1">
      {#each [0, 1, 2] as i (i)}<div class="h-5 animate-pulse rounded-xs bg-raised/60"></div>{/each}
    </div>
  {:else if top.length > 0}
    <span class="label-stencil text-fg-faint">{$c.longestHere.value}</span>
    <ol class="m-0 flex list-none flex-col p-0">
      {#each top as p, i (i)}
        <li class="flex h-6 items-center gap-2 text-2xs">
          <span class="num w-4 shrink-0 text-right font-mono text-fg-faint">{i + 1}</span>
          <span class="min-w-0 flex-1 truncate text-fg" data-selectable>{p.name || "—"}</span>
          <span class="num shrink-0 font-mono text-fg-muted">{duration(p.duration ?? 0)}</span>
        </li>
      {/each}
    </ol>
    {#if m.players.length > top.length}
      <button
        class="self-start text-2xs text-fg-faint underline-offset-2 hover:text-fg hover:underline"
        onclick={() => go("players")}
      >
        {$c.seeAll({ count: m.players.length }).value}
      </button>
    {/if}
  {:else if m.live.error && !m.a2s}
    <p class="m-0 text-2xs text-err">{$c.liveFailed.value}</p>
  {:else if m.a2s && m.a2s.players === 0}
    <p class="m-0 text-2xs text-fg-faint italic">{$c.a2sNoPlayers.value}</p>
  {:else if m.a2s}
    <p class="m-0 text-2xs text-fg-faint italic">{$c.a2sNamesNotReported.value}</p>
  {:else}
    <p class="m-0 text-2xs text-fg-faint italic">{$c.a2sClickRefresh.value}</p>
  {/if}
</Section>

<Section icon={History} title={$c.you.value}>
  <ul class="m-0 flex list-none flex-col gap-1 p-0 text-2xs">
    <li class="flex items-center gap-2">
      <History class="size-3.5 shrink-0 text-fg-faint" />
      {#if lastPlayed !== null}
        <span class="text-fg" title={dateTime(lastPlayed)}
          >{$c.lastPlayedHere({ when: relative(lastPlayed) }).value}</span
        >
        <span class="text-fg-faint">·</span>
        <span class="text-fg-muted">
          {m.history.length === 1
            ? $c.sessionsOne.value
            : $c.sessions({ count: m.history.length }).value}
        </span>
      {:else}
        <span class="text-fg-faint">{$c.neverPlayed.value}</span>
      {/if}
    </li>
    <li class="flex items-center gap-2">
      <Star class="size-3.5 shrink-0 {fav ? 'fill-warn text-warn' : 'text-fg-faint'}" />
      <span class={fav ? "text-fg" : "text-fg-faint"}
        >{fav ? $c.inFavorites.value : $c.notInFavorites.value}</span
      >
    </li>
  </ul>
</Section>
