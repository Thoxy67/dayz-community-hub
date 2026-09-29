/** Numbers, sizes, durations and times, formatted in the current language. */
import { getLocale, words } from "$lib/i18n";

const nf = new Map<string, Intl.NumberFormat>();
function numberFormat(opts: Intl.NumberFormatOptions = {}) {
  const key = getLocale() + JSON.stringify(opts);
  let f = nf.get(key);
  if (!f) nf.set(key, (f = new Intl.NumberFormat(getLocale(), opts)));
  return f;
}

/** 18 432 — or an em dash for nothing. */
export function num(n: number | null | undefined): string {
  return n == null ? "—" : numberFormat().format(n);
}

/** 18.4k */
export function compact(n: number | null | undefined): string {
  return n == null ? "—" : numberFormat({ notation: "compact", maximumFractionDigits: 1 }).format(n);
}

export { bytes, duration, distanceKm } from "./units";

/** "3 minutes ago", from a Unix timestamp in seconds, in the current language. */
export function relative(ts: number): string {
  const w = words("shell");
  const diff = Math.floor(Date.now() / 1000) - ts;
  if (diff < 60) return String(w.timeJustNow);
  if (diff < 120) return String(w.timeMinuteAgo);
  if (diff < 3600) return String(w.timeMinutesAgo({ count: Math.floor(diff / 60) }));
  if (diff < 7200) return String(w.timeHourAgo);
  if (diff < 86400) return String(w.timeHoursAgo({ count: Math.floor(diff / 3600) }));
  if (diff < 172800) return String(w.timeDayAgo);
  if (diff < 604800) return String(w.timeDaysAgo({ count: Math.floor(diff / 86400) }));
  if (diff < 1209600) return String(w.timeWeekAgo);
  if (diff < 2592000) return String(w.timeWeeksAgo({ count: Math.floor(diff / 604800) }));
  return String(w.timeLongAgo);
}

/** A full local date and time, for tooltips. */
export function dateTime(ts: number): string {
  return new Date(ts * 1000).toLocaleString(getLocale());
}

/** A calendar date. */
export function date(d: Date | string | number): string {
  return new Date(d).toLocaleDateString(getLocale(), { year: "numeric", month: "short", day: "numeric" });
}
