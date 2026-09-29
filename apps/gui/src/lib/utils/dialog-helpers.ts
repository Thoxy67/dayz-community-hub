import { app as s } from '$lib/state.svelte';
import { withProfileFallback } from './action-helpers';

/**
 * Create a confirmation dialog with standardized error handling.
 * Eliminates 11+ duplicate confirm dialog patterns across action files.
 *
 * @param title Dialog title
 * @param message Dialog message
 * @param action Async action to perform on confirm
 * @param successMsg Success message to display
 * @param errorPrefix Error message prefix (default: 'Failed')
 * @param confirmLabel Optional custom confirm button label
 * @param confirmVariant Optional button variant ('error', 'warning', etc.)
 *
 * @example
 * // Simple usage
 * confirmAction(
 *   'Delete Mod',
 *   `Delete '${mod.name}'?`,
 *   async () => {
 *     await invoke('delete_mod', { modId: mod.id });
 *     await loadMods();
 *   },
 *   `Deleted ${mod.name}`
 * );
 *
 * @example
 * // With profile fallback
 * confirmActionWithProfile(
 *   'Remove Favorite',
 *   `Remove '${fav.name}' from favorites?`,
 *   async () => {
 *     profileRemoveFavorite(fav.ip, fav.port);
 *     await invoke('remove_favorite', { ip: fav.ip, port: fav.port });
 *   },
 *   'Removed from favorites'
 * );
 */
export function confirmAction(
  title: string,
  message: string,
  action: () => Promise<void>,
  successMsg: string,
  errorPrefix: string = 'Failed',
  confirmLabel?: string,
  confirmVariant?: 'error' | 'info' | 'warning' | 'success' | 'primary',
) {
  s.confirmDialog = {
    title,
    message,
    ...(confirmLabel && { confirmLabel }),
    ...(confirmVariant && { confirmVariant }),
    onConfirm: async () => {
      try {
        await action();
        s.setStatus(successMsg, 'success');
      } catch (e) {
        s.setStatus(`${errorPrefix}: ${e}`, 'error');
      }
    },
  };
}

/**
 * Create a confirmation dialog with profile fallback on error.
 * Use this variant for actions that modify the profile.
 */
export function confirmActionWithProfile(
  title: string,
  message: string,
  action: () => Promise<void>,
  successMsg: string,
  errorPrefix: string = 'Failed',
  confirmLabel?: string,
  confirmVariant?: 'error' | 'info' | 'warning' | 'success' | 'primary',
) {
  s.confirmDialog = {
    title,
    message,
    ...(confirmLabel && { confirmLabel }),
    ...(confirmVariant && { confirmVariant }),
    onConfirm: async () => {
      await withProfileFallback(action, successMsg, errorPrefix);
    },
  };
}
