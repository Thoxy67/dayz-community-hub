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
  return n == null
    ? "—"
    : numberFormat({ notation: "compact", maximumFractionDigits: 1 }).format(n);
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

// `toLocaleString` builds a formatter on every call; lists format a date per row.
const df = new Map<string, Intl.DateTimeFormat>();
function dateFormat(kind: "dateTime" | "date") {
  const key = getLocale() + kind;
  let f = df.get(key);
  if (!f)
    df.set(
      key,
      (f = new Intl.DateTimeFormat(
        getLocale(),
        kind === "date"
          ? { year: "numeric", month: "short", day: "numeric" }
          : // What `toLocaleString` shows: the date and the time with seconds.
            {
              year: "numeric",
              month: "numeric",
              day: "numeric",
              hour: "numeric",
              minute: "2-digit",
              second: "2-digit",
            },
      )),
    );
  return f;
}

/** A full local date and time, for tooltips. */
export function dateTime(ts: number): string {
  return dateFormat("dateTime").format(new Date(ts * 1000));
}

/** A calendar date. */
export function date(d: Date | string | number): string {
  return dateFormat("date").format(new Date(d));
}
