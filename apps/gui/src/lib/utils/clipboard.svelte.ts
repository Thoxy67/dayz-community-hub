import { writeText } from '@tauri-apps/plugin-clipboard-manager';
import { COPY_FLASH_MS } from '$lib/constants/timings';

/**
 * Creates a keyed clipboard copy state manager.
 * Tracks which key was last copied and auto-clears after timeout.
 *
 * @param flashMs - Custom flash duration (defaults to COPY_FLASH_MS)
 * @example
 * const { copiedKey, copy, isCopied } = createCopyState();
 * await copy('192.168.1.1:2302', 'server-1');
 * // copiedKey === 'server-1' for 1500ms, then clears
 */
export function createCopyState(flashMs = COPY_FLASH_MS) {
  let copiedKey = $state('');
  let timeout: ReturnType<typeof setTimeout> | undefined;

  async function copy(text: string, key?: string) {
    await writeText(text);
    const k = key ?? text;
    copiedKey = k;
    clearTimeout(timeout);
    timeout = setTimeout(() => {
      if (copiedKey === k) copiedKey = '';
    }, flashMs);
  }

  function isCopied(key: string): boolean {
    return copiedKey === key;
  }

  return {
    get copiedKey() {
      return copiedKey;
    },
    copy,
    isCopied,
  };
}

/**
 * Simple copy helper that returns a flash state.
 * Use when you don't need to track multiple copy targets.
 *
 * @param flashMs - Custom flash duration (defaults to COPY_FLASH_MS)
 * @example
 * const { copied, copy } = createSimpleCopyState();
 * await copy('192.168.1.1:2302');
 * // copied === true for 1500ms, then false
 */
export function createSimpleCopyState(flashMs = COPY_FLASH_MS) {
  let copied = $state(false);
  let timeout: ReturnType<typeof setTimeout> | undefined;

  async function copy(text: string) {
    await writeText(text);
    copied = true;
    clearTimeout(timeout);
    timeout = setTimeout(() => {
      copied = false;
    }, flashMs);
  }

  function reset() {
    clearTimeout(timeout);
    copied = false;
  }

  return {
    get copied() {
      return copied;
    },
    copy,
    reset,
  };
}

/**
 * One-shot copy to clipboard with no state tracking.
 */
export async function copyToClipboard(text: string): Promise<void> {
  await writeText(text);
}
