import * as m from '$lib/paraglide/messages.js';

/**
 * Format a Unix timestamp (seconds) into a localized relative time string.
 */
export function formatRelativeTime(timestamp: number): string {
  const now = Math.floor(Date.now() / 1000);
  const diff = now - timestamp;

  if (diff < 0) {
    return m.time_just_now();
  }

  if (diff < 60) {
    return m.time_just_now();
  } else if (diff < 120) {
    return m.time_minute_ago();
  } else if (diff < 3600) {
    return m.time_minutes_ago({ count: Math.floor(diff / 60) });
  } else if (diff < 7200) {
    return m.time_hour_ago();
  } else if (diff < 86400) {
    return m.time_hours_ago({ count: Math.floor(diff / 3600) });
  } else if (diff < 172800) {
    return m.time_day_ago();
  } else if (diff < 604800) {
    return m.time_days_ago({ count: Math.floor(diff / 86400) });
  } else if (diff < 1209600) {
    return m.time_week_ago();
  } else if (diff < 2592000) {
    return m.time_weeks_ago({ count: Math.floor(diff / 604800) });
  } else {
    return m.time_long_ago();
  }
}
