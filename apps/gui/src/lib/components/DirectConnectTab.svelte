<script lang="ts">
  import type { A2sDetailsDto, ServerDto, ServerFullDto, InstalledModDto, FavoriteDto, DzchConfig } from '$lib/types';
  import { playerFill, playerBarColor, formatDuration } from '$lib/utils';
  import { createSimpleCopyState } from '$lib/utils/clipboard.svelte';
  import { invoke } from '@tauri-apps/api/core';
  import { openUrl } from '@tauri-apps/plugin-opener';
  import { save as saveDialog } from '@tauri-apps/plugin-dialog';
  import { createServerLookup, findServer as findServerByKey } from '$lib/utils/server-lookup';
  import Icon from '@iconify/svelte';
  import * as m from '$lib/paraglide/messages.js';

  const { copied: copiedIp, copy: copyIpToClipboard } = createSimpleCopyState();
  async function copyIp(ip: string, port: number) {
    await copyIpToClipboard(`${ip}:${port}`);
  }

  /** Prefill payload: when set, the form is populated and a query is triggered. */
  export interface Prefill {
    ip: string;
    port: number;
    /** When set, query uses this port while the port input shows the game port. */
    queryPort?: number;
  }

  interface Props {
    servers: ServerDto[];
    installedMods: InstalledModDto[];
    /** Set of "ip:port" strings for quick favorite lookup */
    favorites?: Set<string>;
    /** Full favorite list — used to auto-fill saved password */
    favoriteList?: FavoriteDto[];
    onConnect: (ip: string, port: number, password?: string, extraArgs?: string[]) => void;
    onAddFavorite?: (name: string, ip: string, port: number, password?: string) => void;
    /** When set, pre-fill the form and auto-query. Set to null after handling. */
    prefill?: Prefill | null;
  }

  let {
    servers,
    installedMods,
    favorites = new Set(),
    favoriteList = [],
    onConnect,
    onAddFavorite,
    prefill = null,
  }: Props = $props();

  let address = $state('');
  let port = $state('2302');
  let password = $state('');
  let showPassword = $state(false);

  // Query state
  let a2s = $state<A2sDetailsDto | null>(null);
  let a2sLoading = $state(false);
  let a2sError = $state('');

  // Resolved port info for display
  type PortKind = 'query' | 'game' | 'unknown';
  let resolvedQueryPort = $state<number | null>(null);
  let resolvedPortKind = $state<PortKind>('unknown');

  // Full server details (from list)
  let fullServer = $state<ServerFullDto | null>(null);
  let fullLoading = $state(false);

  // Installed mod ids for badge
  let installedIds = $derived(new Set(installedMods.map((m) => m.id)));

  // ── Prefill from external navigation (e.g. pressing D in server list) ─────

  /** Track the last prefill we acted on to avoid re-triggering. */
  let lastPrefill: Prefill | null = null;

  $effect(() => {
    if (prefill && prefill !== lastPrefill) {
      lastPrefill = prefill;
      address = prefill.ip;
      port = String(prefill.port);
      password = '';
      // Clear previous query state
      a2s = null;
      a2sError = '';
      resolvedQueryPort = null;
      fullServer = null;
      // When a separate queryPort is provided, query on that port
      // while keeping the game port in the input field.
      const qp = prefill.queryPort ?? undefined;
      // Auto-query after a tick so the DOM updates first
      queueMicrotask(() => queryInfo(qp));
    }
  });

  // ── Modes ──────────────────────────────────────────────────────────────────

  /**
   * A "mode entry" is one launch argument the user wants appended, e.g.
   *   { kind: 'mod',    value: '9876543' }  → -mod=@9876543
   *   { kind: 'custom', value: '-doLogs' }  → -doLogs  (verbatim)
   *
   * Entries detected from the server query are marked `fromServer: true` and
   * sorted to the top (priority).
   */
  interface ModeEntry {
    id: string; // unique key for svelte keying
    kind: 'mod' | 'custom';
    value: string; // mod ID (number string) or verbatim arg
    label: string; // display name (mod name when known, else value)
    fromServer: boolean; // detected from queried server → shown first
    enabled: boolean;
  }

  let modeEntries = $state<ModeEntry[]>([]);
  let newModId = $state<number | null>(null);
  let showModesPanel = $state(false);

  /** Installed mods not yet in the modes list, sorted by name. */
  let availableToAdd = $derived(
    installedMods
      .filter((m) => !modeEntries.some((e) => e.value === String(m.id)))
      .sort((a, b) => a.name.localeCompare(b.name)),
  );

  let idCounter = 0;
  function uid() {
    return `m${++idCounter}`;
  }

  /**
   * Rebuild the modes list after a successful server query.
   *
   * Priority order (top → bottom in the list):
   *   1. Mods from the server list (fullServer / a2s.mods when found in list)
   *   2. Extra mods only seen in A2S response (not in list)
   *   3. User-added entries
   *
   * Previously auto-detected entries that have disappeared are removed;
   * user entries are always preserved.
   */
  function syncServerModes() {
    // List-sourced mods (highest priority — proper names from DayZSA API)
    const listMods =
      fullServer && fullServer.mods.length > 0 ? fullServer.mods : a2s && a2s.mods.length > 0 ? a2s.mods : [];
    const listIds = new Set(listMods.map((m) => String(m.steam_workshop_id)));

    // A2S-only mods (secondary — not in the server list)
    const a2sOnlyMods = (a2s?.mods ?? []).filter((m) => !listIds.has(String(m.steam_workshop_id)));

    const allDetectedIds = new Set([
      ...listMods.map((m) => String(m.steam_workshop_id)),
      ...a2sOnlyMods.map((m) => String(m.steam_workshop_id)),
    ]);

    // Keep only user entries (drop stale auto-detected ones)
    const userEntries = modeEntries.filter((e) => !e.fromServer);

    // Keep previously auto-detected entries that are still present
    const keptDetected = modeEntries.filter((e) => e.fromServer && allDetectedIds.has(e.value));
    const keptIds = new Set(keptDetected.map((e) => e.value));

    // Build new entries for mods not already in keptDetected
    // List mods come first (they have priority), then A2S-only
    const newListEntries: ModeEntry[] = listMods
      .filter((m) => !keptIds.has(String(m.steam_workshop_id)))
      .map((m) => ({
        id: uid(),
        kind: 'mod' as const,
        value: String(m.steam_workshop_id),
        label: m.name || String(m.steam_workshop_id),
        fromServer: true,
        enabled: true,
      }));

    const newA2sEntries: ModeEntry[] = a2sOnlyMods
      .filter((m) => !keptIds.has(String(m.steam_workshop_id)))
      .map((m) => ({
        id: uid(),
        kind: 'mod' as const,
        value: String(m.steam_workshop_id),
        label: m.name || String(m.steam_workshop_id),
        fromServer: true,
        enabled: true,
      }));

    // Final order: kept detected (preserves user reordering) + new list mods
    // + new A2S-only mods + user entries
    // Separate kept entries back into list-sourced vs a2s-only to maintain order
    const keptList = keptDetected.filter((e) => listIds.has(e.value));
    const keptA2sOnly = keptDetected.filter((e) => !listIds.has(e.value));

    modeEntries = [
      ...keptList,
      ...newListEntries, // list-sourced first (priority)
      ...keptA2sOnly,
      ...newA2sEntries, // A2S-only second
      ...userEntries, // user entries last
    ];
  }

  function addModeEntry() {
    if (newModId === null) return;
    const mod = installedMods.find((m) => m.id === newModId);
    if (!mod) return;
    modeEntries = [
      ...modeEntries,
      {
        id: uid(),
        kind: 'mod' as const,
        value: String(mod.id),
        label: mod.name,
        fromServer: false,
        enabled: true,
      },
    ];
    // Reset to first available mod
    newModId = null;
  }

  function removeModeEntry(id: string) {
    modeEntries = modeEntries.filter((e) => e.id !== id);
  }

  function toggleModeEntry(id: string) {
    modeEntries = modeEntries.map((e) => (e.id === id ? { ...e, enabled: !e.enabled } : e));
  }

  function moveModeEntry(id: string, dir: -1 | 1) {
    const idx = modeEntries.findIndex((e) => e.id === id);
    if (idx < 0) return;
    const newIdx = idx + dir;
    if (newIdx < 0 || newIdx >= modeEntries.length) return;
    const next = [...modeEntries];
    [next[idx], next[newIdx]] = [next[newIdx], next[idx]];
    modeEntries = next;
  }

  /** Build the extra_args array from the modes list to pass to launch_direct. */
  function buildModeArgs(): string[] {
    const args: string[] = [];
    for (const e of modeEntries) {
      if (!e.enabled) continue;
      if (e.kind === 'mod') {
        args.push(`-mod=@${e.value}`);
      } else {
        // custom: emit verbatim
        args.push(e.value.startsWith('-') ? e.value : `-${e.value}`);
      }
    }
    return args;
  }

  // ── Helpers ────────────────────────────────────────────────────────────────

  // Pre-built lookup map rebuilt only when `servers` changes (O(n) once),
  // so resolving the entered ip:port is O(1) per keystroke instead of O(n).
  let serverByKey = $derived(createServerLookup(servers));

  /** Server found in the cached list by IP + port. */
  let foundServer = $derived(
    (() => {
      const p = parseInt(port, 10);
      if (!address || isNaN(p)) return null;
      return findServerByKey(serverByKey, address.trim(), p);
    })(),
  );

  /** True if the current ip:port (or any related port) is already a favorite. */
  let isFavorite = $derived(
    (() => {
      const ip = address.trim();
      const p = parseInt(port, 10);
      if (!ip || isNaN(p)) return false;
      if (favorites.has(`${ip}:${p}`)) return true;
      if (!foundServer) return false;
      return favorites.has(`${ip}:${foundServer.game_port}`) || favorites.has(`${ip}:${foundServer.query_port}`);
    })(),
  );

  /** Matching favorite entry (if any) — used to auto-fill the saved password. */
  let matchedFavorite = $derived(
    (() => {
      const ip = address.trim();
      const p = parseInt(port, 10);
      if (!ip || isNaN(p)) return null;
      return (
        favoriteList.find(
          (f) =>
            f.ip === ip &&
            (f.port === p || (foundServer && (f.port === foundServer.game_port || f.port === foundServer.query_port))),
        ) ?? null
      );
    })(),
  );

  // Auto-fill password from the matched favorite whenever the address resolves
  // to a known favorite — but only if the user hasn't typed anything yet.
  $effect(() => {
    if (matchedFavorite?.password && !password) {
      password = matchedFavorite.password;
    }
  });

  function parseAddress() {
    const raw = address.trim();
    const colon = raw.lastIndexOf(':');
    if (colon !== -1) {
      const after = raw.slice(colon + 1);
      if (/^\d+$/.test(after)) {
        address = raw.slice(0, colon);
        port = after;
      }
    }
  }

  // ── Query ──────────────────────────────────────────────────────────────────

  /**
   * Try to query A2S at a specific port. Returns the result or throws.
   */
  async function tryQuery(ip: string, qport: number): Promise<A2sDetailsDto> {
    return invoke<A2sDetailsDto>('query_a2s', { ip, port: qport });
  }

  async function queryInfo(overrideQueryPort: number | undefined = undefined) {
    parseAddress();
    const p = parseInt(port, 10);
    if (!address || isNaN(p)) return;

    const ip = address.trim();
    a2sLoading = true;
    a2sError = '';
    a2s = null;
    fullServer = null;
    resolvedQueryPort = null;
    resolvedPortKind = 'unknown';

    // Snapshot the list-matched server BEFORE any async work.  This avoids
    // issues with $derived re-evaluating after port changes below.
    const fs = foundServer;

    // ── Kick off server-details fetch (parallel with A2S) ──────────────────
    // When the server is in the list, fetch its full mod list immediately
    // using the exact same IP + query_port that the server-list detail panel
    // uses.  This is independent of A2S — if A2S times out the mod list
    // still loads.
    let detailPromise: Promise<ServerFullDto | null> = Promise.resolve(null);
    if (fs && fs.mods_count > 0) {
      fullLoading = true;
      detailPromise = invoke<ServerFullDto>('get_server_details', {
        ip: fs.ip,
        port: fs.query_port,
      }).catch(() => null);
    }

    // ── A2S query ────────────────────────────────────────────────────────────
    // Determine which port to query: prefer foundServer's query_port,
    // then an explicit override (from prefill), then the typed port.
    const qPort = fs?.query_port ?? overrideQueryPort ?? p;
    try {
      if (fs) {
        // Server found in list — use authoritative IP + query port so the
        // backend lookup matches exactly (avoids mismatches from typed IP).
        a2s = await tryQuery(fs.ip, qPort);
        resolvedQueryPort = qPort;
        resolvedPortKind = p === qPort ? 'query' : 'game';
      } else {
        // Not in list — use override or typed port as the query port.
        // If the port is 2302 (default game port) the backend will
        // automatically try 27016 (default A2S query port) first.
        a2s = await tryQuery(ip, qPort);
        resolvedQueryPort = a2s.query_port;
        resolvedPortKind = p === a2s.query_port ? 'query' : 'unknown';
      }

      // ── Resolve the real game port and update the port field ─────────────
      const resolvedGamePort =
        fs?.game_port ?? // from server list (most reliable)
        a2s!.game_port ?? // from A2S extended server info
        null;
      if (resolvedGamePort !== null) {
        port = String(resolvedGamePort);
      }

      // If the server wasn't in the list but A2S succeeded, try fetching
      // details now using the resolved query port.
      if (!fs) {
        fullLoading = true;
        detailPromise = invoke<ServerFullDto>('get_server_details', {
          ip,
          port: a2s!.query_port,
        }).catch(() => null);
      }
    } catch (e) {
      a2sError = String(e);
    } finally {
      a2sLoading = false;
    }

    // ── Await server details (always, even if A2S failed) ────────────────────
    try {
      fullServer = await detailPromise;
    } catch {
      fullServer = null;
    } finally {
      fullLoading = false;
    }

    // Sync modes list with detected server mods (detected entries get priority)
    syncServerModes();
  }

  function connect() {
    parseAddress();
    const p = parseInt(port, 10);
    if (!address || isNaN(p)) return;
    // Always launch with the game port, not the query port.
    // Priority: server list game_port → A2S game_port → typed port as-is.
    const fs = foundServer;
    const gamePort = fs?.game_port ?? a2s?.game_port ?? p;
    const extra = buildModeArgs();
    onConnect(address.trim(), gamePort, password || undefined, extra.length ? extra : undefined);
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') connect();
  }

  // Server name to show: prefer a2s (live) > fullServer > foundServer
  let displayName = $derived(a2s ? a2s.server_name : (foundServer?.name ?? ''));

  /**
   * Merged mod list with priority: list mods first (fullServer or a2s.mods from
   * server list), then any additional mods only found via A2S (not already
   * covered by the list).  List mods are preferred because they carry reliable
   * names and IDs coming from the DayZSA Launcher API.
   */
  let displayMods = $derived(
    (() => {
      // Source 1 — server list mods (highest priority, have proper names + IDs)
      const listMods =
        fullServer && fullServer.mods.length > 0 ? fullServer.mods : a2s && a2s.mods.length > 0 ? a2s.mods : [];

      // Source 2 — raw A2S mods (only present when server is NOT in list, so
      // listMods will be empty in that case — no real dedup needed, but guard
      // anyway in case both sources ever overlap in the future)
      const listIds = new Set(listMods.map((m) => m.steam_workshop_id));
      const a2sOnly = (a2s?.mods ?? []).filter((m) => !listIds.has(m.steam_workshop_id));

      return [...listMods, ...a2sOnly];
    })(),
  );

  let showCard = $derived(!!(a2s || foundServer));

  // Count enabled mode entries for the badge
  let enabledModeCount = $derived(modeEntries.filter((e) => e.enabled).length);

  // ── Export / Share ──────────────────────────────────────────────────────────

  /** Build a DzchConfig from the current form state + queried server info. */
  function buildDzchConfig(): DzchConfig {
    const fs = foundServer;
    const mods = displayMods.map((m) => ({ id: m.steam_workshop_id, name: m.name }));
    // Resolve game port: server list → A2S → typed port
    const gamePort = fs?.game_port ?? a2s?.game_port ?? (parseInt(port, 10) || 2302);
    // Resolve query port: server list → A2S → null
    const qport = fs?.query_port ?? (a2s ? a2s.query_port : null);
    return {
      version: 1,
      ip: address.trim(),
      port: gamePort,
      query_port: qport && qport !== gamePort ? qport : undefined,
      name: displayName || '',
      password: password || undefined,
      mods,
    };
  }

  const { copied: copiedUrl, copy: copyUrlToClipboard } = createSimpleCopyState();

  async function exportDzchFile() {
    const config = buildDzchConfig();
    const defaultName = config.name
      ? `${config.name.replace(/[^a-zA-Z0-9_\- ]/g, '_')}.dzch`
      : `${config.ip}_${config.port}.dzch`;
    const path = await saveDialog({
      title: 'Export server config',
      defaultPath: defaultName,
      filters: [{ name: 'DayZ Community Hub config', extensions: ['dzch'] }],
    });
    if (!path) return;
    await invoke('write_dzch_file', { path, config });
  }

  async function copyDzchUrl() {
    const config = buildDzchConfig();
    // Build URL manually to avoid a round-trip
    let url = `dzch://${config.ip}:${config.port}`;
    const params: string[] = [];
    if (config.query_port) params.push(`qport=${config.query_port}`);
    if (config.name) params.push(`name=${encodeURIComponent(config.name)}`);
    if (config.password) params.push(`password=${encodeURIComponent(config.password)}`);
    if (config.mods.length > 0) params.push(`mods=${config.mods.map((m) => m.id).join(',')}`);
    if (params.length > 0) url += '?' + params.join('&');
    await copyUrlToClipboard(url);
  }
