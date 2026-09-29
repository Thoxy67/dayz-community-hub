import { describe, expect, test } from "bun:test";
import { splitHostPort } from "./address";

describe("splitHostPort", () => {
  test("ip and port", () =>
    expect(splitHostPort("1.2.3.4:2402")).toEqual({ host: "1.2.3.4", port: 2402 }));
  test("bare ip has no port", () =>
    expect(splitHostPort("1.2.3.4")).toEqual({ host: "1.2.3.4", port: null }));
  test("trims whitespace", () =>
    expect(splitHostPort("  1.2.3.4:2302 ")).toEqual({ host: "1.2.3.4", port: 2302 }));
  test("hostname and port", () =>
    expect(splitHostPort("play.example.org:2302")).toEqual({
      host: "play.example.org",
      port: 2302,
    }));
  test("a non-numeric port stays part of the host", () =>
    expect(splitHostPort("1.2.3.4:abc")).toEqual({ host: "1.2.3.4:abc", port: null }));
  test("an out-of-range port is not a port", () =>
    expect(splitHostPort("1.2.3.4:70000")).toEqual({ host: "1.2.3.4:70000", port: null }));
  test("port 0 is not a port", () => expect(splitHostPort("1.2.3.4:0").port).toBeNull());
  test("bare IPv6 keeps its colons", () =>
    expect(splitHostPort("2001:db8::1")).toEqual({ host: "2001:db8::1", port: null }));
  test("bracketed IPv6 with port", () =>
    expect(splitHostPort("[2001:db8::1]:2302")).toEqual({ host: "2001:db8::1", port: 2302 }));
  test("bracketed IPv6 without port", () =>
    expect(splitHostPort("[::1]")).toEqual({ host: "::1", port: null }));
});
