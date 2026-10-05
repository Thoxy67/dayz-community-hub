/** Steam's launch options for a game, taken apart. Pure, so it is tested on its own. */

export type SteamOptions = {
  /** `NAME=value` set before the game: Proton, DXVK/VKD3D, Mesa switches. */
  env: [string, string][];
  /** Programs the game runs inside (`mangohud`, `gamemoderun`…). */
  wrappers: string[];
  /** Arguments after the game, given to DayZ itself. */
  args: string[];
  /** `%command%` was written: without it Steam adds everything after the game. */
  hasCommand: boolean;
};

/** Split like a shell would: spaces separate, quotes group. */
function words(s: string): string[] {
  const out: string[] = [];
  let cur = "";
  let quote: string | null = null;
  let any = false;
  for (const c of s) {
    if (quote) {
      if (c === quote) quote = null;
      else cur += c;
    } else if (c === '"' || c === "'") {
      quote = c;
      any = true;
    } else if (/\s/.test(c)) {
      if (cur || any) out.push(cur);
      cur = "";
      any = false;
    } else cur += c;
  }
  if (cur || any) out.push(cur);
  return out;
}

const ENV = /^([A-Za-z_][A-Za-z0-9_]*)=(.*)$/s;

export function parseSteamOptions(raw: string): SteamOptions {
  const w = words(raw.trim());
  const at = w.indexOf("%command%");
  if (at < 0) return { env: [], wrappers: [], args: w, hasCommand: false };
  const env: [string, string][] = [];
  const wrappers: string[] = [];
  for (const t of w.slice(0, at)) {
    const m = wrappers.length === 0 ? ENV.exec(t) : null;
    if (m) env.push([m[1]!, m[2]!]);
    else wrappers.push(t);
  }
  return { env, wrappers, args: w.slice(at + 1), hasCommand: true };
}
