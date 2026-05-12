<script lang="ts">
  import '../app.css';
  import { invoke } from '@tauri-apps/api/core';
  import { listen } from '@tauri-apps/api/event';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { openUrl as shellOpen } from '@tauri-apps/plugin-opener';
  import { onMount, untrack } from 'svelte';

  import type { CliArgs, A2sDetailsDto, ServerDto, TabId } from '$lib/types';

  import Icon from '@iconify/svelte';
  import TitleBar from '$lib/components/TitleBar.svelte';
  import TabBar from '$lib/components/TabBar.svelte';
  import ServersTab from '$lib/components/ServersTab.svelte';
  // Type-only import keeps `ReturnType<typeof ModsTabType>` available for
  // `modsTabRef` while letting the runtime import stay lazy below.
  import type ModsTabType from '$lib/components/ModsTab.svelte';

  // Lazy-loaded tabs.  Only ServersTab is eager because it's the default
  // landing tab; everything else is dynamic-imported on first activation.
  const lazyTabs = {
    favorites: () => import('$lib/components/FavoritesTab.svelte'),
    history: () => import('$lib/components/HistoryTab.svelte'),
    mods: () => import('$lib/components/ModsTab.svelte'),
    connect: () => import('$lib/components/DirectConnectTab.svelte'),
    options: () => import('$lib/components/OptionsTab.svelte'),
    news: () => import('$lib/components/NewsTab.svelte'),
    offline: () => import('$lib/components/OfflineTab.svelte'),
    about: () => import('$lib/components/AboutTab.svelte'),
    setup: () => import('$lib/components/SetupWizard.svelte'),
  };
  import ConfirmModal from '$lib/components/ConfirmModal.svelte';
  import ProgressModal from '$lib/components/ProgressModal.svelte';
  import ConnectModal from '$lib/components/ConnectModal.svelte';
  import ExcludedIpsModal from '$lib/components/ExcludedIpsModal.svelte';
  import ToastContainer from '$lib/components/ToastContainer.svelte';
  import ModConfirmModal from '$lib/components/ModConfirmModal.svelte';

  // ── Shared state ────────────────────────────────────────────────────────
  import { app as s, AWAY_SERVERS_MS, AWAY_MODS_MS } from '$lib/state.svelte';
  import * as m from '$lib/paraglide/messages.js';
  import { formatRelativeTime } from '$lib/utils/i18n';
  import { localeState } from '$lib/locale.svelte';

  // ── Action modules ──────────────────────────────────────────────────────
  import { refreshServers, pingSingle, loadStats } from '$lib/actions/servers';
  import { loadProfile, saveProfileSettings, toggleOption, setOptionValue } from '$lib/actions/profile';
  import {
    addFavorite,
    addFavoriteDirect,
    removeFavorite,
    removeFavoriteQuick,
    excludeIp,
    unexcludeIp,
  } from '$lib/actions/favorites';
  import {
    loadMods,
    checkModUpdates,
    deleteMod,
    toggleModManaged,
    updateMod,
    updateAllMods,
    updateStaleMods,
    cleanupMods,
    deleteSelectedMods,
    updateSelectedMods,
    installMods,
    dismissModOp,
    sendSteamcmdPassword,
    cancelModOperation,
  } from '$lib/actions/mods';
  import { connectToServer, connectByAddress, connectDirect, openInDirectConnect } from '$lib/actions/connect';
  import { removeHistoryEntry, clearHistory, addFavoriteFromHistory } from '$lib/actions/history';
  import {
    loadOfflineMissions,
    updateOfflineMode,
    launchOfflineMission,
    removeOfflineMode,
    removeMission,
    clearOfflineSaves,
  } from '$lib/actions/offline';
  import { loadNews } from '$lib/actions/news';
  import { exportProfile, importProfile, resetProfile } from '$lib/actions/profile-io';
  import { checkForUpdate, installUpdate } from '$lib/actions/updater';
  import { handleCliArgs } from '$lib/actions/cli';
  import { doInitialize, selectTab } from '$lib/actions/init';
  import { serverData } from '$lib/services/server-data.svelte';

  // ── Window state ────────────────────────────────────────────────────────
  let isMaximized = $state(false);
  let isFocused = $state(true);

  // ── Component refs ─────────────────────────────────────────────────────────
  let modsTabRef = $state<ReturnType<typeof ModsTabType> | null>(null);

  // ── Verify full servers via A2S ─────────────────────────────────────────
  // The API marks many servers as "full" optimistically; we re-verify each one
  // via A2S so the UI can flag stale player counts. With 18k+ servers this can
  // queue hundreds of jobs, so we cap concurrency at 8 to avoid drowning the
  // initial ping scan and the UI render in invoke() calls.
  const fullServerVerified = new Set<string>();
  const verifyQueue: Array<{ ip: string; port: number; server: ServerDto }> = [];
  let verifyDraining = false;
  const VERIFY_CONCURRENCY = 8;

  async function drainVerifyQueue() {
    if (verifyDraining) return;
    verifyDraining = true;
    const workers = Array.from({ length: VERIFY_CONCURRENCY }, async () => {
      while (verifyQueue.length > 0) {
        const job = verifyQueue.shift();
        if (!job) break;
        try {
          const result = await serverData.refreshA2s(job.ip, job.port);
          if (result) {
            job.server.players = result.players;
            job.server.max_players = result.max_players;
          }
        } catch {
          // Failures land in app.a2sFailures via the service.
        }
      }
    });
    await Promise.all(workers);
    verifyDraining = false;
  }

  // Effect: When the servers list changes, queue A2S verification for all
  // "full" servers.  We depend ONLY on `s.servers.length` — reading `players`
  // / `max_players` inside `untrack` so the worker mutations below don't
  // cause this effect to re-run, which would re-scan all 18k entries on
  // every single verify result (a feedback loop).
  $effect(() => {
    const len = s.servers.length;
    if (len === 0) return;

    untrack(() => {
      for (const server of s.servers) {
        if (server.players > 0 && server.players === server.max_players) {
          const key = `${server.ip}:${server.query_port}`;
          if (fullServerVerified.has(key)) continue;
          fullServerVerified.add(key);
          verifyQueue.push({ ip: server.ip, port: server.query_port, server });
        }
      }
      drainVerifyQueue();
    });
  });

  // ── Derived helpers ───────────────────────────────────────────────────────
  let favoritesSet = $derived(new Set(s.profile?.favorites.map((f) => `${f.ip}:${f.port}`) ?? []));

  // Include localeState.locale as a dependency so tabs recompute when language changes
  let tabs = $derived(
    (localeState.locale,
    [
      { id: 'servers' as TabId, label: m.tab_servers(), count: s.servers.length },
      { id: 'favorites' as TabId, label: m.tab_favorites(), count: s.profile?.favorites.length ?? 0 },
      { id: 'history' as TabId, label: m.tab_history(), count: s.profile?.history.length ?? 0 },
      { id: 'mods' as TabId, label: m.tab_mods(), count: s.installedMods.length },
      { id: 'news' as TabId, label: m.tab_news() },
      { id: 'connect' as TabId, label: m.tab_connect() },
      { id: 'options' as TabId, label: m.tab_options() },
      { id: 'offline' as TabId, label: m.tab_offline() },
      { id: 'about' as TabId, label: m.tab_about(), icon: 'mdi:information', pushRight: true },
    ]),
  );

  let staleModCount = $derived(s.installedMods.filter((m) => m.update_available).length);

  let lastHistoryEntry = $derived(!s.quickConnectDismissed ? (s.profile?.history?.[0] ?? null) : null);

  let excludedIpsSet = $derived(new Set<string>(s.profile?.excluded_ips ?? []));

  let dlPercent = $derived(s.dlTotal > 0 ? Math.round((s.dlReceived / s.dlTotal) * 100) : 0);

  // ── Last-played banner A2S refresh ───────────────────────────────────────
  $effect(() => {
    s.profile?.history?.[0]?.ip;
    s.bannerA2s = null;
  });

  async function refreshBannerA2s() {
    const lh = s.profile?.history?.[0] ?? null;
    if (!lh) return;
    const sv = s.servers.find((svr) => svr.ip === lh.ip && (svr.query_port === lh.port || svr.game_port === lh.port));
    const queryPort = sv ? sv.query_port : lh.port;
    const gamePort = sv ? sv.game_port : lh.port;
    s.bannerA2sLoading = true;
    try {
      s.bannerA2s = await invoke<A2sDetailsDto>('query_a2s', {
        ip: lh.ip,
        queryPort,
        gamePort,
      });
    } catch {
      // silently ignore
    } finally {
      s.bannerA2sLoading = false;
    }
  }

  // ── Away-time handling ─────────────────────────────────────────────────
  function handleWindowHide() {
    s.awayAt = Date.now();
  }

  function handleWindowShow() {
    if (s.awayAt === null) return;
    const elapsed = Date.now() - s.awayAt;
    s.awayAt = null;
    s.titleGlitchTick += 1;
    if (elapsed >= AWAY_SERVERS_MS) {
      refreshServers();
    }
    if (elapsed >= AWAY_MODS_MS && s.profile?.steam_api_key) {
      checkModUpdates();
    }
  }

  // ── News & misc ───────────────────────────────────────────────────────
  function openUrl(url: string) {
    shellOpen(url).catch(() => {
      window.open(url, '_blank');
    });
  }

  // ── Global keyboard shortcuts ─────────────────────────────────────────
  function handleGlobalKeydown(e: KeyboardEvent) {
    const tag = (e.target as HTMLElement)?.tagName;
    const isInput = tag === 'INPUT' || tag === 'TEXTAREA' || tag === 'SELECT';

    if (e.ctrlKey && !e.shiftKey && !e.altKey && !e.metaKey) {
      const n = parseInt(e.key, 10);
      if (n >= 1 && n <= tabs.length) {
        e.preventDefault();
        selectTab(tabs[n - 1].id);
        return;
      }
      if (e.key === 'r' && !isInput) {
        e.preventDefault();
        refreshServers();
        return;
      }
      if (e.key === 'u' && !isInput) {
        e.preventDefault();
        if (staleModCount > 0) {
          selectTab('mods');
          setTimeout(() => modsTabRef?.triggerUpdateStale(), 0);
        } else {
          s.setStatus(m.mods_up_to_date(), 'success');
        }
        return;
      }
      if (e.key === 'l' && !isInput) {
        e.preventDefault();
        const last = s.profile?.history?.[0] ?? null;
        if (last) {
          connectByAddress(last.ip, last.port, last.name);
        } else {
          s.setStatus(m.no_history(), 'warning');
        }
        return;
      }
    }
  }

  // ── Event listeners & lifecycle ───────────────────────────────────────
  let cleanupFns: Array<() => void> = [];

  onMount(() => {
    window.addEventListener('keydown', handleGlobalKeydown);
    cleanupFns.push(() => window.removeEventListener('keydown', handleGlobalKeydown));

    // Track window maximize state for conditional rounded corners
    const win = getCurrentWindow();
    win.isMaximized().then((m) => (isMaximized = m));
    win
      .onResized(() => {
        win.isMaximized().then((m) => (isMaximized = m));
      })
      .then((unlisten) => cleanupFns.push(unlisten));

    const onVisibilityChange = () => (document.hidden ? handleWindowHide() : handleWindowShow());
    document.addEventListener('visibilitychange', onVisibilityChange);
    window.addEventListener('blur', () => {
      handleWindowHide();
      isFocused = false;
    });
    window.addEventListener('focus', () => {
      handleWindowShow();
      isFocused = true;
    });
    cleanupFns.push(() => {
      document.removeEventListener('visibilitychange', onVisibilityChange);
      window.removeEventListener('blur', handleWindowHide);
      window.removeEventListener('focus', handleWindowShow);
    });

    s.loadTheme();
    s.loadWindowSettings();

    doInitialize();

    // Ping results now handled via Tauri Channels in servers.ts (handlePingBatch)
    // for better performance than event emission.

    listen<string>('launch-done', ({ payload }) => {
      s.setStatus(m.status_launched({ name: payload }), 'success');
      loadProfile();
    }).then((fn) => cleanupFns.push(fn));

    listen<string>('launch-error', ({ payload }) => {
      s.setStatus(m.status_launch_error({ error: payload }), 'error');
    }).then((fn) => cleanupFns.push(fn));

    listen('offline-mode-updated', () => {
      s.offlineLoading = false;
      s.offlineStatus = m.status_offline_updated();
      s.offlineStatusKind = 'success';
      loadOfflineMissions();
    }).then((fn) => cleanupFns.push(fn));

    listen<string>('offline-mode-error', ({ payload }) => {
      s.offlineLoading = false;
      s.offlineStatus = m.status_update_failed({ error: payload });
      s.offlineStatusKind = 'error';
    }).then((fn) => cleanupFns.push(fn));

    // CLI args — read once at startup, then listen for a second instance.
    invoke<CliArgs>('get_cli_args').then((args) => {
      if (args.connect || args.reconnect) handleCliArgs(args);
    });
    listen<CliArgs>('cli-args', ({ payload }) => {
      handleCliArgs(payload);
    }).then((fn) => cleanupFns.push(fn));

    // Periodic mod update check — every 30 minutes.
    s.modUpdateInterval = setInterval(
      () => {
        if (s.profile?.steam_api_key) {
          checkModUpdates();
        }
      },
      30 * 60 * 1000,
    );

    return () => {
      cleanupFns.forEach((fn) => fn());
      if (s.statusTimeout) clearTimeout(s.statusTimeout);
      if (s.modUpdateInterval) clearInterval(s.modUpdateInterval);
    };
  });
