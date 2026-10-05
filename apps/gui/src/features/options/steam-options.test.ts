import { describe, expect, test } from "bun:test";
import { parseSteamOptions } from "./steam-options";

describe("parseSteamOptions", () => {
  test("variables, wrappers and game arguments", () =>
    expect(
      parseSteamOptions(
        "PROTON_USE_NTSYNC=1 RADV_PERFTEST=gpl,sam mangohud gamemoderun %command% -nolauncher",
      ),
    ).toEqual({
      env: [
        ["PROTON_USE_NTSYNC", "1"],
        ["RADV_PERFTEST", "gpl,sam"],
      ],
      wrappers: ["mangohud", "gamemoderun"],
      args: ["-nolauncher"],
      hasCommand: true,
    }));
  test("without %command% everything goes after the game", () =>
    expect(parseSteamOptions("-nosplash -world=empty")).toEqual({
      env: [],
      wrappers: [],
      args: ["-nosplash", "-world=empty"],
      hasCommand: false,
    }));
  test("quotes group a value", () =>
    expect(parseSteamOptions('DXVK_HUD="fps,gpuload" %command%').env).toEqual([
      ["DXVK_HUD", "fps,gpuload"],
    ]));
  test("a variable after a wrapper is the wrapper's argument", () =>
    expect(parseSteamOptions("env FOO=1 %command%").wrappers).toEqual(["env", "FOO=1"]));
});
