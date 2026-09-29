import { describe, expect, test } from "bun:test";
import { untilText } from "./until";

describe("untilText", () => {
  const now = Date.parse("2026-09-29T12:00:00Z");
  test("minutes and hours", () => {
    expect(untilText("2026-09-29T12:45:00Z", now)).toBe("45 min");
    expect(untilText("2026-09-29T15:00:00Z", now)).toBe("3 h");
    expect(untilText("2026-09-29T14:15:00+00:00", now)).toBe("2 h 15 min");
  });
  test("past or unreadable", () => {
    expect(untilText("2026-09-29T11:00:00Z", now)).toBeNull();
    expect(untilText("soon", now)).toBeNull();
  });
});
