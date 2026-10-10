import { presetById } from "./presets";

/** The words a name needs: the house themes' names are translated. */
type Words = { presetChernarus: { value: string }; presetTopo: { value: string } };

/** A preset's name as shown: translated for the two house themes, as-is for the rest. */
export function presetName(id: string, t: Words): string {
  const p = presetById(id);
  if (p?.id === "chernarus") return t.presetChernarus.value;
  if (p?.id === "topo") return t.presetTopo.value;
  return p?.name ?? id;
}
