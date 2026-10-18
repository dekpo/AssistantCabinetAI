import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { CATALOGUES, SUPPORTED_LOCALES } from "../i18n/catalogues";
import { lookupKey, type Catalogue } from "../i18n/translate";

const read = (relative: string) => readFileSync(new URL(relative, import.meta.url), "utf8");

/** Source without its comments, so a sentence about "sending" is not taken for a call to send. */
function code(relative: string): string {
  return read(relative)
    .replace(/\/\*[\s\S]*?\*\//g, "")
    .split("\n")
    .filter((line) => !line.trim().startsWith("//"))
    .join("\n");
}

const FILES = [
  "./repairChips.ts",
  "./useRepairSuggestions.ts",
  "../components/RepairChips.tsx",
];

describe("a name suggestion only offers", () => {
  it.each(FILES)("%s never sends, submits or touches the conversation", (file) => {
    expect(code(file)).not.toMatch(/\b(send|onSend|submit|useChat|regenerate|resend)\b/);
  });

  it.each(FILES)("%s opens no connection and keeps nothing", (file) => {
    expect(code(file)).not.toMatch(
      /https?:|fetch\(|XMLHttpRequest|WebSocket|sendBeacon|localStorage|sessionStorage|indexedDB|createObjectURL|new Blob|writeFile|@tauri-apps/,
    );
  });

  it("asks Rust for one thing only: the suggestions", () => {
    const calls = [...code("./useRepairSuggestions.ts").matchAll(/\b(speech[A-Z][A-Za-z]+)\(/g)].map(
      ([, name]) => name,
    );

    expect([...new Set(calls)]).toEqual(["speechRepairNames"]);
  });

  it("writes to the knowledge base through nothing: no entity command is imported", () => {
    for (const file of FILES) {
      expect(code(file)).not.toMatch(/entityCard|suggestQuestions|knowledge[A-Z]\w+\(|invoke\(/);
    }
  });

  it("never rewrites the draft by itself: the only call that changes it is the user's press", () => {
    const hook = code("./useRepairSuggestions.ts");
    const calls = [...hook.matchAll(/setDraft\(/g)];

    expect(calls).toHaveLength(1);
    expect(hook).toMatch(/accept: \(chipId: string, entityId: number\) => \{[\s\S]*?setDraft\(/);
  });

  it("is a hook that knows nothing of the question box, so a note editor can use it", () => {
    for (const file of ["./useRepairSuggestions.ts", "./repairChips.ts", "../components/RepairChips.tsx"]) {
      expect(code(file)).not.toMatch(/Composer|ChatPanel|useDictation|useChat|DictationBridge/);
    }
  });
});

describe("the chat panel", () => {
  const panel = read("../components/ChatPanel.tsx");

  it("calls the hook once and hands the bridge to the box", () => {
    expect(panel.match(/useRepairSuggestions\(/g)).toHaveLength(1);
    expect(panel).toMatch(/repair=\{repair\}/);
  });

  it("gives the hook the dictated words only, the reveal switch it already owns, and the selection", () => {
    expect(panel).toContain("source: dictation?.heard ?? null");
    expect(panel).toMatch(/revealNames,\s*onRevealNamesChange,/);
    expect(panel).toContain("enabled: dictation !== undefined");
  });
});

describe("the sentences of the suggestions", () => {
  const keys = [
    ...new Set(
      FILES.flatMap((file) =>
        [...read(file).matchAll(/"(speech\.repair\.[A-Za-z]+)"/g)].map(([, key]) => key as string),
      ),
    ),
  ];

  it("finds the keys the sources name", () => {
    expect(keys).toEqual(
      expect.arrayContaining([
        "speech.repair.label",
        "speech.repair.suggest",
        "speech.repair.ask",
        "speech.repair.accept",
        "speech.repair.show",
      ]),
    );
  });

  it.each(SUPPORTED_LOCALES)("%s has a sentence for each of them", (locale) => {
    const catalogue = CATALOGUES[locale] as Catalogue;

    for (const key of keys) {
      expect(lookupKey(catalogue, key), key).toBeTruthy();
    }
  });
});
