<script lang="ts">
  import Icon from '@iconify/svelte';
  import * as m from '$lib/paraglide/messages.js';

  /** Mod info for confirmation modal */
  export interface ConnectModInfo {
    id: number;
    name: string;
    local_updated?: number;
    remote_updated?: number | null;
    size_human?: string;
  }

  export interface ConnectRequest {
    serverName: string;
    /** 'missing' = mods need to be downloaded first, 'update' = mods already installed */
    kind: 'missing' | 'update';
    modCount: number;
    /** Detailed mod info for confirmation modal */
    mods: ConnectModInfo[];
    /** Called when the user clicks Connect — receives whether to update/install mods first */
    onConnect: (updateMods: boolean) => void;
  }

  interface Props {
    request: ConnectRequest | null;
    onClose: () => void;
    /** Called when user wants to show mod confirmation before connecting */
    onShowModConfirm?: (mods: ConnectModInfo[], mode: 'update' | 'install', onConfirm: () => void) => void;
  }

  let { request, onClose, onShowModConfirm }: Props = $props();

  // Default: on for missing mods (required), off for update checks (optional)
  let updateMods = $state(false);

  // Reset toggle whenever a new request comes in
  $effect(() => {
    if (request) {
      updateMods = request.kind === 'missing';
    }
  });

  function handleConnect() {
    if (!request) return;

    if (updateMods && request.mods.length > 0 && onShowModConfirm) {
      // Show mod confirmation first
      onShowModConfirm(request.mods, request.kind === 'missing' ? 'install' : 'update', () => request.onConnect(true));
      onClose();
    } else {
      request.onConnect(updateMods);
      onClose();
    }
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') handleConnect();
    if (e.key === 'Escape') onClose();
  }
</script>

{#if request}
  <!-- top: 36px — keeps the titlebar draggable while the modal is open -->
  <div
    class="fixed inset-0 modal-backdrop-window z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm"
    style="top: 36px;"
    role="presentation"
    onclick={onClose}
  >
    <div
      class="bg-base-100 rounded-xl shadow-2xl w-full max-w-sm mx-4 p-6"
      role="dialog"
      aria-modal="true"
      tabindex="-1"
      onkeydown={handleKeydown}
      onclick={(e) => e.stopPropagation()}
    >
      <!-- Header -->
      <div class="flex items-start gap-3 mb-4">
        <div
          class="size-9 rounded-lg bg-primary/10 border border-primary/20 flex items-center justify-center shrink-0 mt-0.5"
        >
          <Icon icon="ph:play-fill" class="size-4 text-primary" />
        </div>
        <div class="min-w-0">
          <h3 class="font-bold text-base text-base-content leading-tight">{m.connect_modal_title()}</h3>
          <p class="text-xs text-base-content/50 truncate mt-0.5">{request.serverName}</p>
        </div>
      </div>

      <!-- Toggle row -->
      <div
        class="flex items-center justify-between gap-4 px-3 py-3 rounded-lg bg-base-200/70 border border-base-300/60"
      >
        <div class="flex items-center gap-2.5 min-w-0">
          <Icon
            icon={request.kind === 'missing' ? 'ph:download-simple' : 'ph:arrows-clockwise'}
            class="size-4 shrink-0 {updateMods ? 'text-primary' : 'text-base-content/30'}"
          />
          <div class="min-w-0">
            <p class="text-sm font-medium text-base-content leading-tight">
              {request.kind === 'missing' ? m.connect_modal_install_mods() : m.connect_modal_update_mods()}
            </p>
            <p class="text-xs text-base-content/45 mt-0.5">
              {request.kind === 'missing'
                ? request.modCount === 1
                  ? m.connect_modal_mods_not_installed({ count: request.modCount })
                  : m.connect_modal_mods_not_installed_plural({ count: request.modCount })
                : request.modCount === 1
                  ? m.connect_modal_mods_to_check({ count: request.modCount })
                  : m.connect_modal_mods_to_check_plural({ count: request.modCount })}
            </p>
          </div>
        </div>
        <input type="checkbox" class="toggle toggle-sm toggle-primary shrink-0" bind:checked={updateMods} />
      </div>

      {#if !updateMods && request.kind === 'missing'}
        <p class="text-xs text-warning/80 mt-2 flex items-center gap-1.5 px-1">
          <Icon icon="ph:warning" class="size-3.5 shrink-0" />
          {m.connect_modal_warning()}
        </p>
      {/if}

      <!-- Actions -->
      <div class="flex justify-end gap-2 mt-4">
        <button class="btn btn-ghost btn-sm" onclick={onClose}>{m.connect_modal_cancel()}</button>
        <button class="btn btn-primary btn-sm gap-1.5" onclick={handleConnect}>
          <Icon icon="ph:play" class="size-3.5" />
          {updateMods
            ? request.kind === 'missing'
              ? m.connect_modal_install_connect()
              : m.connect_modal_update_connect()
            : m.connect_modal_connect()}
        </button>
      </div>
    </div>
  </div>
{/if}
