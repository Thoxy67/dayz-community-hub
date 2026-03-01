<script lang="ts">
  import type { ConfirmDialog } from '$lib/types';
  import * as m from '$lib/paraglide/messages.js';

  interface Props {
    dialog: ConfirmDialog | null;
    onClose: () => void;
  }

  let { dialog, onClose }: Props = $props();

  function confirm() {
    dialog?.onConfirm();
    onClose();
  }

  function decline() {
    if (dialog?.onDecline) {
      dialog.onDecline();
    }
    onClose();
  }

  function cancel() {
    if (dialog?.onCancel) {
      dialog.onCancel();
    }
    onClose();
  }
</script>

{#if dialog}
  <!-- top: 36px — keeps the titlebar draggable while the modal is open -->
  <div
    class="fixed inset-0 z-50 flex items-center justify-center bg-black/60 backdrop-blur-sm"
    style="top: 36px;"
    role="presentation"
    onclick={cancel}
  >
    <!-- svelte-ignore a11y_click_events_have_key_events -->
    <div
      class="bg-base-100 rounded-xl shadow-2xl w-full max-w-md mx-4 p-6"
      role="dialog"
      aria-modal="true"
      tabindex="-1"
      onclick={(e) => e.stopPropagation()}
    >
      <h3 class="font-bold text-lg text-warning">{dialog.title}</h3>
      <div class="py-4 whitespace-pre-wrap text-base-content/80 text-sm leading-relaxed">
        {dialog.message}
      </div>
      <div class="flex justify-end gap-2">
        {#if dialog.onCancel}
          <button class="btn btn-outline btn-error btn-sm" onclick={cancel}>{m.confirm_cancel()}</button>
        {/if}
        <button
          class="btn btn-outline btn-sm"
          class:btn-ghost={!dialog.declineVariant || dialog.declineVariant === 'ghost'}
          class:btn-warning={dialog.declineVariant === 'warning'}
          class:btn-success={dialog.declineVariant === 'success'}
          class:btn-error={dialog.declineVariant === 'error'}
          class:btn-info={dialog.declineVariant === 'info'}
          onclick={decline}
        >
          {dialog.onDecline ? (dialog.declineLabel ?? m.confirm_decline_default()) : m.confirm_cancel()}
        </button>
        <button
          class="btn btn-outline btn-sm"
          class:btn-warning={!dialog.confirmVariant || dialog.confirmVariant === 'warning'}
          class:btn-success={dialog.confirmVariant === 'success'}
          class:btn-error={dialog.confirmVariant === 'error'}
          class:btn-info={dialog.confirmVariant === 'info'}
          class:btn-primary={dialog.confirmVariant === 'primary'}
          onclick={confirm}
        >
          {dialog.confirmLabel ?? (dialog.onDecline ? m.confirm_yes() : m.confirm_confirm())}
        </button>
      </div>
    </div>
  </div>
{/if}
