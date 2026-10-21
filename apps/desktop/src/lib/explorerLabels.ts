import type { Translator } from "../i18n/translate";
import type { KnowledgeFacts } from "./explorerFacts";
import { ROLE_MENTION } from "./explorerFilters";
import { parseEntityChoice } from "./explorerModel";

/*
 * The words the filters of the knowledge base use, from the catalogues. Codes are machine words that the
 * catalogue turns into the user's language: the business types and the roles never reach the screen as
 * codes unless the build has no word for them (a type the user defined keeps its own name).
 */

/** A business type in the user's language, or its code when the catalogue has no word for it. */
export function businessTypeLabel(t: Translator, code: string): string {
  const key = `businessType.${code}`;
  const text = t(key);
  return text === key ? code : text;
}

/** A role in the user's language, or null for a role the second step does not offer: a name met in the
 * text with no role says nothing, and a code this build has no word for would only show a code. */
export function roleChoiceLabel(t: Translator, role: string): string | null {
  if (role === ROLE_MENTION) {
    return null;
  }
  for (const key of [`explorer.role.${role}`, `knowledge.roles.${role}`]) {
    const text = t(key);
    if (text !== key) {
      return text;
    }
  }
  return null;
}

/**
 * A person or an organisation choice as a pill or a chip reads it: the name, then the role when one was
 * chosen. A name the base does not know any more says so, once the facts have been read (before that
 * the name is not yet known, and the label waits).
 */
export function entityChoiceLabel(
  t: Translator,
  value: string,
  facts: KnowledgeFacts,
  loaded: boolean,
): string {
  const { entityId, role } = parseEntityChoice(value);
  const name = facts.entities.get(entityId)?.name;
  const base = name ?? (loaded ? t("explorer.nameMissing") : "…");
  const roleText = role === null ? null : roleChoiceLabel(t, role);
  return roleText === null ? base : `${base} · ${roleText}`;
}
