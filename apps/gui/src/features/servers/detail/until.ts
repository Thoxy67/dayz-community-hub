/** "2 h 15 min" until an ISO time, or null once it is past or unreadable. */
export function untilText(iso: string, now = Date.now()): string | null {
  const t = new Date(iso).getTime();
  if (Number.isNaN(t)) return null;
  const mins = Math.round((t - now) / 60_000);
  if (mins < 0) return null;
  if (mins < 60) return `${mins} min`;
  const h = Math.floor(mins / 60);
  const m = mins % 60;
  return m ? `${h} h ${m} min` : `${h} h`;
}
