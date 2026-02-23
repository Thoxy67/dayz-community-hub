<script lang="ts">
  import type { A2sDetailsDto, ServerDto } from '$lib/types';
  import { invoke } from '@tauri-apps/api/core';
  import Icon from '@iconify/svelte';

  interface Props {
    servers: ServerDto[];
    onConnect: (ip: string, port: number, password?: string) => void;
  }

  let { servers, onConnect }: Props = $props();

  let address = $state('');
  let port = $state('2302');
  let password = $state('');
  let a2s = $state<A2sDetailsDto | null>(null);
  let a2sLoading = $state(false);
  let a2sError = $state('');

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

  let foundServer = $derived(() => {
    const p = parseInt(port, 10);
    if (!address || isNaN(p)) return null;
    return (
      servers.find(
        (s) =>
          s.ip === address.trim() &&
          (s.query_port === p || s.game_port === p)
      ) ?? null
    );
  });

  async function queryInfo() {
    parseAddress();
    const p = parseInt(port, 10);
    if (!address || isNaN(p)) return;
    a2sLoading = true;
    a2sError = '';
    a2s = null;
    try {
      a2s = await invoke<A2sDetailsDto>('query_a2s', { ip: address.trim(), port: p });
    } catch (e) {
      a2sError = String(e);
    } finally {
      a2sLoading = false;
    }
  }

  function connect() {
    parseAddress();
    const p = parseInt(port, 10);
    if (!address || isNaN(p)) return;
    onConnect(address.trim(), p, password || undefined);
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') connect();
  }
</script>

<div class="flex flex-col h-full overflow-auto">
  <div class="max-w-md mx-auto w-full p-8 space-y-5">

    <!-- Header -->
    <div class="flex items-center gap-2 mb-2">
      <Icon icon="mdi:lan-connect" class="size-5 text-primary" />
      <h2 class="text-base font-semibold text-base-content">Direct Connect</h2>
    </div>

    <!-- Address + Port on one row -->
    <div class="flex gap-3">
      <div class="form-control flex-1">
        <label class="label py-1 pb-1.5" for="dc-address">
          <span class="label-text text-xs flex items-center gap-1.5">
            <Icon icon="mdi:ip-network" class="size-3.5 text-base-content/40" />
            IP Address
          </span>
        </label>
        <input
          id="dc-address"
          type="text"
          class="input input-bordered input-sm font-mono"
          placeholder="e.g. 192.168.1.1"
          bind:value={address}
          onblur={parseAddress}
          onkeydown={handleKeydown}
        />
      </div>

      <div class="form-control w-28">
        <label class="label py-1 pb-1.5" for="dc-port">
          <span class="label-text text-xs flex items-center gap-1.5">
            <Icon icon="mdi:ethernet-cable" class="size-3.5 text-base-content/40" />
            Port
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
      <label class="label py-1 pb-1.5" for="dc-password">
        <span class="label-text text-xs flex items-center gap-1.5">
          <Icon icon="mdi:lock-outline" class="size-3.5 text-base-content/40" />
          Password
          <span class="text-base-content/30 ml-1">— optional</span>
        </span>
      </label>
      <input
        id="dc-password"
        type="password"
        class="input input-bordered input-sm"
        placeholder="Leave blank if none"
        bind:value={password}
        onkeydown={handleKeydown}
      />
    </div>

    <!-- Actions -->
    <div class="flex gap-2 pt-1">
      <button
        class="btn btn-ghost btn-sm gap-1.5"
        onclick={queryInfo}
        disabled={!address || a2sLoading}
      >
        {#if a2sLoading}
          <span class="loading loading-spinner loading-xs"></span>
        {:else}
          <Icon icon="mdi:magnify" class="size-4" />
        {/if}
        Query
      </button>
      <button
        class="btn btn-primary btn-sm flex-1 gap-1.5"
        onclick={connect}
        disabled={!address}
      >
        <Icon icon="mdi:lan-connect" class="size-4" />
        Connect
      </button>
    </div>

    <!-- Found in server list -->
    {#if foundServer()}
      <div class="flex items-center gap-2 px-3 py-2 rounded-lg bg-success/10 border border-success/20 text-sm">
        <Icon icon="mdi:check-circle-outline" class="size-4 text-success shrink-0" />
        <div class="min-w-0">
          <span class="text-base-content/70">Found in list: </span>
          <span class="font-medium text-base-content truncate">{foundServer()!.name}</span>
          <span class="text-base-content/50 ml-2">{foundServer()!.players}/{foundServer()!.max_players} players</span>
        </div>
      </div>
    {/if}

    <!-- A2S error -->
    {#if a2sError}
      <div class="flex items-start gap-2 px-3 py-2 rounded-lg bg-error/10 border border-error/20 text-sm text-error">
        <Icon icon="mdi:alert-circle-outline" class="size-4 shrink-0 mt-0.5" />
        <span>{a2sError}</span>
      </div>
    {/if}

    <!-- A2S result card -->
    {#if a2s}
      <div class="rounded-lg border border-base-300 bg-base-200 overflow-hidden">
        <!-- Card header -->
        <div class="px-4 py-3 border-b border-base-300 flex items-center gap-2">
          <Icon icon="mdi:server" class="size-4 text-primary shrink-0" />
          <span class="font-semibold text-sm text-base-content truncate">{a2s.server_name}</span>
        </div>
        <!-- Stats grid -->
        <div class="px-4 py-3 grid grid-cols-2 gap-x-6 gap-y-2 text-xs">
          <div class="flex items-center gap-2 text-base-content/50">
            <Icon icon="mdi:gamepad-variant-outline" class="size-3.5 shrink-0" />
            <span>Game</span>
          </div>
          <span class="text-base-content">{a2s.game}</span>

          <div class="flex items-center gap-2 text-base-content/50">
            <Icon icon="mdi:controller" class="size-3.5 shrink-0" />
            <span>Players</span>
          </div>
          <span class="font-medium text-base-content">{a2s.players} / {a2s.max_players}</span>

          <div class="flex items-center gap-2 text-base-content/50">
            <Icon icon="mdi:map-outline" class="size-3.5 shrink-0" />
            <span>Map</span>
          </div>
          <span class="text-base-content">{a2s.map}</span>

          <div class="flex items-center gap-2 text-base-content/50">
            <Icon icon="mdi:tag-outline" class="size-3.5 shrink-0" />
            <span>Version</span>
          </div>
          <span class="text-base-content">{a2s.version}</span>
        </div>

        <!-- Online players list -->
        {#if a2s.players_list.length > 0}
          <div class="border-t border-base-300">
            <div class="px-4 py-2 flex items-center gap-1.5 text-xs text-base-content/40">
              <Icon icon="mdi:account-multiple-outline" class="size-3.5" />
              <span>Online players ({a2s.players_list.length})</span>
            </div>
            <div class="px-4 pb-3 space-y-1 max-h-36 overflow-y-auto">
              {#each a2s.players_list as pl}
                <div class="flex justify-between items-center text-xs">
                  <div class="flex items-center gap-1.5 text-base-content/80">
                    <Icon icon="mdi:account-outline" class="size-3 text-base-content/30" />
                    <span>{pl.name || '—'}</span>
                  </div>
                  <span class="text-base-content/30 tabular-nums">{pl.score} pts</span>
                </div>
              {/each}
            </div>
          </div>
        {/if}
      </div>
    {/if}

  </div>
</div>
