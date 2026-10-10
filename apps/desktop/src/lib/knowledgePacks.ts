/**
 * The health module of the knowledge base, as the settings dialog offers it. The product serves
 * health professionals in France and in Switzerland, whose patients carry different numbers (the
 * French social security number, the Swiss AVS / AHV number), so the module has a country option
 * (`docs/DECISIONS.md`, "Health professionals in Switzerland").
 *
 * What is stored is the list of lexicon pack ids (`knowledgePacks` in the settings). The choice
 * below is only how that list is shown and edited; any other pack in the list (a legal or an
 * accounting pack) is carried through untouched.
 *
 * A pack id is `<domain>-<country>` (`health-fr`, `health-ch`, later `health-eu`, `legal-ch`...): a
 * module for another country is another pack, never a switch inside one
 * (`docs/DECISIONS.md`, "Knowledge packs are named domain-country").
 */
export type HealthModule = "none" | "france" | "switzerland";

export const HEALTH_PACK = "health-fr";
export const SWISS_HEALTH_PACK = "health-ch";

export const HEALTH_MODULES: HealthModule[] = ["france", "switzerland", "none"];

export function healthModuleOf(packs: readonly string[]): HealthModule {
  if (packs.includes(SWISS_HEALTH_PACK)) {
    return "switzerland";
  }
  return packs.includes(HEALTH_PACK) ? "france" : "none";
}

/** The health packs, whatever their country: `health-fr`, `health-ch`, later `health-eu`. */
function isHealthPack(id: string): boolean {
  return id.startsWith("health-");
}

/**
 * The pack list after the user picks a module: the packs of other domains stay, and exactly one
 * health pack is set. Switzerland is `health-ch` **instead of** `health-fr`, never beside it: the
 * modules of two countries are not mixed (`docs/DECISIONS.md`, "Knowledge packs are named
 * domain-country").
 */
export function packsForHealthModule(packs: readonly string[], choice: HealthModule): string[] {
  const others = packs.filter((id) => !isHealthPack(id));
  switch (choice) {
    case "france":
      return [HEALTH_PACK, ...others];
    case "switzerland":
      return [SWISS_HEALTH_PACK, ...others];
    case "none":
      return others;
  }
}
