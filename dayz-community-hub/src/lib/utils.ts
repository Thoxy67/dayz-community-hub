// ─── Shared utility functions ────────────────────────────────────────────────
// Extracted from ServersTab, FavoritesTab, HistoryTab, ServerDetailPanel,
// DirectConnectTab to eliminate ~40 duplicate implementations.

/** Threshold above which a ping is considered a timeout. */
export const PING_TIMEOUT_MS = 5_000;

/** True when the ping value represents a timeout or is missing. */
export function isTimeout(ms: number | null | undefined): boolean {
  return ms === undefined || ms === null || ms >= PING_TIMEOUT_MS;
}

/** Human-readable ping label: "42ms" or "TIMEOUT". */
export function pingLabel(ms: number | null | undefined): string {
  return isTimeout(ms) ? 'TIMEOUT' : `${ms}ms`;
}

/**
 * Tailwind text colour class for a ping value.
 * @param dimOpacity - opacity suffix for the timeout/null case (default "30").
 *   ServerDetailPanel passes "40" for slightly different styling.
 */
export function pingColor(ms: number | null | undefined, dimOpacity = '30'): string {
  if (isTimeout(ms)) return `text-base-content/${dimOpacity}`;
  if (ms! < 50)  return 'text-success';
  if (ms! < 100) return 'text-warning';
  return 'text-error';
}

/** Tailwind background colour class for a ping indicator dot. */
export function pingDot(ms: number | null | undefined): string {
  if (isTimeout(ms)) return 'bg-base-content/20';
  if (ms! < 50)  return 'bg-success';
  if (ms! < 100) return 'bg-warning';
  return 'bg-error';
}

/**
 * Tailwind text colour class for a player count.
 * @param dimOpacity - opacity suffix for the zero-player case (default "30").
 *   ServerDetailPanel passes "40".
 */
export function playerFill(players: number, max: number, dimOpacity = '30'): string {
  if (players === 0) return `text-base-content/${dimOpacity}`;
  if (players >= max) return 'text-error';
  if (players > max / 2) return 'text-warning';
  return 'text-success';
}

/**
 * Tailwind background colour class for a player bar.
 * @param showEmpty - when true, returns a dim class for zero players (default true).
 *   DirectConnectTab passes false to skip the empty case.
 */
export function playerBarColor(players: number, max: number, showEmpty = true): string {
  if (showEmpty && players === 0) return 'bg-base-content/20';
  if (players >= max) return 'bg-error';
  if (players > max / 2) return 'bg-warning';
  return 'bg-success';
}

/**
 * Format a duration in seconds to a human-readable string like "2h 15m".
 * @param showSubMinute - when true, durations under 1m display as "<1m" (default true).
 *   DirectConnectTab passes false, showing "0m" instead.
 */
export function formatDuration(secs: number, showSubMinute = true): string {
  const s = Math.floor(secs);
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  if (h > 0) return `${h}h ${m}m`;
  if (showSubMinute && m === 0) return '<1m';
  if (m > 0 || !showSubMinute) return `${m}m`;
  return '<1m';
}

/**
 * Compute a human-readable relative time string from a Unix timestamp (seconds).
 * Computed in the frontend so it never goes stale when the app is left open.
 */
export function relativeTime(ts: number): string {
  const secs = Math.floor(Date.now() / 1000) - ts;
  if (secs < 60) return 'just now';
  const mins = Math.floor(secs / 60);
  if (mins < 60) return `${mins} minute${mins === 1 ? '' : 's'} ago`;
  const hours = Math.floor(mins / 60);
  if (hours < 24) return `${hours} hour${hours === 1 ? '' : 's'} ago`;
  const days = Math.floor(hours / 24);
  return `${days} day${days === 1 ? '' : 's'} ago`;
}

/**
 * Sort icon for a column header.
 * Pure function — pass the current sort state explicitly.
 */
export function sortIcon(col: string, currentCol: string, currentAsc: boolean): string {
  if (currentCol !== col) return 'ph:arrows-down-up';
  return currentAsc ? 'ph:arrow-up' : 'ph:arrow-down';
}

/**
 * Build an SVG path string for a sparkline chart.
 * @param history - array of [timestamp, value] pairs
 * @param w - viewBox width (default 120)
 * @param h - viewBox height (default 28)
 */
export function sparklinePath(history: [number, number][], w = 120, h = 28): string {
  if (history.length < 2) return '';
  const pts = [...history].sort((a, b) => a[0] - b[0]);
  const maxVal = Math.max(...pts.map((p) => p[1]), 1);
  const step = w / (pts.length - 1);
  return pts
    .map((p, i) => {
      const x = i * step;
      const y = h - (p[1] / maxVal) * h;
      return `${i === 0 ? 'M' : 'L'}${x.toFixed(1)},${y.toFixed(1)}`;
    })
    .join(' ');
}
