import { describe, expect, test } from "bun:test";
import { BLOCK, blocksFor, isFar } from "./blocks";

describe("blocksFor", () => {
  test("nothing known yet still asks for the first block", () =>
    expect(blocksFor(0, 0, 0)).toEqual([0]));
  test("top of a long list: the block in view and the next", () =>
    expect(blocksFor(0, 20, 9500)).toEqual([0, 1]));
  test("middle: one block of margin each side", () =>
    expect(blocksFor(350, 380, 9500)).toEqual([2, 3, 4]));
  test("a view across a block boundary", () =>
    expect(blocksFor(190, 215, 9500)).toEqual([0, 1, 2, 3]));
  test("never past the end", () => expect(blocksFor(9480, 9500, 9500)).toEqual([93, 94]));
  test("a short list is one block", () => expect(blocksFor(0, 12, 12)).toEqual([0]));
  test("exact multiple of the block size", () => expect(blocksFor(180, 200, 200)).toEqual([0, 1]));
  test("custom block size", () => expect(blocksFor(25, 30, 100, 10)).toEqual([1, 2, 3]));
});

describe("isFar", () => {
  test("in view is not far", () => expect(isFar(500, 480, 520)).toBe(false));
  test("within the kept margin is not far", () =>
    expect(isFar(480 - 4 * BLOCK, 480, 520)).toBe(false));
  test("beyond the margin above is far", () =>
    expect(isFar(480 - 4 * BLOCK - 1, 480, 520)).toBe(true));
  test("beyond the margin below is far", () => expect(isFar(520 + 4 * BLOCK, 480, 520)).toBe(true));
});
