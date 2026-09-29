/**
 * A map's name as players say it, from the id servers report: `enoch` is
 * Livonia, `chernarusplus` is Chernarus+. Unknown ids are split and
 * capitalised, which reads well enough for community maps.
 */
const KNOWN: Record<string, string> = {
  chernarusplus: "Chernarus+",
  chernarus: "Chernarus",
  enoch: "Livonia",
  sakhal: "Sakhal",
  namalsk: "Namalsk",
  deerisle: "Deer Isle",
  banov: "Banov",
  esseker: "Esseker",
  takistanplus: "Takistan+",
  pripyat: "Pripyat",
  chiemsee: "Chiemsee",
  rostow: "Rostow",
  valning: "Valning",
  iztek: "Iztek",
  melkart: "Melkart",
  anastara: "Anastara",
  alteria: "Alteria",
  swansisland: "Swans Island",
  pnw: "Pacific Northwest",
};

export function mapName(id: string | null | undefined): string {
  if (!id) return "";
  const k = id.toLowerCase();
  if (KNOWN[k]) return KNOWN[k];
  return id
    .replace(/[_-]+/g, " ")
    .replace(/([a-z])([A-Z])/g, "$1 $2")
    .replace(/\b\w/g, (c) => c.toUpperCase());
}
