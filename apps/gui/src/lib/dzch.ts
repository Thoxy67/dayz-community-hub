/** `dzch://` links, which open this app straight on a server. Pure, so it is tested on its own. */

export type DzchTarget = {
  ip: string;
  /** The port players connect to. */
  gamePort: number;
  /** Only written when it differs from the game port. */
  queryPort?: number | null;
  name?: string | null;
  password?: string | null;
  modIds?: readonly number[];
};

/** "dzch://1.2.3.4:2302?qport=27016&name=…": what the app parses back in `parse_dzch_url`. */
export function dzchLink(t: DzchTarget): string {
  const p: string[] = [];
  if (t.queryPort && t.queryPort !== t.gamePort) p.push(`qport=${t.queryPort}`);
  if (t.name) p.push(`name=${encodeURIComponent(t.name)}`);
  if (t.password) p.push(`password=${encodeURIComponent(t.password)}`);
  if (t.modIds?.length) p.push(`mods=${t.modIds.join(",")}`);
  return `dzch://${t.ip}:${t.gamePort}${p.length ? `?${p.join("&")}` : ""}`;
}
