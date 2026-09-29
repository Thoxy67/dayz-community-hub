import { describe, expect, test } from "bun:test";
import { isLocale, pickLocale, plural } from "./locale";

describe("pickLocale", () => {
  test("a saved choice wins", () => expect(pickLocale("ru", "fr-FR")).toBe("ru"));
  test("an unknown saved choice is ignored", () => expect(pickLocale("xx", "de-DE")).toBe("de"));
  test("regional OS languages count", () => expect(pickLocale(null, "fr-CA")).toBe("fr"));
  test("upper-case OS language", () => expect(pickLocale(null, "ES")).toBe("es"));
  test("an unshipped OS language falls back to English", () => expect(pickLocale(null, "ja-JP")).toBe("en"));
  test("nothing known falls back to English", () => expect(pickLocale(undefined, undefined)).toBe("en"));
});

describe("isLocale", () => {
  test("shipped", () => expect(isLocale("fr")).toBe(true));
  test("not shipped", () => expect(isLocale("it")).toBe(false));
  test("empty", () => expect(isLocale("")).toBe(false));
});

describe("plural", () => {
  test("one", () => expect(plural(1, "a", "b")).toBe("a"));
  test("zero is other", () => expect(plural(0, "a", "b")).toBe("b"));
  test("many", () => expect(plural(3, "a", "b")).toBe("b"));
});
