/**
 * Reading a tool's log lines: what kind of line each is (by the words tools
 * use for errors, warnings and success), a byte rate that does not jump, and
 * a clock for elapsed times.
 */
export type LineKind = "err" | "warn" | "ok" | "progress" | "login" | "dim" | "normal";

export function classifyLine(line: string): LineKind {
  const l = line.toLowerCase();
  if (
    /\berror\b|failed|failure|abort|not found|invalid password|access denied|no connection|timeout expired/.test(
      l,
    )
  )
    return "err";
  if (/warning|timed out|retry|retrying|steam guard|two-factor|rate limit/.test(l)) return "warn";
  if (/success|already up to date|fully installed|logged in ok|downloaded item|\bok\b\.?$/.test(l))
    return "ok";
  if (/downloading|update state|reconfiguring|validating|progress:|\d+\s*%/.test(l))
    return "progress";
  if (/logging in|\+login|connecting|loading steam|steamcmd|^steam>/.test(l)) return "login";
  if (l.trim() === "" || /^\[|appinfo|waiting on|idle|^\s*\d+\s*$/.test(l)) return "dim";
  return "normal";
}

export const LINE_CLASS: Record<LineKind, string> = {
  err: "text-err",
  warn: "text-warn",
  ok: "text-ok",
  progress: "text-info",
  login: "text-accent",
  dim: "text-fg-faint",
  normal: "text-fg-muted",
};

/**
 * Bytes per second over the last few seconds of samples, so one burst does
 * not swing the figure. A sample that goes backwards (a new file) resets it.
 */
export class Rate {
  #samples: { t: number; bytes: number }[] = [];
  constructor(private windowMs = 5000) {}

  add(t: number, bytes: number) {
    const last = this.#samples.at(-1);
    if (last && bytes < last.bytes) this.#samples = [];
    this.#samples.push({ t, bytes });
    while (this.#samples.length > 2 && t - this.#samples[0]!.t > this.windowMs)
      this.#samples.shift();
  }

  /** Bytes per second, or null before there are two samples a moment apart. */
  get perSecond(): number | null {
    const a = this.#samples[0];
    const b = this.#samples.at(-1);
    if (!a || !b || b.t - a.t < 400) return null;
    return ((b.bytes - a.bytes) * 1000) / (b.t - a.t);
  }

  reset() {
    this.#samples = [];
  }
}

/** m:ss, or h:mm:ss past the hour. */
export function clock(ms: number): string {
  const s = Math.max(0, Math.floor(ms / 1000));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const sec = String(s % 60).padStart(2, "0");
  return h > 0 ? `${h}:${String(m).padStart(2, "0")}:${sec}` : `${m}:${sec}`;
}
