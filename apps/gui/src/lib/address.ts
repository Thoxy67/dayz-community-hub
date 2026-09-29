/** Server addresses as people type and paste them. Pure, so it is tested on its own. */

/** DayZ's default game port. */
export const DEFAULT_GAME_PORT = 2302;

/**
 * "1.2.3.4:2402" → host and port. A bare host has no port; an IPv6 address
 * keeps its colons unless it is bracketed ("[::1]:2302"). Whitespace around
 * is ignored.
 */
export function splitHostPort(raw: string): { host: string; port: number | null } {
  const s = raw.trim();
  const bracket = /^\[([^\]]+)\](?::(\d+))?$/.exec(s);
  if (bracket) return { host: bracket[1]!, port: bracket[2] ? portOf(bracket[2]) : null };
  const i = s.indexOf(":");
  if (i !== -1 && i === s.lastIndexOf(":")) {
    const port = portOf(s.slice(i + 1));
    if (port !== null) return { host: s.slice(0, i), port };
  }
  return { host: s, port: null };
}

function portOf(p: string): number | null {
  if (!/^\d{1,5}$/.test(p)) return null;
  const n = Number(p);
  return n >= 1 && n <= 65535 ? n : null;
}
