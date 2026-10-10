import { describe, expect, test } from "bun:test";
import { hexToOklch, oklchToHex, parseOklch } from "./oklch";
import { DEFAULT_DARK, DEFAULT_LIGHT, PRESETS, RETIRED, TOKEN_NAMES, presetById } from "./presets";
import { apca, audit, confusable, wcag } from "./contrast";
import { deriveTheme } from "./derive";
import { load } from "./stored";

describe("presets", () => {
  test("ids are unique", () => expect(new Set(PRESETS.map((p) => p.id)).size).toBe(PRESETS.length));
  test("every preset has every token, each a colour the parser reads", () => {
    for (const p of PRESETS) {
      for (const t of TOKEN_NAMES) {
        const v = p.tokens[t];
        expect(v, `${p.id}.${t}`).toBeString();
        expect(parseOklch(v), `${p.id}.${t} = ${v}`).not.toBeNull();
      }
    }
  });
  test("the defaults exist and have the right scheme", () => {
    expect(presetById(DEFAULT_DARK)?.scheme).toBe("dark");
    expect(presetById(DEFAULT_LIGHT)?.scheme).toBe("light");
  });
  test("a scheme matches how light the ground is", () => {
    for (const p of PRESETS) {
      const l = parseOklch(p.tokens.bg)!.l;
      if (p.scheme === "light") expect(l, p.id).toBeGreaterThan(0.6);
      else expect(l, p.id).toBeLessThan(0.6);
    }
  });
});

describe("oklch", () => {
  test("parses percentages and fractions alike", () => {
    expect(parseOklch("oklch(65% 0.2 255)")).toEqual({ l: 0.65, c: 0.2, h: 255 });
    expect(parseOklch("oklch(0.65 0.2 255)")).toEqual({ l: 0.65, c: 0.2, h: 255 });
  });
  test("rejects what is not oklch", () => expect(parseOklch("#ff0000")).toBeNull());
  test("hex round trip stays within one step per channel", () => {
    for (const hex of ["#111310", "#e3b23c", "#e6e3d3", "#2f6386", "#ffffff", "#000000"]) {
      const back = oklchToHex(hexToOklch(hex));
      for (let i = 1; i < 7; i += 2) {
        expect(
          Math.abs(parseInt(back.slice(i, i + 2), 16) - parseInt(hex.slice(i, i + 2), 16)),
          hex,
        ).toBeLessThanOrEqual(1);
      }
    }
  });
});

describe("contrast", () => {
  const black = "oklch(0% 0 0)";
  const white = "oklch(100% 0 0)";
  test("WCAG: black on white is 21:1, a colour on itself 1:1", () => {
    expect(wcag(black, white)).toBeCloseTo(21, 2);
    expect(wcag(white, white)).toBeCloseTo(1, 5);
  });
  // Reference values from the APCA-W3 README (https://github.com/Myndex/apca-w3).
  test("APCA: black on white is Lc 106.04, white on black Lc -107.88", () => {
    expect(apca(black, white)).toBeCloseTo(106.04, 1);
    expect(apca(white, black)).toBeCloseTo(-107.88, 1);
  });
});

describe("every shipped preset is legible", () => {
  for (const p of PRESETS) {
    test(p.id, () => {
      for (const c of audit(p.tokens)) {
        expect(c.ratio, `${p.id} ${c.id} ${c.fg} on ${c.bg}`).toBeGreaterThanOrEqual(c.min);
      }
      expect(confusable(p.tokens), `${p.id}: colours too close`).toEqual([]);
    });
  }
  test("every preset has a name and a blurb", () => {
    for (const p of PRESETS) {
      expect(p.name.length, p.id).toBeGreaterThan(1);
      expect(p.blurb, p.id).toStartWith("blurb");
    }
  });
});

describe("retired presets", () => {
  test("each resolves to a survivor of the same scheme", () => {
    for (const id of Object.keys(RETIRED)) {
      expect(
        PRESETS.some((p) => p.id === id),
        id,
      ).toBe(false);
      expect(presetById(id), id).toBeDefined();
    }
  });
});

describe("derive", () => {
  const cases: Array<[string, string]> = [
    ["oklch(18% 0.01 130)", "oklch(78% 0.14 85)"],
    ["oklch(12% 0.03 260)", "oklch(65% 0.2 250)"],
    ["oklch(0% 0 0)", "oklch(60% 0.22 25)"],
    ["oklch(30% 0.03 300)", "oklch(70% 0.15 300)"],
    ["oklch(95% 0.01 90)", "oklch(55% 0.15 50)"],
    ["oklch(88% 0.03 220)", "oklch(60% 0.12 200)"],
    ["oklch(99% 0 0)", "oklch(50% 0.2 145)"],
  ];
  for (const [bg, accent] of cases) {
    test(`${bg} + ${accent} passes every check`, () => {
      const d = deriveTheme(bg, accent);
      expect(d.scheme).toBe(parseOklch(bg)!.l < 0.6 ? "dark" : "light");
      for (const c of audit(d.tokens)) expect(c.ratio, c.id).toBeGreaterThanOrEqual(c.min);
      expect(confusable(d.tokens)).toEqual([]);
    });
  }
});

describe("stored choice", () => {
  const store = (v: Record<string, string>) => (k: string) => v[k] ?? null;
  test("nothing stored follows the system with the house pair", () => {
    const s = load(store({}));
    expect(s.preset).toBeNull();
    expect(s.pair).toEqual({ dark: DEFAULT_DARK, light: DEFAULT_LIGHT });
    expect(s.customs).toEqual([]);
  });
  test("a v1 single custom becomes the first of the list, still worn", () => {
    const tokens = presetById("nord")!.tokens;
    const s = load(
      store({
        "dzch.theme": JSON.stringify({
          preset: "custom",
          custom: { scheme: "dark", tokens },
          frame: { radius: 6 },
        }),
      }),
    );
    expect(s.customs).toHaveLength(1);
    expect(s.customs[0]!.tokens).toEqual(tokens);
    expect(s.preset).toBe(s.customs[0]!.id);
    expect(s.frame.radius).toBe(6);
  });
  test("a retired preset resolves to its survivor", () => {
    const s = load(store({ "dzch.theme": JSON.stringify({ preset: "dracula", custom: null }) }));
    expect(s.preset).toBe("catppuccin");
  });
  test("the previous interface's choice carries over", () => {
    expect(load(store({ "active-preset-id": "dark" })).preset).toBe(DEFAULT_DARK);
    expect(load(store({ "active-preset-id": "mint" })).preset).toBe("topo");
  });
  test("a pair naming a preset of the wrong scheme falls back", () => {
    const s = load(
      store({
        "dzch.theme": JSON.stringify({ preset: null, pair: { dark: "topo", light: "sand" } }),
      }),
    );
    expect(s.pair).toEqual({ dark: DEFAULT_DARK, light: "sand" });
  });
});
