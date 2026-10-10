/**
 * The health modules of the knowledge base, as the settings dialog offers them: one choice per
 * country, any number of them on at once. The product serves health professionals in France and in
 * Switzerland, and a cross-border worker has both a French and a Swiss number, so the modules are
 * cumulative, never exclusive (`docs/DECISIONS.md`, "Knowledge packs are named domain-country").
 *
 * What is stored is the list of lexicon pack ids (`knowledgePacks` in the settings); a pack id is
 * `<domain>-<country>`. A module only improves what is read (identifiers, columns, vocabulary): it
 * never removes or changes the type of a name, which Rust guarantees. Any other pack in the list
 * (a legal or an accounting pack) is carried through untouched.
 */
export const HEALTH_MODULES = [
  { id: "health-fr", labelKey: "settings.healthModule.france" },
  { id: "health-ch", labelKey: "settings.healthModule.switzerland" },
] as const;

export type HealthModuleId = (typeof HEALTH_MODULES)[number]["id"];

export function isModuleOn(packs: readonly string[], id: HealthModuleId): boolean {
  return packs.includes(id);
}

/** The pack list after a module is switched on or off: only that pack changes, once, in order. */
export function withModule(packs: readonly string[], id: HealthModuleId, on: boolean): string[] {
  const others = packs.filter((known) => known !== id);
  return on ? [...others, id] : others;
}
