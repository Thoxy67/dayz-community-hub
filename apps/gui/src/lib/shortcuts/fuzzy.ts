/**
 * Fuzzy-ish matching for the command palette: a word found whole wins, at
 * the start of a word more; else every word of the query somewhere; else
 * the letters in order where each run starts a word ("dch" finds "DayZ
 * Community Hub"), closer together better. Accents and case do not count. Returns the
 * matched character ranges so the palette can underline them.
 */
export type Match = { score: number; ranges: [number, number][] };

/** Lower case, accents gone: "Énoch" finds "enoch". One character per character. */
export const fold = (t: string) =>
  t
    .normalize("NFD")
    .replace(/\p{M}/gu, "")
    .toLowerCase()
    // Decomposition can leave a different length for rare characters: keep it aligned.
    .slice(0, t.length)
    .padEnd(t.length, " ");

const wordStart = (t: string, i: number) => i === 0 || /[\s\-_.:/[(|·]/.test(t[i - 1]!);

/** How well `query` (already folded) matches `text`; `null` if not at all. */
export function match(query: string, text: string): Match | null {
  if (!query) return { score: 0, ranges: [] };
  const t = fold(text);
  const at = t.indexOf(query);
  if (at >= 0) {
    // Whole: 1000, at a word's start +200, at the very start +100, shorter texts first.
    const score =
      1000 + (wordStart(t, at) ? 200 : 0) + (at === 0 ? 100 : 0) - Math.min(t.length, 100);
    return { score, ranges: [[at, at + query.length]] };
  }
  // Each word of the query found somewhere ("deer pvp" in "PvP Deer Isle").
  const parts = query.split(/\s+/).filter(Boolean);
  if (parts.length > 1) {
    const ranges: [number, number][] = [];
    for (const w of parts) {
      const i = t.indexOf(w);
      if (i < 0) {
        ranges.length = 0;
        break;
      }
      ranges.push([i, i + w.length]);
    }
    if (ranges.length === parts.length)
      return { score: 600 - Math.min(t.length, 100), ranges: merge(ranges) };
  }
  // The letters in order, each new run starting a word: initials ("dch"
  // finds "DayZ Community Hub"), never letters picked out of the middle of words.
  const q = query.replace(/\s+/g, "");
  const ranges: [number, number][] = [];
  let score = 300;
  let from = 0;
  let prev = -2;
  for (const ch of q) {
    let i = t.indexOf(ch, from);
    if (i !== prev + 1) {
      while (i >= 0 && !wordStart(t, i)) i = t.indexOf(ch, i + 1);
    }
    if (i < 0) return null;
    if (i === prev + 1) {
      ranges[ranges.length - 1]![1] = i + 1;
      score += 5;
    } else {
      ranges.push([i, i + 1]);
      score -= Math.min(i - from, 20);
    }
    prev = i;
    from = i + 1;
  }
  return { score: score - Math.min(t.length, 100) / 2, ranges };
}

function merge(r: [number, number][]): [number, number][] {
  const s = [...r].sort((a, b) => a[0] - b[0]);
  const out: [number, number][] = [];
  for (const x of s) {
    const last = out.at(-1);
    if (last && x[0] <= last[1]) last[1] = Math.max(last[1], x[1]);
    else out.push([...x]);
  }
  return out;
}

/** `text` cut into pieces, each marked whether it matched: for drawing. */
export function pieces(text: string, ranges: [number, number][]) {
  const out: { text: string; hit: boolean }[] = [];
  let at = 0;
  for (const [a, b] of ranges) {
    if (a > at) out.push({ text: text.slice(at, a), hit: false });
    out.push({ text: text.slice(a, b), hit: true });
    at = b;
  }
  if (at < text.length) out.push({ text: text.slice(at), hit: false });
  return out;
}

/** "1.2.3.4:2302" or "host.example:2302": an address to connect to. */
export function addressOf(text: string): { host: string; port: number } | null {
  const m = /^(\d{1,3}(?:\.\d{1,3}){3}|[a-z0-9-]+(?:\.[a-z0-9-]+)+):(\d{2,5})$/i.exec(text.trim());
  const port = m ? Number(m[2]) : 0;
  return m && port > 0 && port < 65536 ? { host: m[1]!, port } : null;
}