</script>

{#key localeState.locale}
  <div
    class="flex flex-col h-screen w-screen overflow-hidden bg-base-100 text-base-content"
    data-theme={s.theme}
    style:border-radius={!isMaximized ? s.windowRadius : '0'}
    style:border={isFocused
      ? `${s.windowBorderSize} solid ${s.windowBorderFocus}`
      : `${s.windowBorderSize} solid ${s.windowBorderBlur}`}
    style:--window-radius={!isMaximized ? s.windowRadius : '0'}
    style:--window-border-size={s.windowBorderSize}
  >
    <TitleBar
      stats={s.stats}
      avatarUrl={s.avatarUrl}
      steamPlayers={s.steamPlayers}
      theme={s.theme}
      profile={s.profile}
      {staleModCount}
      updateState={s.updateState}
      glitchTick={s.titleGlitchTick}
      onSetTheme={(t) => s.setTheme(t)}
      onSaveSettings={saveProfileSettings}
      onUnexcludeIp={unexcludeIp}
      onOpenExcludedIps={() => {
        s.showExcludedIpsModal = true;
      }}
      onUpdateMods={() => {
        selectTab('mods');
        // Use timeout to ensure tab switch happens before triggering modal
        setTimeout(() => modsTabRef?.triggerUpdateStale(), 0);
      }}
      onGoToUpdate={() => selectTab('about')}
    />
    <TabBar activeTab={s.activeTab} {tabs} onSelect={selectTab} />

    <ToastContainer />

    <!-- Quick-connect banner: last played server, shown on Servers tab -->
    {#if lastHistoryEntry && s.activeTab === 'servers' && s.initialized}
      {@const _lh = lastHistoryEntry}
      {@const _lhServer = s.servers.find(
        (sv) => sv.ip === _lh.ip && (sv.query_port === _lh.port || sv.game_port === _lh.port),
      )}
      {@const _lhPing = _lhServer
        ? (s.pingCache.get(`${_lhServer.ip}:${_lhServer.query_port}`) ?? s.pingCache.get(`${_lh.ip}:${_lh.port}`))
        : s.pingCache.get(`${_lh.ip}:${_lh.port}`)}
      <div class="flex items-center gap-3 px-3 py-1.5 bg-base-200 border-b border-base-300 flex-shrink-0 text-xs">
        <!-- Icon + label -->
        <div class="flex items-center gap-1.5 shrink-0 text-base-content/40">
          <Icon icon="ph:clock-counter-clockwise" class="size-3.5" />
          <span class="uppercase tracking-wide" style="font-size:10px;">{m.last_played()}</span>
        </div>

        <!-- Server name (clickable → opens in History tab with detail) -->
        <button
          class="font-semibold text-base-content/85 truncate hover:text-primary hover:underline transition-colors cursor-pointer text-left"
          onclick={() => {
            s.historyAutoOpenDetail = true;
            selectTab('history');
          }}
        >
          {_lh.name}
        </button>

        <!-- Live stats — only if server is in the list -->
        {#if _lhServer}
          {@const _players = s.bannerA2s?.players ?? _lhServer.players}
          {@const _maxPlayers = s.bannerA2s?.max_players ?? _lhServer.max_players}
          <span class="w-px h-3.5 bg-base-300 shrink-0"></span>
          <!-- Players — click to refresh via A2S -->
          <button
            class="flex items-center gap-1 shrink-0 tabular-nums cursor-pointer hover:opacity-70 transition-opacity
                 {_players >= _maxPlayers
              ? 'text-error'
              : _players > _maxPlayers / 2
                ? 'text-warning'
                : 'text-success'}"
            onclick={refreshBannerA2s}
            title={m.last_played_refresh()}
            disabled={s.bannerA2sLoading}
          >
            {#if s.bannerA2sLoading}
              <span class="loading loading-spinner" style="width:10px;height:10px;"></span>
            {:else}
              <Icon icon="ph:users" class="size-3 shrink-0" />
            {/if}
            {_players}/{_maxPlayers}
          </button>
          <!-- Map -->
          <span class="flex items-center gap-1 shrink-0 text-accent-map/80">
            <Icon icon="ph:map-trifold" class="size-3 shrink-0" />
            {_lhServer.map}
          </span>
          <!-- Ping — click to re-ping -->
          <button
            class="flex items-center gap-1 shrink-0 tabular-nums font-mono cursor-pointer hover:opacity-70 transition-opacity
                 {_lhPing !== undefined
              ? _lhPing < 50
                ? 'text-success'
                : _lhPing < 100
                  ? 'text-warning'
                  : 'text-error'
              : 'text-base-content/30'}"
            onclick={() => pingSingle(_lhServer.ip, _lhServer.query_port)}
            title={m.last_played_ping()}
          >
            <Icon icon="ph:wave-triangle" class="size-3 shrink-0" />
            {_lhPing !== undefined ? `${_lhPing}ms` : '—'}
          </button>
          <!-- In-game time -->
          {#if _lhServer.time}
            <span class="flex items-center gap-1 shrink-0 text-base-content/50 tabular-nums font-mono">
              <Icon icon={s.timeIcon(_lhServer.time)} class="size-3 shrink-0" />
              {_lhServer.time}
            </span>
          {/if}
        {:else}
          <span class="shrink-0 text-warning/70" style="font-size:10px;" title={m.last_played_offline_hint()}
            >{m.last_played_offline()}</span
          >
        {/if}

        <!-- Time -->
        <span class="text-base-content/30 shrink-0 ml-auto" title={new Date(_lh.ts * 1000).toLocaleString()}>
          {formatRelativeTime(_lh.ts)}
        </span>

        <!-- Reconnect -->
        <button
          class="btn btn-primary btn-xs gap-1.5 shrink-0"
          onclick={() => connectByAddress(_lh.ip, _lh.port, _lh.name)}
        >
          <Icon icon="solar:undo-right-bold" class="size-3.5" />
          {m.last_played_reconnect()}
        </button>

        <!-- Dismiss -->
        <button
          class="size-5 flex items-center justify-center rounded text-base-content/30 hover:text-base-content/70 hover:bg-base-300 transition-colors shrink-0"
          onclick={() => (s.quickConnectDismissed = true)}
          title={m.last_played_dismiss()}
        >
          <Icon icon="ph:x" class="size-3" />
        </button>
      </div>
    {/if}

    <div class="flex-1 overflow-hidden">
      {#if !s.initialized}
        <div class="flex flex-col items-center justify-center h-full gap-4">
          {#if s.initError}
            <div class="text-error text-lg font-semibold">{m.init_failed()}</div>
            <div class="text-error/70 text-sm max-w-md text-center">{s.initError}</div>
            <button class="btn btn-primary btn-sm" onclick={() => window.location.reload()}>{m.init_retry()}</button>
          {:else}
            <span class="loading loading-spinner loading-lg text-primary"></span>
            <div class="text-base-content/60 text-sm">{m.init_loading()}</div>
          {/if}
        </div>
      {:else if s.activeTab === 'servers'}
        <ServersTab
          servers={s.servers}
          pingCache={s.pingCache}
          pingPending={s.pingPending}
          pingTimeouts={s.pingTimeouts}
          a2sFailures={s.a2sFailures}
          installedMods={s.installedMods}
          favorites={favoritesSet}
          excludedIps={excludedIpsSet}
          loading={s.serversLoading}
          bind:filter={s.serversFilter}
          bmApiKey={s.profile?.battlemetrics_api_key ?? null}
          userLocation={s.profile?.user_location ?? null}
          onConnect={connectToServer}
          onAddFavorite={addFavorite}
          onRemoveFavorite={(sv) => removeFavoriteQuick(sv.ip, sv.query_port)}
          onRefresh={refreshServers}
          onPing={pingSingle}
          onExcludeIp={excludeIp}
          onUnexcludeIp={unexcludeIp}
          onManageExcluded={() => {
            s.showExcludedIpsModal = true;
          }}
          onDirectConnect={openInDirectConnect}
        />
      {:else if s.activeTab === 'favorites'}
        {#await lazyTabs.favorites() then module}
          {@const FavoritesTab = module.default}
          <FavoritesTab
            favorites={s.profile?.favorites ?? []}
            servers={s.servers}
            pingCache={s.pingCache}
            pingPending={s.pingPending}
            pingTimeouts={s.pingTimeouts}
            a2sFailures={s.a2sFailures}
            bmApiKey={s.profile?.battlemetrics_api_key ?? null}
            userLocation={s.profile?.user_location ?? null}
            onConnect={connectByAddress}
            onRemove={removeFavorite}
            onGoToServers={() => selectTab('servers')}
            onPing={pingSingle}
            onDirectConnect={openInDirectConnect}
          />
        {/await}
      {:else if s.activeTab === 'history'}
        {#await lazyTabs.history() then module}
          {@const HistoryTab = module.default}
          <HistoryTab
            history={s.profile?.history ?? []}
            servers={s.servers}
            pingCache={s.pingCache}
            pingPending={s.pingPending}
            pingTimeouts={s.pingTimeouts}
            a2sFailures={s.a2sFailures}
            favorites={favoritesSet}
            bmApiKey={s.profile?.battlemetrics_api_key ?? null}
            userLocation={s.profile?.user_location ?? null}
            onConnect={connectByAddress}
            onAddFavorite={addFavoriteFromHistory}
            onRemoveFavorite={(h) => removeFavoriteQuick(h.ip, h.port)}
            onRemove={removeHistoryEntry}
            onClearAll={clearHistory}
            onGoToServers={() => selectTab('servers')}
            onPing={pingSingle}
            onDirectConnect={openInDirectConnect}
          />
        {/await}
      {:else if s.activeTab === 'mods'}
        {#await lazyTabs.mods() then module}
          {@const ModsTab = module.default}
          <ModsTab
            bind:this={modsTabRef}
            mods={s.installedMods}
            loading={s.modsLoading}
            checking={s.modsChecking}
            staleCount={staleModCount}
            onRefresh={() =>
              loadMods().then(() => {
                if (s.profile?.steam_api_key) checkModUpdates(true);
              })}
            onCheckUpdates={() => checkModUpdates(true)}
            steamApiKey={s.profile?.steam_api_key ?? ''}
            onDelete={deleteMod}
            onToggleManaged={toggleModManaged}
            onUpdate={updateMod}
            onUpdateAll={updateAllMods}
            onUpdateStale={updateStaleMods}
            onCleanup={cleanupMods}
            onOpenWorkshopDir={() => invoke('open_workshop_dir').catch(() => {})}
            onOpenModDir={(mod) => invoke('open_mod_dir', { modId: mod.id }).catch(() => {})}
            onDeleteSelected={deleteSelectedMods}
            onUpdateSelected={updateSelectedMods}
            onInstallMods={installMods}
          />
        {/await}
      {:else if s.activeTab === 'news'}
        {#await lazyTabs.news() then module}
          {@const NewsTab = module.default}
          <NewsTab
            articles={s.articles}
            loading={s.newsLoading}
            onRefresh={() => {
              s.articles = [];
              loadNews();
            }}
            onOpenUrl={openUrl}
          />
        {/await}
      {:else if s.activeTab === 'connect'}
        {#await lazyTabs.connect() then module}
          {@const DirectConnectTab = module.default}
          <DirectConnectTab
            servers={s.servers}
            installedMods={s.installedMods}
            favorites={favoritesSet}
            favoriteList={s.profile?.favorites ?? []}
            onConnect={connectDirect}
            onAddFavorite={addFavoriteDirect}
            prefill={s.directConnectPrefill}
          />
        {/await}
      {:else if s.activeTab === 'options'}
        {#await lazyTabs.options() then module}
          {@const OptionsTab = module.default}
          <OptionsTab
            options={s.profile?.options ?? []}
            bind:search={s.optionsSearch}
            onToggle={toggleOption}
            onSetValue={setOptionValue}
          />
        {/await}
      {:else if s.activeTab === 'offline'}
        {#await lazyTabs.offline() then module}
          {@const OfflineTab = module.default}
          <OfflineTab
            missions={s.offlineMissions}
            loading={s.offlineLoading}
            status={s.offlineStatus}
            statusKind={s.offlineStatusKind}
            onRefresh={loadOfflineMissions}
            onUpdate={updateOfflineMode}
            onLaunch={launchOfflineMission}
            onRemoveOfflineMode={removeOfflineMode}
            onRemoveMission={removeMission}
            onClearSaves={clearOfflineSaves}
            onOpenMissionDir={(mission) => invoke('open_mission_dir', { mission }).catch(() => {})}
            onOpenMissionsDir={() => invoke('open_missions_dir').catch(() => {})}
          />
        {/await}
      {:else if s.activeTab === 'about'}
        {#await lazyTabs.about() then module}
          {@const AboutTab = module.default}
          <AboutTab
            onExport={exportProfile}
            onImport={importProfile}
            onReset={resetProfile}
            updateState={s.updateState}
            updateInfo={s.updateInfo}
            updateError={s.updateError}
            {dlPercent}
            dlReceived={s.dlReceived}
            dlTotal={s.dlTotal}
            onCheckForUpdate={checkForUpdate}
            onInstallUpdate={installUpdate}
          />
        {/await}
      {/if}
    </div>

    <ConfirmModal dialog={s.confirmDialog} onClose={() => (s.confirmDialog = null)} />
    <ConnectModal
      request={s.connectRequest}
      onClose={() => (s.connectRequest = null)}
      onShowModConfirm={(mods, mode, onConfirm) => {
        s.pendingServerModOp = { mods, mode, onConfirm };
      }}
    />
    {#if s.pendingServerModOp}
      <ModConfirmModal
        mods={s.pendingServerModOp.mods}
        mode={s.pendingServerModOp.mode}
        onConfirm={() => {
          s.pendingServerModOp?.onConfirm();
          s.pendingServerModOp = null;
        }}
        onCancel={() => {
          s.pendingServerModOp = null;
        }}
      />
    {/if}
    <ProgressModal
      modOp={s.modOp}
      onDismiss={dismissModOp}
      onSendPassword={sendSteamcmdPassword}
      steamLogin={s.profile?.steam_login}
      onDontTrust={cancelModOperation}
      onCancel={cancelModOperation}
    />

    {#if s.showExcludedIpsModal}
      <ExcludedIpsModal
        excludedIps={s.profile?.excluded_ips ?? []}
        onUnexclude={unexcludeIp}
        onClose={() => {
          s.showExcludedIpsModal = false;
        }}
      />
    {/if}

    {#if s.showWizard}
      {#await lazyTabs.setup() then module}
        {@const SetupWizard = module.default}
        <SetupWizard
          onDone={async () => {
            s.showWizard = false;
            await Promise.all([loadProfile(), loadStats()]);
            if (s.profile?.steam_api_key && s.profile?.steam_id) {
              invoke<string | null>('fetch_steam_avatar')
                .then((url) => {
                  s.avatarUrl = url;
                })
                .catch(() => {});
            }
          }}
        />
      {/await}
    {/if}
  </div>
{/key}
