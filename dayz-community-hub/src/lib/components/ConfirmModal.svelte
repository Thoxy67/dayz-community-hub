<script lang="ts">
  import type { ConfirmDialog } from '$lib/types';

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
  <!-- inset-0 is overridden: top starts at 36px (h-9 titlebar) so the
       titlebar stays interactive and draggable while the modal is open. -->
  <div class="modal modal-open" style="top: 36px;">
    <div class="modal-box max-w-md">
      <h3 class="font-bold text-lg text-warning">{dialog.title}</h3>
      <div class="py-4 whitespace-pre-wrap text-base-content/80 text-sm leading-relaxed">
        {dialog.message}
      </div>
      <div class="modal-action">
        {#if dialog.onCancel}
          <button class="btn btn-outline btn-error btn-sm" onclick={cancel}>Cancel</button>
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
          {dialog.onDecline ? (dialog.declineLabel ?? 'No, connect anyway') : 'Cancel'}
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
          {dialog.confirmLabel ?? (dialog.onDecline ? 'Yes' : 'Confirm')}
        </button>
      </div>
    </div>
    <button class="modal-backdrop bg-base-content/20" onclick={cancel} aria-label="Close dialog"></button>
  </div>
{/if}
