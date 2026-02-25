<script lang="ts">
  import Icon from '@iconify/svelte';

  export interface ConnectRequest {
    serverName: string;
    /** 'missing' = mods need to be downloaded first, 'update' = mods already installed */
    kind: 'missing' | 'update';
    modCount: number;
    /** Called when the user clicks Connect — receives whether to update/install mods first */
    onConnect: (updateMods: boolean) => void;
  }

  interface Props {
    request: ConnectRequest | null;
    onClose: () => void;
  }

  let { request, onClose }: Props = $props();

  // Default: on for missing mods (required), off for update checks (optional)
  let updateMods = $state(false);

  // Reset toggle whenever a new request comes in
  $effect(() => {
    if (request) {
      updateMods = request.kind === 'missing';
    }
  });

  function handleConnect() {
    request?.onConnect(updateMods);
    onClose();
  }

  function handleKeydown(e: KeyboardEvent) {
    if (e.key === 'Enter') handleConnect();
    if (e.key === 'Escape') onClose();
  }
</script>

{#if request}
  <!-- top: 36px — keeps the titlebar draggable while the modal is open -->
  <div class="modal modal-open" style="top: 36px;">
    <div
      class="modal-box max-w-sm"
      role="dialog"
      aria-modal="true"
      tabindex="-1"
      onkeydown={handleKeydown}
    >
      <!-- Header -->
      <div class="flex items-start gap-3 mb-4">
        <div class="size-9 rounded-lg bg-primary/10 border border-primary/20 flex items-center justify-center shrink-0 mt-0.5">
          <Icon icon="ph:play-fill" class="size-4 text-primary" />
        </div>
        <div class="min-w-0">
          <h3 class="font-bold text-base text-base-content leading-tight">Connect to server</h3>
          <p class="text-xs text-base-content/50 truncate mt-0.5">{request.serverName}</p>
        </div>
      </div>

      <!-- Toggle row -->
      <div class="flex items-center justify-between gap-4 px-3 py-3 rounded-lg bg-base-200/70 border border-base-300/60">
        <div class="flex items-center gap-2.5 min-w-0">
          <Icon
            icon={request.kind === 'missing' ? 'ph:download-simple' : 'ph:arrows-clockwise'}
            class="size-4 shrink-0 {updateMods ? 'text-primary' : 'text-base-content/30'}"
          />
          <div class="min-w-0">
            <p class="text-sm font-medium text-base-content leading-tight">
              {request.kind === 'missing' ? 'Install missing mods' : 'Update mods first'}
            </p>
            <p class="text-xs text-base-content/45 mt-0.5">
              {request.kind === 'missing'
                ? `${request.modCount} mod${request.modCount !== 1 ? 's' : ''} not installed`
                : `${request.modCount} mod${request.modCount !== 1 ? 's' : ''} to check`}
            </p>
          </div>
        </div>
        <input
          type="checkbox"
          class="toggle toggle-sm toggle-primary shrink-0"
          bind:checked={updateMods}
        />
      </div>

      {#if !updateMods && request.kind === 'missing'}
        <p class="text-xs text-warning/80 mt-2 flex items-center gap-1.5 px-1">
          <Icon icon="ph:warning" class="size-3.5 shrink-0" />
          Missing mods may cause connection issues or a crash.
        </p>
      {/if}

      <!-- Actions -->
      <div class="modal-action mt-4">
        <button class="btn btn-ghost btn-sm" onclick={onClose}>Cancel</button>
        <button class="btn btn-primary btn-sm gap-1.5" onclick={handleConnect}>
          <Icon icon="ph:play" class="size-3.5" />
          {updateMods
            ? (request.kind === 'missing' ? 'Install & connect' : 'Update & connect')
            : 'Connect'}
        </button>
      </div>
    </div>
    <button class="modal-backdrop bg-base-content/20" onclick={onClose} aria-label="Close"></button>
  </div>
{/if}

<style></style>
