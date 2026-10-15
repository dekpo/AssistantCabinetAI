import type { UserType } from "./ipc";

/** The prefix of every code of a user's own type, and the longest code: the same as the index's rule. */
const PREFIX = "user_";
const MAX_CODE_CHARS = 48;

/** The code a name gives: `user_`, then the name in lower case without accents, other signs as underscores. */
export function codeFromLabel(label: string): string | null {
  const slug = label
    .normalize("NFD")
    .replace(/[̀-ͯ]/g, "")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "_")
    .replace(/^_+|_+$/g, "");
  if (slug === "") {
    return null;
  }
  return (PREFIX + slug).slice(0, MAX_CODE_CHARS).replace(/_+$/, "");
}

/** One phrase a line, or separated by semicolons; blanks and repeats are dropped. */
export function phrasesFromText(text: string): string[] {
  const phrases: string[] = [];
  for (const piece of text.split(/[\n;]/)) {
    const phrase = piece.trim();
    if (phrase !== "" && !phrases.includes(phrase)) {
      phrases.push(phrase);
    }
  }
  return phrases;
}

export function textFromPhrases(phrases: string[]): string {
  return phrases.join("\n");
}

export interface UserTypeForm {
  label: string;
  scope: "documents" | "data";
  name: string;
  title: string;
  header: string;
  body: string;
  sheet: string;
}

/** The rule to keep from what was typed; `null` when it cannot be one (no name, or not one phrase). */
export function userTypeFromForm(form: UserTypeForm): UserType | null {
  const label = form.label.trim();
  const code = codeFromLabel(label);
  if (code === null) {
    return null;
  }
  const rule: UserType = {
    code,
    scope: form.scope,
    label,
    name: phrasesFromText(form.name),
    title: phrasesFromText(form.title),
    header: phrasesFromText(form.header),
    body: phrasesFromText(form.body),
    sheet: phrasesFromText(form.sheet),
  };
  const phrases = rule.name.length + rule.title.length + rule.header.length + rule.body.length + rule.sheet.length;
  return phrases === 0 ? null : rule;
}
