import { describe, expect, test } from "bun:test";
import { formatOklch, hexToOklch, oklchToHex, parseOklch } from "./oklch";
import { DEFAULT_DARK, DEFAULT_LIGHT, PRESETS, TOKEN_NAMES, presetById } from "./presets";

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
  test("text stands out from the ground", () => {
    for (const p of PRESETS) {
      const bg = parseOklch(p.tokens.bg)!.l;
      const fg = parseOklch(p.tokens.fg)!.l;
      expect(Math.abs(fg - bg), p.id).toBeGreaterThan(0.45);
    }
  });
});

describe("oklch", () => {
  test("parses percentages and fractions alike", () => {
    expect(parseOklch("oklch(65% 0.2 255)")).toEqual({ l: 0.65, c: 0.2, h: 255 });
    expect(parseOklch("oklch(0.65 0.2 255)")).toEqual({ l: 0.65, c: 0.2, h: 255 });
  });
  test("rejects what is not oklch", () => expect(parseOklch("#ff0000")).toBeNull());
  test("formats back", () =>
    expect(formatOklch({ l: 0.654, c: 0.2, h: 255.4 })).toBe("oklch(65% 0.20 255)"));
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
