import { describe, expect, test } from "bun:test";
import { dzchLink } from "./dzch";

describe("dzchLink", () => {
  test("address only", () =>
    expect(dzchLink({ ip: "1.2.3.4", gamePort: 2302 })).toBe("dzch://1.2.3.4:2302"));
  test("a query port equal to the game port is left out", () =>
    expect(dzchLink({ ip: "1.2.3.4", gamePort: 2302, queryPort: 2302 })).toBe(
      "dzch://1.2.3.4:2302",
    ));
  test("everything, encoded", () =>
    expect(
      dzchLink({
        ip: "1.2.3.4",
        gamePort: 2302,
        queryPort: 27016,
        name: "My Server & co",
        password: "s3cr&t",
        modIds: [1559212036, 1564026768],
      }),
    ).toBe(
      "dzch://1.2.3.4:2302?qport=27016&name=My%20Server%20%26%20co&password=s3cr%26t&mods=1559212036,1564026768",
    ));
  test("empty name and password are left out", () =>
    expect(dzchLink({ ip: "1.2.3.4", gamePort: 2302, name: "", password: "" })).toBe(
      "dzch://1.2.3.4:2302",
    ));
});
