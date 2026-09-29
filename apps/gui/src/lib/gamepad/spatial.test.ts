import { describe, expect, test } from "bun:test";
import { best, cost, ranked, type Box } from "./spatial";

const box = (left: number, top: number, w = 40, h = 20): Box => ({
  left,
  top,
  right: left + w,
  bottom: top + h,
});

describe("cost", () => {
  const from = box(100, 100);
  test("nothing behind counts", () => {
    expect(cost(from, box(100, 40), "down")).toBeNull();
    expect(cost(from, box(100, 160), "up")).toBeNull();
    expect(cost(from, box(160, 100), "left")).toBeNull();
    expect(cost(from, box(40, 100), "right")).toBeNull();
  });
  test("a neighbour on the same line is not below", () => {
    expect(cost(from, box(150, 104), "down")).toBeNull();
  });
  test("straight below costs the gap", () => {
    expect(cost(from, box(100, 150), "down")).toBeCloseTo(30);
  });
});

describe("best", () => {
  const from = box(100, 100);
  test("prefers what is under it to what is nearer but aside", () => {
    const under = box(100, 180);
    const aside = box(220, 125);
    expect(best(from, [aside, under], "down")).toBe(1);
  });
  test("picks the nearer of two in line", () => {
    expect(best(from, [box(100, 300), box(110, 140)], "down")).toBe(1);
  });
  test("left and right", () => {
    const cands = [box(20, 100), box(200, 100), box(300, 100)];
    expect(best(from, cands, "left")).toBe(0);
    expect(best(from, cands, "right")).toBe(1);
  });
  test("-1 when nothing is that way", () => {
    expect(best(from, [box(100, 20)], "down")).toBe(-1);
  });
});

describe("ranked", () => {
  test("cheapest first, the other way left out", () => {
    const from = box(100, 100);
    expect(ranked(from, [box(100, 300), box(100, 20), box(110, 140)], "down")).toEqual([2, 0]);
  });
  test("half a line lower is still beside, not below", () => {
    const from = box(300, 207, 60, 28);
    expect(ranked(from, [box(0, 221, 200, 30)], "down")).toEqual([]);
  });
});
