import { describe, expect, test } from "bun:test";
import { bytes, distanceKm, duration } from "./units";

describe("bytes", () => {
  test("stays in bytes under a kilobyte", () => expect(bytes(512)).toBe("512 B"));
  test("one decimal below 100 of a unit", () => expect(bytes(1536)).toBe("1.5 KB"));
  test("no decimal from 100 of a unit", () => expect(bytes(150 * 1024 * 1024)).toBe("150 MB"));
  test("gigabytes", () => expect(bytes(1.4 * 1024 ** 3)).toBe("1.4 GB"));
  test("caps at terabytes", () => expect(bytes(5 * 1024 ** 5)).toBe("5120 TB"));
});

describe("duration", () => {
  test("under a minute", () => expect(duration(42)).toBe("<1m"));
  test("minutes", () => expect(duration(125)).toBe("2m"));
  test("hours and minutes", () => expect(duration(2 * 3600 + 15 * 60 + 9)).toBe("2h 15m"));
  test("negative is treated as nothing", () => expect(duration(-5)).toBe("<1m"));
});

describe("distanceKm", () => {
  test("same point is zero", () => expect(distanceKm([2.35, 48.85], [2.35, 48.85])).toBe(0));
  test("Paris to Berlin is about 878 km", () => {
    const d = distanceKm([2.3522, 48.8566], [13.405, 52.52]);
    expect(d).toBeGreaterThan(870);
    expect(d).toBeLessThan(885);
  });
  test("symmetric", () => {
    const a: [number, number] = [-74, 40.7];
    const b: [number, number] = [139.7, 35.7];
    expect(distanceKm(a, b)).toBeCloseTo(distanceKm(b, a), 6);
  });
});