</script>

<div class="h-full overflow-auto">
  <div class="max-w-6xl mx-auto w-full p-6">
    <!-- Header -->
    <div class="flex items-center gap-2 mb-5">
      <Icon icon="ph:plugs-connected" class="size-5 text-primary" />
      <h2 class="text-base font-semibold">{m.dc_title()}</h2>
    </div>

    <!-- ── Two-column grid (desktop) / single column (mobile) ────────── -->
    <div class="grid grid-cols-1 lg:grid-cols-[minmax(0,420px)_1fr] gap-6 items-start">
      <!-- ═══════════════════ LEFT COLUMN: form + controls ═══════════════ -->
      <div class="space-y-4 lg:sticky lg:top-6">
        <!-- Form card -->
        <div class="rounded-xl border border-base-300 bg-base-100 overflow-hidden">
          <div class="px-4 py-2.5 bg-base-200 border-b border-base-300 flex items-center gap-2">
            <Icon icon="ph:network" class="size-4 text-base-content/50" />
            <span class="text-xs font-semibold text-base-content/70 uppercase tracking-wide">{m.dc_connection()}</span>
          </div>

          <div class="p-4 space-y-3">
            <!-- Address + Port -->
            <div class="flex gap-3">
              <div class="form-control flex-1">
                <label class="label py-0 pb-1" for="dc-address">
                  <span class="label-text text-xs text-base-content/60 flex items-center gap-1.5">
                    <Icon icon="ph:globe" class="size-3.5" />
                    {m.dc_ip_hostname()}
                  </span>
                </label>
                <input
                  id="dc-address"
                  type="text"
                  class="input input-bordered input-sm font-mono"
                  placeholder={m.dc_ip_placeholder()}
                  bind:value={address}
                  onblur={parseAddress}
                  onkeydown={handleKeydown}
                />
              </div>
              <div class="form-control w-24">
                <label class="label py-0 pb-1" for="dc-port">
                  <span class="label-text text-xs text-base-content/60 flex items-center gap-1.5">
                    <Icon icon="ph:plugs" class="size-3.5" />
                    {#if foundServer && parseInt(port, 10) === foundServer?.game_port}
                      {m.dc_port()} <span class="text-port-game ml-0.5">{m.dc_port_game()}</span>
                    {:else if foundServer && parseInt(port, 10) === foundServer?.query_port}
                      {m.dc_port()} <span class="text-port-query ml-0.5">{m.dc_port_query()}</span>
                    {:else}
                      {m.dc_port()}
                    {/if}
                  </span>
                </label>
                <input
                  id="dc-port"
                  type="number"
                  class="input input-bordered input-sm font-mono"
                  placeholder="2302"
                  bind:value={port}
                  onkeydown={handleKeydown}
                />
              </div>
            </div>

            <!-- Password -->
            <div class="form-control">
              <label class="label py-0 pb-1" for="dc-password">
                <span class="label-text text-xs text-base-content/60 flex items-center gap-1.5">
                  <Icon icon="ph:lock-simple" class="size-3.5" />
                  {m.dc_password()}
                  <span class="text-base-content/30 ml-1">{m.dc_optional()}</span>
                </span>
              </label>
              <div class="relative">
                <input
                  id="dc-password"
                  type={showPassword ? 'text' : 'password'}
                  class="input input-bordered input-sm w-full pr-9"
                  placeholder={m.dc_password_placeholder()}
                  bind:value={password}
                  onkeydown={handleKeydown}
                />
                <button
                  class="absolute right-2 top-1/2 -translate-y-1/2 text-base-content/40 hover:text-base-content transition-colors"
                  onclick={() => (showPassword = !showPassword)}
                  type="button"
                  aria-label={showPassword ? m.settings_hide() : m.settings_show()}
                  aria-pressed={showPassword}
                  title={showPassword ? m.settings_hide() : m.settings_show()}
                >
                  <Icon icon={showPassword ? 'ph:eye-slash' : 'ph:eye'} class="size-4" />
                </button>
              </div>
            </div>

            <!-- Actions -->
            <div class="flex gap-2 pt-1">
              <button
                class="btn btn-ghost btn-sm gap-1.5"
                onclick={() => queryInfo()}
                disabled={!address || a2sLoading}
              >
                {#if a2sLoading}
                  <span class="loading loading-spinner loading-xs"></span>
                  {m.dc_querying()}
                {:else}
                  <Icon icon="ph:magnifying-glass" class="size-4" />
                  {m.dc_query()}
                {/if}
              </button>
              <button class="btn btn-primary btn-sm flex-1 gap-1.5" onclick={connect} disabled={!address}>
                <Icon icon="ph:rocket-launch" class="size-4" />
                {m.dc_connect()}
              </button>
              <!-- Favorite shortcut -->
              {#if onAddFavorite && address.trim()}
                {#if isFavorite}
                  <span class="btn btn-ghost btn-sm btn-square shrink-0 cursor-default" title={m.dc_already_favorite()}>
                    <Icon icon="ph:star-fill" class="size-4 text-warning" />
                  </span>
                {:else}
                  <button
                    class="btn btn-ghost btn-sm btn-square shrink-0"
                    onclick={() =>
                      onAddFavorite!(
                        displayName || address.trim(),
                        address.trim(),
                        parseInt(port, 10),
                        password || undefined,
                      )}
                    title={m.dc_add_favorite()}
                  >
                    <Icon icon="ph:star" class="size-4 text-warning/60 hover:text-warning transition-colors" />
                  </button>
                {/if}
              {/if}
            </div>
          </div>
        </div>

        <!-- ── Share / Export ─────────────────────────────────────────── -->
        {#if address.trim()}
          <div class="flex gap-2">
            <button class="btn btn-ghost btn-sm gap-1.5 flex-1" onclick={copyDzchUrl} title={m.dc_copy_url_title()}>
              <Icon icon={copiedUrl ? 'ph:check' : 'ph:link'} class="size-4 {copiedUrl ? 'text-success' : ''}" />
              {copiedUrl ? m.dc_copied() : m.dc_copy_url()}
            </button>
            <button class="btn btn-ghost btn-sm gap-1.5 flex-1" onclick={exportDzchFile} title={m.dc_export_title()}>
              <Icon icon="ph:export" class="size-4" />
              {m.dc_export()}
            </button>
          </div>
        {/if}

        <!-- ── Extra Mods panel ──────────────────────────────────────── -->
        <div class="rounded-xl border border-base-300 bg-base-100 overflow-hidden">
          <!-- Collapsible header -->
          <button
            class="w-full px-4 py-2.5 bg-base-200 border-b border-base-300 flex items-center gap-2 hover:bg-base-300/50 transition-colors text-left"
            onclick={() => (showModesPanel = !showModesPanel)}
            type="button"
          >
            <Icon icon="ph:sliders-horizontal" class="size-4 text-base-content/50" />
            <span class="text-xs font-semibold text-base-content/70 uppercase tracking-wide flex-1">
              {m.dc_extra_mods()}
            </span>
            {#if enabledModeCount > 0}
              <span class="badge badge-primary badge-xs">{enabledModeCount}</span>
            {/if}
            <Icon
              icon={showModesPanel ? 'ph:caret-up' : 'ph:caret-down'}
              class="size-3.5 text-base-content/40 shrink-0"
            />
          </button>

          {#if showModesPanel}
            <div class="p-4 space-y-3">
              <!-- Explanation -->
              <p class="text-xs text-base-content/50 leading-relaxed">
                {m.dc_extra_mods_hint()}
              </p>

              <!-- Existing entries -->
              {#if modeEntries.length > 0}
                <div class="space-y-1">
                  {#each modeEntries as entry (entry.id)}
                    {@const isFirst = modeEntries[0].id === entry.id}
                    {@const isLast = modeEntries[modeEntries.length - 1].id === entry.id}
                    <div
                      class="flex items-center gap-2 px-2.5 py-1.5 rounded-lg text-xs group
                        {entry.fromServer
                        ? 'bg-primary/5 border border-primary/15'
                        : 'bg-base-200/60 border border-base-300/50'}
                        {!entry.enabled ? 'opacity-50' : ''}"
                    >
                      <!-- Kind badge -->
                      <span class="shrink-0 font-mono text-base-content/40 w-14 truncate">
                        {#if entry.kind === 'mod'}
                          <span class="text-accent-mods">{m.dc_mod()}</span>
                        {:else}
                          <span class="text-badge-custom">{m.dc_custom()}</span>
                        {/if}
                      </span>

                      <!-- Label / value -->
                      <span class="flex-1 truncate text-base-content/80 font-mono" title={entry.value}>
                        {#if entry.kind !== 'custom'}
                          @{entry.value}
                          {#if entry.label !== entry.value}
                            <span class="text-base-content/40 non-mono"> — {entry.label}</span>
                          {/if}
                        {:else}
                          {entry.value}
                        {/if}
                      </span>

                      <!-- Server-detected badge -->
                      {#if entry.fromServer}
                        <span class="badge badge-xs gap-0.5 bg-primary/10 text-primary border-primary/20 shrink-0">
                          <Icon icon="ph:magnifying-glass" class="size-2.5" />
                          {m.dc_detected()}
                        </span>
                      {/if}

                      <!-- Controls -->
                      <div
                        class="flex items-center gap-0.5 shrink-0 opacity-0 group-hover:opacity-100 transition-opacity"
                      >
                        <button
                          class="btn btn-ghost btn-xs btn-square"
                          title={m.dc_move_up()}
                          disabled={isFirst}
                          onclick={() => moveModeEntry(entry.id, -1)}
                        >
                          <Icon icon="ph:arrow-up" class="size-3" />
                        </button>
                        <button
                          class="btn btn-ghost btn-xs btn-square"
                          title={m.dc_move_down()}
                          disabled={isLast}
                          onclick={() => moveModeEntry(entry.id, 1)}
                        >
                          <Icon icon="ph:arrow-down" class="size-3" />
                        </button>
                        <button
                          class="btn btn-ghost btn-xs btn-square"
                          title={entry.enabled ? m.dc_disable() : m.dc_enable()}
                          onclick={() => toggleModeEntry(entry.id)}
                        >
                          <Icon
                            icon={entry.enabled ? 'ph:toggle-right' : 'ph:toggle-left'}
                            class="size-3.5 {entry.enabled ? 'text-success' : 'text-base-content/30'}"
                          />
                        </button>
                        <button
                          class="btn btn-ghost btn-xs btn-square text-error/60 hover:text-error"
                          title={m.dc_remove()}
                          onclick={() => removeModeEntry(entry.id)}
                        >
                          <Icon icon="ph:x" class="size-3" />
                        </button>
                      </div>
                    </div>
                  {/each}
                </div>
              {:else}
                <p class="text-xs text-base-content/35 italic">
                  {m.dc_no_modes()}
                </p>
              {/if}

              <!-- Add new entry -->
              {#if availableToAdd.length > 0}
                <div class="flex gap-2 pt-1">
                  <select class="select select-bordered select-xs flex-1 min-w-0" bind:value={newModId}>
                    <option value={null} disabled selected>{m.dc_pick_mod()}</option>
                    {#each availableToAdd as mod}
                      <option value={mod.id}>{mod.name}</option>
                    {/each}
                  </select>
                  <button
                    class="btn btn-ghost btn-xs btn-square shrink-0"
                    title={m.dc_add_mod()}
                    disabled={newModId === null}
                    onclick={addModeEntry}
                  >
                    <Icon icon="ph:plus" class="size-3.5" />
                  </button>
                </div>
              {:else if installedMods.length === 0}
                <p class="text-xs text-base-content/35 italic">
                  {m.dc_no_installed_mods()}
                </p>
              {:else}
                <p class="text-xs text-base-content/35 italic">{m.dc_all_mods_added()}</p>
              {/if}

              <!-- Preview of generated args -->
              {#if enabledModeCount > 0}
                {@const preview = buildModeArgs()}
                <div class="rounded-lg bg-base-200 p-2.5 space-y-0.5">
                  <p class="text-xs text-base-content/40 mb-1 uppercase tracking-wide font-semibold">
                    {m.dc_launch_preview()}
                  </p>
                  {#each preview as arg}
                    <p class="font-mono text-xs text-base-content/70 break-all">{arg}</p>
                  {/each}
                </div>
              {/if}
            </div>
          {/if}
        </div>

        <!-- Error (below the form so it's close to the action that caused it) -->
        {#if a2sError}
          <div
            class="flex items-start gap-2.5 px-3.5 py-3 rounded-xl bg-error/10 border border-error/25 text-sm text-error"
          >
            <Icon icon="ph:warning-circle" class="size-4 shrink-0 mt-0.5" />
            <div>
              <p class="font-medium text-sm">{m.dc_query_failed()}</p>
              <p class="text-xs mt-0.5 opacity-80 break-all">{a2sError}</p>
            </div>
          </div>
        {/if}
      </div>
      <!-- end left column -->

      <!-- ═══════════════════ RIGHT COLUMN: server info ═════════════════ -->
      <div class="min-w-0">
        {#if a2sLoading}
          <!-- Loading skeleton -->
          <div class="rounded-xl border border-base-300 bg-base-100 overflow-hidden">
            <div class="px-4 py-3 bg-base-200 border-b border-base-300 flex items-center gap-3">
              <span class="loading loading-spinner loading-sm text-primary"></span>
              <span class="text-sm text-base-content/50">{m.dc_querying_server()}</span>
            </div>
            <div class="p-4 space-y-3">
              <div class="h-3 w-3/4 rounded bg-base-300 animate-pulse"></div>
              <div class="h-3 w-1/2 rounded bg-base-300 animate-pulse"></div>
              <div class="h-2 w-full rounded bg-base-300 animate-pulse"></div>
              <div class="h-3 w-2/3 rounded bg-base-300 animate-pulse"></div>
            </div>
          </div>
        {:else if showCard}
          {@const fs = foundServer}
          {@const players = a2s?.players ?? fs?.players ?? 0}
          {@const maxPlayers = a2s?.max_players ?? fs?.max_players ?? 0}
          {@const map = a2s?.map ?? fs?.map ?? ''}
          {@const version = a2s?.version ?? fs?.version ?? ''}
          {@const mods = displayMods}
          {@const name = displayName}

          <div class="rounded-xl border border-base-300 bg-base-100 overflow-hidden">
            <!-- Card header: name + source badge -->
            <div class="px-4 py-3 bg-base-200 border-b border-base-300 flex items-start gap-3">
              <Icon icon="ph:server" class="size-4 text-primary shrink-0 mt-0.5" />
              <div class="flex-1 min-w-0">
                <p class="font-semibold text-sm text-base-content leading-snug">{name}</p>
                <div class="flex items-center gap-2 mt-1 flex-wrap">
                  {#if fs}
                    <span class="badge badge-success badge-xs gap-1">
                      <Icon icon="ph:check-circle" class="size-2.5" />
                      {m.dc_in_server_list()}
                    </span>
                  {:else if a2s}
                    <span class="badge badge-warning badge-xs gap-1">
                      <Icon icon="ph:question" class="size-2.5" />
                      {m.dc_not_in_list()}
                    </span>
                  {/if}
                  {#if resolvedQueryPort !== null}
                    {#if resolvedPortKind === 'query'}
                      <span class="badge badge-xs badge-country gap-1" title={m.dc_badge_query_port()}>
                        <Icon icon="ph:plugs" class="size-2.5" />
                        {m.dc_query_port_badge()}
                      </span>
                    {:else if resolvedPortKind === 'game'}
                      <span class="badge badge-xs badge-rank gap-1" title={m.dc_badge_game_port()}>
                        <Icon icon="ph:game-controller" class="size-2.5" />
                        {m.dc_game_port_badge()} → Q:{resolvedQueryPort}
                      </span>
                    {/if}
                  {/if}
                  {#if fs?.password}
                    <span class="badge badge-error badge-xs gap-1">
                      <Icon icon="ph:lock-simple" class="size-2.5" />
                      {m.dc_password()}
                    </span>
                  {/if}
                  {#if fs?.battl_eye}
                    <span class="badge badge-xs badge-official gap-1">
                      <Icon icon="ph:shield-check" class="size-2.5" />
                      {m.dc_battleye()}
                    </span>
                  {/if}
                  {#if fs?.first_person_only}
                    <span class="badge badge-xs badge-players gap-1">
                      <Icon icon="ph:eye" class="size-2.5" />
                      {m.dc_1pp()}
                    </span>
                  {/if}
                  {#if fs?.vac}
                    <span class="badge badge-xs gap-1 bg-base-content/10 text-base-content/50 border-base-content/20">
                      {m.dc_vac()}
                    </span>
                  {/if}
                </div>
              </div>
              {#if isFavorite}
                <span class="btn btn-ghost btn-xs btn-square shrink-0 cursor-default" title={m.dc_already_favorite()}>
                  <Icon icon="ph:star-fill" class="size-4 text-warning" />
                </span>
              {/if}
            </div>

            <div class="p-4 space-y-4">
              <!-- Player bar + info grid side by side on wide screens -->
              <div class="grid grid-cols-1 xl:grid-cols-2 gap-4">
                <!-- Player bar -->
                <div>
                  <div class="flex items-center justify-between mb-1.5">
                    <span class="text-xs text-base-content/50 flex items-center gap-1.5">
                      <Icon icon="ph:users" class="size-3.5" />
                      {m.dc_players()}
                    </span>
                    <span class="text-sm font-bold {playerFill(players, maxPlayers)} tabular-nums">
                      {players}<span class="text-base-content/30 font-normal">/{maxPlayers}</span>
                    </span>
                  </div>
                  <div class="w-full h-2 rounded-full bg-base-300 overflow-hidden">
                    <div
                      class="h-full rounded-full transition-all {playerBarColor(players, maxPlayers, false)}"
                      style="width: {maxPlayers > 0 ? Math.min(100, (players / maxPlayers) * 100) : 0}%"
                    ></div>
                  </div>
                </div>

                <!-- Info grid -->
                <div class="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1.5 text-xs">
                  <div class="text-base-content/50 flex items-center gap-1.5">
                    <Icon icon="ph:map-trifold" class="size-3.5" />
                    {m.dc_map()}
                  </div>
                  <span class="text-accent-map font-medium">{map || '—'}</span>

                  {#if fs}
                    <div class="text-base-content/50 flex items-center gap-1.5">
                      <Icon icon="ph:globe-hemisphere-west" class="size-3.5" />
                      {m.dc_ip()}
                    </div>
                    <button
                      class="font-mono text-base-content hover:text-primary transition-colors flex items-center gap-1 group/ip text-left"
                      title={m.dc_copy_ip()}
                      onclick={() => copyIp(fs.ip, fs.game_port)}
                    >
                      {#if copiedIp}
                        <span class="text-success">{m.dc_copied()}</span>
                      {:else}
                        {fs.ip}:{fs.game_port}
                        <Icon icon="ph:copy" class="size-3 opacity-0 group-hover/ip:opacity-100 transition-opacity" />
                      {/if}
                    </button>

                    <div class="text-base-content/50 flex items-center gap-1.5">
                      <Icon icon="ph:game-controller" class="size-3.5" />
                      {m.dc_game_port()}
                    </div>
                    <span class="font-mono text-base-content">{fs.game_port}</span>

                    <div class="text-base-content/50 flex items-center gap-1.5">
                      <Icon icon="ph:plugs" class="size-3.5" />
                      {m.dc_query_port()}
                    </div>
                    <span class="font-mono text-base-content">{fs.query_port}</span>

                    <div class="text-base-content/50 flex items-center gap-1.5">
                      <Icon icon="ph:clock" class="size-3.5" />
                      {m.dc_server_time()}
                    </div>
                    <span class="text-base-content">{fs.time || '—'}</span>

                    <div class="text-base-content/50 flex items-center gap-1.5">
                      <Icon icon="ph:monitor" class="size-3.5" />
                      {m.dc_platform()}
                    </div>
                    <span class={fs.environment === 'w' ? 'text-info' : 'text-success'}>
                      {fs.environment === 'w' ? m.dc_platform_windows() : m.dc_platform_linux()}
                    </span>
                  {:else if a2s}
                    {#if a2s.game_port}
                      <div class="text-base-content/50 flex items-center gap-1.5">
                        <Icon icon="ph:game-controller" class="size-3.5" />
                        {m.dc_game_port()}
                      </div>
                      <span class="font-mono text-base-content">{a2s.game_port}</span>
                    {/if}

                    <div class="text-base-content/50 flex items-center gap-1.5">
                      <Icon icon="ph:plugs" class="size-3.5" />
                      {m.dc_query_port()}
                    </div>
                    <span class="font-mono text-base-content">{a2s.query_port}</span>
                  {/if}

                  {#if version}
                    <div class="text-base-content/50 flex items-center gap-1.5">
                      <Icon icon="ph:tag" class="size-3.5" />
                      {m.dc_version()}
                    </div>
                    <span class="text-base-content/70">{version}</span>
                  {/if}
                </div>
              </div>

              <!-- Mods + Players: side by side on xl -->
              <div class="grid grid-cols-1 xl:grid-cols-2 gap-4">
                <!-- Mods -->
                <div>
                  {#if fullLoading}
                    <div class="flex items-center gap-2 text-xs text-base-content/50">
                      <span class="loading loading-spinner loading-xs"></span>
                      {m.dc_loading_mods()}
                    </div>
                  {:else if mods.length > 0}
                    <div>
                      <div class="flex items-center gap-1.5 mb-2">
                        <Icon icon="ph:puzzle-piece" class="size-3.5 text-base-content/50" />
                        <span class="text-xs font-semibold text-base-content/60 uppercase tracking-wide">
                          {m.dc_mods_count({ count: mods.length })}
                        </span>
                      </div>
                      <div class="space-y-1 max-h-64 overflow-y-auto pr-1">
                        {#each mods as mod}
                          {@const installed = installedIds.has(mod.steam_workshop_id)}
                          <div class="flex items-center gap-2 text-xs group">
                            <span
                              class="flex-shrink-0 w-4 text-center font-bold {installed
                                ? 'text-success'
                                : 'text-error/70'}"
                            >
                              {installed ? '✓' : '−'}
                            </span>
                            <span class="truncate flex-1 text-base-content/80">{mod.name}</span>
                            <button
                              class="font-mono text-base-content/30 hover:text-primary transition-colors flex-shrink-0 opacity-0 group-hover:opacity-100"
                              onclick={(e) => {
                                e.stopPropagation();
                                openUrl(
                                  `https://steamcommunity.com/sharedfiles/filedetails/?id=${mod.steam_workshop_id}`,
                                );
                              }}
                              title={m.dc_open_workshop()}
                            >
                              {mod.steam_workshop_id}
                            </button>
                          </div>
                        {/each}
                      </div>
                    </div>
                  {:else if (fs?.mods_count ?? 0) > 0}
                    <div class="text-xs text-base-content/40 italic flex items-center gap-1.5">
                      <Icon icon="ph:puzzle-piece" class="size-3.5" />
                      {m.dc_mods_no_ids({ count: fs!.mods_count })}
                    </div>
                  {:else}
                    <div class="text-xs text-base-content/40 italic flex items-center gap-1.5">
                      <Icon icon="ph:puzzle-piece" class="size-3.5" />
                      {m.dc_no_mods()}
                    </div>
                  {/if}
                </div>

                <!-- Live players from A2S -->
                <div>
                  {#if a2s}
                    <div>
                      <div class="flex items-center gap-1.5 mb-2">
                        <Icon icon="ph:users-three" class="size-3.5 text-base-content/50" />
                        <span class="text-xs font-semibold text-base-content/60 uppercase tracking-wide">
                          {m.dc_online_players()}
                        </span>
                        <span class="ml-auto text-xs text-base-content/40">
                          {a2s.players_list.length > 0
                            ? m.dc_players_shown({ count: a2s.players_list.length })
                            : m.dc_live_count_only()}
                        </span>
                      </div>
                      {#if a2s.players_list.length > 0}
                        <div class="space-y-1 max-h-64 overflow-y-auto pr-1">
                          {#each a2s.players_list as pl}
                            <div class="flex items-center gap-2 text-xs">
                              <Icon icon="ph:user" class="size-3 text-base-content/30 flex-shrink-0" />
                              <span class="truncate flex-1 text-base-content/80">{pl.name || '—'}</span>
                              <span class="text-base-content/40 tabular-nums flex-shrink-0">
                                {formatDuration(pl.duration, false)}
                              </span>
                            </div>
                          {/each}
                        </div>
                      {:else if a2s.players === 0}
                        <p class="text-xs text-base-content/40 italic">{m.dc_no_players()}</p>
                      {:else}
                        <p class="text-xs text-base-content/40 italic">
                          {a2s.players === 1
                            ? m.dc_players_no_names_one({ count: a2s.players })
                            : m.dc_players_no_names({ count: a2s.players })}
                        </p>
                      {/if}
                    </div>
                  {/if}
                </div>
              </div>

              <!-- Connect action at bottom of card -->
              <div class="flex gap-2 pt-1 border-t border-base-200">
                <button class="btn btn-ghost btn-sm gap-1.5" onclick={() => queryInfo()} disabled={a2sLoading}>
                  {#if a2sLoading}
                    <span class="loading loading-spinner loading-xs"></span>
                  {:else}
                    <Icon icon="ph:arrows-clockwise" class="size-4" />
                  {/if}
                  {m.dc_refresh()}
                </button>
                <button class="btn btn-primary btn-sm flex-1 gap-1.5" onclick={connect}>
                  <Icon icon="ph:rocket-launch" class="size-4" />
                  {m.dc_connect()}
                </button>
              </div>
            </div>
          </div>
        {:else}
          <!-- Empty state: no server queried yet -->
          <div
            class="rounded-xl border border-dashed border-base-300/60 bg-base-200/30 flex flex-col items-center justify-center py-16 px-8 text-center"
          >
            <Icon icon="ph:plugs-connected" class="size-10 text-base-content/15 mb-3" />
            <p class="text-sm text-base-content/40 font-medium">{m.dc_no_server()}</p>
            <p class="text-xs text-base-content/30 mt-1 max-w-xs">
              {m.dc_no_server_hint()}
            </p>
          </div>
        {/if}
      </div>
      <!-- end right column -->
    </div>
    <!-- end grid -->
  </div>
</div>
