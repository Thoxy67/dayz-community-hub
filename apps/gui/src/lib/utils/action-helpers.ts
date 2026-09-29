import { app as s } from '$lib/state.svelte';
import { loadProfile } from '$lib/actions/profile';
import * as m from '$lib/paraglide/messages.js';

/**
 * Wrapper for action operations that handles errors with profile fallback.
 * Eliminates 20+ identical try-catch blocks across action files.
 *
 * @param operation - The async operation to execute
 * @param successMessage - Message to display on success
 * @param errorPrefix - Prefix for error message (default: translated 'Failed')
 *
 * @example
 * await withProfileFallback(
 *   () => invoke('add_favorite', { name, ip, port, password }),
 *   m.favorites_added({ name })
 * );
 */
export async function withProfileFallback<T>(
  operation: () => Promise<T>,
  successMessage: string,
  errorPrefix?: string,
): Promise<void> {
  try {
    await operation();
    s.setStatus(successMessage, 'success');
  } catch (e) {
    await loadProfile();
    const prefix = errorPrefix ?? m.error_failed();
    s.setStatus(`${prefix}: ${e}`, 'error');
  }
}
