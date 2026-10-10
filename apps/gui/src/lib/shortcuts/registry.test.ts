import { describe, expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import keys from "../../content/keys.content";
import pad from "../../content/pad.content";
import nav from "../../content/nav.content";
import { addressOf, match, pieces } from "./fuzzy";
import { find, inScope, matches, normal, SHORTCUTS, VIEW_ORDER, type Scope } from "./registry";

const LOCALES = ["en", "fr", "de", "es", "ru"] as const;

/** A content entry's words in every language (a `t()` node, or an `insert(t())` around one). */
function translations(node: unknown): Record<string, unknown> | null {
  const n = node as {
    nodeType?: string;
    translation?: Record<string, unknown>;
    insertion?: unknown;
  };
  if (n?.translation) return n.translation;
  if (n?.insertion) return translations(n.insertion);
  return null;
}
function inAllLanguages(dict: { content: Record<string, unknown> }, key: string) {
  const tr = translations(dict.content[key]);
  return !!tr && LOCALES.every((l) => typeof tr[l] === "string" && (tr[l] as string).length > 0);
}

const press = (key: string, o: Partial<KeyboardEventInit> = {}) =>
  ({
    key,
    code: /^\d$/.test(key) ? `Digit${key}` : "",
    ctrlKey: false,
    metaKey: false,
    shiftKey: false,
    altKey: false,
    ...o,
  }) as KeyboardEvent;

describe("registry", () => {
  test("ids are unique", () => {
    expect(new Set(SHORTCUTS.map((s) => s.id)).size).toBe(SHORTCUTS.length);
  });

  test("no two shortcuts of a scope share a key", () => {
    const scopes = new Set(SHORTCUTS.map((s) => s.scope));
    // The window's own (global and views) answer everywhere: one pool.
    const pools: Scope[][] = [["global", "views"], ...[...scopes].map((s) => [s])];
    for (const pool of pools) {
      const seen = new Map<string, string>();
      for (const s of SHORTCUTS.filter((x) => pool.includes(x.scope)))
        for (const k of s.keys) {
          const n = normal(k);
          expect(seen.get(n), `${k}: ${s.id} and ${seen.get(n)}`).toBeUndefined();
          seen.set(n, s.id);
        }
    }
  });

  test("the list keys do not shadow the window's", () => {
    const global = new Set(
      SHORTCUTS.filter((s) => s.scope === "global" || s.scope === "views")
        .flatMap((s) => s.keys)
        .filter((k) => k !== "Esc")
        .map(normal),
    );
    for (const s of SHORTCUTS.filter((x) => !["global", "views"].includes(x.scope)))
      for (const k of s.keys) expect(global.has(normal(k)), `${s.id} ${k}`).toBe(false);
  });

  test("every label has its words in all five languages", () => {
    for (const s of SHORTCUTS) {
      if (s.view) expect(inAllLanguages(nav, s.view), s.id).toBe(true);
      else expect(inAllLanguages(keys, s.label ?? ""), `${s.id} → keys.${s.label}`).toBe(true);
      if (s.hint) expect(inAllLanguages(pad, s.hint), `${s.id} → pad.${s.hint}`).toBe(true);
    }
  });

  test("every shortcut can be reached: a key or a button", () => {
    for (const s of SHORTCUTS)
      expect(s.keys.length + (s.pad?.length ?? 0), s.id).toBeGreaterThan(0);
  });

  test("every view has a key", () => {
    const places = readFileSync(join(import.meta.dir, "../../shell/nav.ts"), "utf8");
    const ids = [...places.matchAll(/id: "([a-z]+)",\n\s+group:/g)].map((m) => m[1]);
    expect(ids.length).toBeGreaterThan(0);
    expect([...VIEW_ORDER].sort()).toEqual([...ids].sort() as never);
  });

  test("every list shortcut is handled by every server list", () => {
    const read = (p: string) => readFileSync(join(import.meta.dir, "../..", p), "utf8");
    const servers = read("features/servers/View.svelte");
    const saved = read("features/favorites/SavedServers.svelte");
    for (const s of inScope("list")) {
      expect(servers.includes(`"${s.id}"`), `servers: ${s.id}`).toBe(true);
      expect(saved.includes(`"${s.id}"`), `saved: ${s.id}`).toBe(true);
    }
    for (const s of inScope("saved")) expect(saved.includes(`"${s.id}"`), s.id).toBe(true);
    const mods = read("features/mods/View.svelte");
    for (const s of inScope("mods")) expect(mods.includes(`"${s.id}"`), s.id).toBe(true);
  });
});

describe("matching keys", () => {
  test("Ctrl combinations", () => {
    expect(matches(press("k", { ctrlKey: true }), "Ctrl+K")).toBe(true);
    expect(matches(press("K", { ctrlKey: true, shiftKey: true }), "Ctrl+K")).toBe(false);
    expect(matches(press("k"), "Ctrl+K")).toBe(false);
    expect(matches(press(",", { ctrlKey: true }), "Ctrl+,")).toBe(true);
  });
  test("digits by their physical key (AZERTY types & for 1)", () => {
    expect(matches(press("&", { ctrlKey: true, code: "Digit1" }), "Ctrl+1")).toBe(true);
    expect(find(press("&", { ctrlKey: true, code: "Digit1" }), "views")?.view).toBe("servers");
  });
  test("? whatever Shift it takes", () => {
    expect(matches(press("?", { shiftKey: true }), "?")).toBe(true);
    expect(matches(press("?"), "?")).toBe(true);
  });
  test("letters either case, named keys exactly", () => {
    expect(find(press("F", { shiftKey: true }), "list")?.id).toBe("list.fav");
    expect(find(press("Delete"), "saved")?.id).toBe("saved.remove");
    expect(find(press("ArrowDown"), "list")?.id).toBe("list.move");
    expect(find(press("Tab", { shiftKey: true }), "palette")?.id).toBe("palette.section");
    expect(find(press("Enter", { ctrlKey: true }), "list")).toBeNull();
  });
});

describe("fuzzy", () => {
  test("whole words beat scattered letters, accents and case aside", () => {
    const a = match("deer", "PvP Deer Isle")!;
    const b = match("deer", "Dead Eager Rats")!;
    expect(a.score).toBeGreaterThan(b?.score ?? -Infinity);
    expect(match("enoch", "Énoch Survival")).not.toBeNull();
  });
  test("ranges cut the text into what matched", () => {
    const m = match("isle", "Deer Isle")!;
    expect(pieces("Deer Isle", m.ranges)).toEqual([
      { text: "Deer ", hit: false },
      { text: "Isle", hit: true },
    ]);
  });
  test("no match is none", () => expect(match("zzz", "Chernarus")).toBeNull());
  test("addresses", () => {
    expect(addressOf("1.2.3.4:2302")).toEqual({ host: "1.2.3.4", port: 2302 });
    expect(addressOf("play.example.com:2402")?.port).toBe(2402);
    expect(addressOf("chernarus")).toBeNull();
    expect(addressOf("1.2.3.4:99999")).toBeNull();
  });
});

describe("fuzzy initials", () => {
  test("initials match, letters from the middle of words do not", () => {
    expect(match("dch", "DayZ Community Hub")).not.toBeNull();
    expect(match("dayco", "DayZ Community Hub")).not.toBeNull();
    expect(match("deer", "Données du profil")).toBeNull();
    expect(match("deer", "Actualiser la liste des serveurs")).toBeNull();
  });
});
