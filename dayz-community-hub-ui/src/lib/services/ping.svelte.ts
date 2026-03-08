// ─── Ping Service ───────────────────────────────────────────────────────────
// Centralizes ping utilities and flash state management.

import { invoke } from '@tauri-apps/api/core';
import { app } from '$lib/state.svelte';
import type { ServerDto } from '$lib/types';
import { pingSingle as pingSingleAction } from '$lib/actions/servers';

// ── Constants ───────────────────────────────────────────────────────────────

const FLASH_DURATION_MS = 1000;

// ── Service class ───────────────────────────────────────────────────────────

class PingService {
  /** Keys with active flash animation */
  private _flashKeys = $state<Set<string>>(new Set());

  /** Pending flash cleanup timeouts */
  private _flashTimeouts = new Map<string, ReturnType<typeof setTimeout>>();

  /** Pending flash additions (batched via RAF) */
  private _pendingFlashAdds = new Set<string>();
  private _pendingFlashRemoves = new Set<string>();
  private _flashUpdatePending = false;

  // ── Public API: Flash Management ──────────────────────────────────────────

  /** Check if a key has an active ping flash */
  hasFlash(key: string): boolean {
    return this._flashKeys.has(key);
  }

  /** Batch flash state updates via RAF to reduce re-renders */
  private _flushFlashUpdates(): void {
    if (this._flashUpdatePending) return;
    this._flashUpdatePending = true;
    requestAnimationFrame(() => {
      // Apply all pending adds
      for (const key of this._pendingFlashAdds) {
        this._flashKeys.add(key);
      }
      // Apply all pending removes
      for (const key of this._pendingFlashRemoves) {
        this._flashKeys.delete(key);
      }
      this._pendingFlashAdds.clear();
      this._pendingFlashRemoves.clear();
      // Single reactive update
      this._flashKeys = new Set(this._flashKeys);
      this._flashUpdatePending = false;
    });
  }

  /** Trigger flash animation for a key */
  triggerFlash(key: string): void {
    // Clear existing timeout if any
    const existingTimeout = this._flashTimeouts.get(key);
    if (existingTimeout) {
      clearTimeout(existingTimeout);
    }

    // Queue flash add (batched)
    this._pendingFlashAdds.add(key);
    this._pendingFlashRemoves.delete(key); // Cancel pending remove
    this._flushFlashUpdates();

    // Schedule removal
    const timeout = setTimeout(() => {
      this._pendingFlashRemoves.add(key);
      this._pendingFlashAdds.delete(key); // Cancel pending add
      this._flashTimeouts.delete(key);
      this._flushFlashUpdates();
    }, FLASH_DURATION_MS);

    this._flashTimeouts.set(key, timeout);
  }

  /** Trigger flash for multiple keys */
  triggerFlashBatch(keys: string[]): void {
    for (const key of keys) {
      this.triggerFlash(key);
    }
  }

  // ── Public API: Ping Operations ───────────────────────────────────────────

  /** Ping a single server with optional flash effect */
  async pingSingle(ip: string, port: number, flash = true): Promise<number | null> {
    const key = `${ip}:${port}`;

    if (flash) {
      this.triggerFlash(key);
    }

    // Delegate to centralized action (handles all state updates)
    await pingSingleAction(ip, port);
    return app.pingCache.get(key) ?? null;
  }

  /** Ping multiple servers (visible viewport) */
  async pingVisible(targets: string[]): Promise<void> {
    if (targets.length === 0) return;

    // Mark as pending
    for (const key of targets) {
      app.pingPending.add(key);
    }
    app.pingPending = new Set(app.pingPending);

    await invoke('ping_servers', { targets }).catch(() => {});
  }

  /** Start background ping for all servers */
  async pingAllBackground(targets: string[]): Promise<void> {
    if (targets.length === 0) return;

    // Mark as pending
    for (const key of targets) {
      app.pingPending.add(key);
    }
    app.pingPending = new Set(app.pingPending);

    await invoke('ping_all_background', { targets }).catch(() => {});
  }

  // ── Public API: Utilities ─────────────────────────────────────────────────

  /** Generate canonical ping key for a server */
  pingKey(ip: string, queryPort: number): string {
    return `${ip}:${queryPort}`;
  }

  /** Generate ping key from a ServerDto */
  serverKey(server: ServerDto): string {
    return `${server.ip}:${server.query_port}`;
  }

  /** Get cached ping value for a server */
  getPing(ip: string, port: number): number | null {
    const key = `${ip}:${port}`;
    return app.pingCache.get(key) ?? null;
  }

  /** Check if a server is currently being pinged */
  isPending(ip: string, port: number): boolean {
    const key = `${ip}:${port}`;
    return app.pingPending.has(key);
  }

  /** Check if server needs pinging (not pending, no valid result, not too many failures) */
  needsPing(key: string): boolean {
    if (app.pingPending.has(key)) return false;
    const cached = app.pingCache.get(key);
    if (cached !== undefined && cached < 5000) return false; // Has valid result
    const timeoutCount = app.pingTimeouts.get(key) ?? 0;
    if (timeoutCount >= 3) return false; // Too many failures
    return true;
  }

  /** Check if server has valid ping result (not timeout) */
  hasValidPing(key: string): boolean {
    const cached = app.pingCache.get(key);
    return cached !== undefined && cached < 9999;
  }

  /** Get timeout count for a server */
  getTimeoutCount(key: string): number {
    return app.pingTimeouts.get(key) ?? 0;
  }

  /** Reset timeout count for a server */
  resetTimeoutCount(key: string): void {
    if (app.pingTimeouts.has(key)) {
      app.pingTimeouts.delete(key);
      app.pingTimeouts = new Map(app.pingTimeouts);
    }
  }

  /** Clear all flash animations */
  clearFlashes(): void {
    for (const timeout of this._flashTimeouts.values()) {
      clearTimeout(timeout);
    }
    this._flashTimeouts.clear();
    this._flashKeys = new Set();
  }
}

// ── Singleton export ────────────────────────────────────────────────────────

export const pingService = new PingService();
