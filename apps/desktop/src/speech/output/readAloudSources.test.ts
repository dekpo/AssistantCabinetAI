import { readdirSync, readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { CATALOGUES, SUPPORTED_LOCALES } from "../../i18n/catalogues";
import { lookupKey, type Catalogue } from "../../i18n/translate";
import { noteKey, noteSentence, NOTE_CODES } from "./pauses";

const read = (relative: string) => readFileSync(new URL(relative, import.meta.url), "utf8");

const OUTPUT_FILES = readdirSync(new URL(".", import.meta.url))
  .filter((name) => /\.tsx?$/.test(name) && !name.endsWith(".test.ts"))
  .map((name) => `./${name}`);
const BUTTON_FILES = ["../../components/ReadAloudButton.tsx", "../../components/SpeakerGlyph.tsx"];

/** The code of a file without its comments, so a sentence that explains a rule is not read as a use. */
function code(path: string): string {
  return read(path)
    .replace(/\/\*[\s\S]*?\*\//g, " ")
    .replace(/^\s*\/\/.*$/gm, " ");
}

describe("the read-aloud sources", () => {
  it("finds the files to scan", () => {
    expect(OUTPUT_FILES).toContain("./readAloud.ts");
    expect(OUTPUT_FILES).toContain("./systemVoices.ts");
    expect(OUTPUT_FILES).toContain("./playback.ts");
  });

  it.each([...OUTPUT_FILES, ...BUTTON_FILES])("%s reaches no network, no blob and no media element", (path) => {
    expect(code(path)).not.toMatch(
      /\bfetch\s*\(|XMLHttpRequest|WebSocket|EventSource|sendBeacon|createObjectURL|\bBlob\b|new Audio\b|<audio|<video|localStorage|sessionStorage|indexedDB/,
    );
  });

  it("names no server, no provider, no model and no address", () => {
    for (const path of OUTPUT_FILES) {
      expect(code(path), path).not.toMatch(/https?:\/\/|serverUrl|gateway|\/v1\//i);
    }
  });

  it("starts nothing at import: no call outside a function in the engine and the reading files", () => {
    for (const path of ["./systemVoices.ts", "./readAloud.ts", "./playback.ts"]) {
      expect(code(path), path).not.toMatch(/^(speechSynthesis|window\.speechSynthesis|new AudioContext)/m);
    }
  });

  it("gives the content security policy no media-src, and no blob: to a script, a worker or a media", () => {
    const config = JSON.parse(read("../../../src-tauri/tauri.conf.json")) as {
      app: { security: { csp: string; devCsp: string } };
    };

    for (const policy of [config.app.security.csp, config.app.security.devCsp]) {
      const directives = policy.split(";").map((part) => part.trim());
      expect(directives.some((directive) => directive.startsWith("media-src"))).toBe(false);
      for (const directive of directives) {
        if (/^(script-src|worker-src|default-src)/.test(directive)) {
          expect(directive, directive).not.toContain("blob:");
        }
      }
    }
  });

  it("starts a reading only from a click handler of the button", () => {
    const button = code("../../components/ReadAloudButton.tsx");

    const starts = [...button.matchAll(/controller\.start\(/g)];
    expect(starts).toHaveLength(1);
    expect(button).toMatch(/onClick=\{\(\) => \{[\s\S]*controller\.start\(/);
    const effects = [...button.matchAll(/useEffect\([\s\S]*?\}, \[[^\]]*\]\);/g)].map(([text]) => text);
    expect(effects).toHaveLength(1);
    expect(effects.join("")).not.toContain(".start(");
  });

  it("ends the reading of a button when it leaves the screen", () => {
    const button = code("../../components/ReadAloudButton.tsx");

    expect(button).toMatch(/useEffect\(\(\) => \{\s*return \(\) => \{[\s\S]*leaveEntry\(/);
  });

  it("mounts the button once in the conversation, after Regenerate, in the idle row of a finished answer", () => {
    const list = code("../../components/MessageList.tsx");

    expect(list.match(/<ReadAloudButton/g)).toHaveLength(1);
    expect(list.indexOf("<ReadAloudButton")).toBeGreaterThan(list.indexOf('"actions.regenerate"'));
    expect(list.indexOf("<ReadAloudButton")).toBeLessThan(list.indexOf("showDiagnostics && idle"));
  });
});

describe("the sentences of the read-aloud button and its notes", () => {
  const SENTENCES = [
    "speech.readAloud.start",
    "speech.readAloud.stop",
    "speech.readAloud.nothingToRead",
    "speech.readAloudNote.rowOne",
    "speech.readAloudNote.rowMany",
    "speech.readAloudNote.lineOne",
    "speech.readAloudNote.lineMany",
    "speech.systemVoices.found",
    "speech.systemVoices.none",
    "speech.systemVoices.ignoredOne",
    "speech.systemVoices.ignoredMany",
    "errors.tts_no_local_voice",
    "errors.tts_failed",
    ...NOTE_CODES.map(noteKey),
  ];

  it.each(SUPPORTED_LOCALES)("%s has a sentence for each of them", (locale) => {
    const catalogue = CATALOGUES[locale] as Catalogue;

    for (const key of SENTENCES) {
      expect(lookupKey(catalogue, key), key).toBeTruthy();
    }
  });

  it.each(SUPPORTED_LOCALES)("%s says every note code with its size in the right plural", (locale) => {
    const catalogue = CATALOGUES[locale] as Catalogue;
    const t = (key: string, values?: Record<string, string | number>) =>
      (lookupKey(catalogue, key) ?? key).replace(/\{(\w+)\}/g, (_, name: string) => String(values?.[name]));

    for (const noteCode of NOTE_CODES) {
      const sentence = noteSentence(t, noteCode, 12);
      expect(sentence).toMatch(/12/);
      expect(sentence).not.toContain("{");
      expect(sentence).not.toContain("undefined");
    }
    expect(noteSentence(t, "code_omitted", 1)).toMatch(/\b1 \S+/);
    expect(noteSentence(t, "code_omitted", 1)).not.toBe(noteSentence(t, "code_omitted", 2));
  });

  it("every message the window can show for a reading is in the catalogue", () => {
    const source = code("./readAloud.ts") + code("./ReadAloudContext.tsx");
    const keys = [...source.matchAll(/"((?:speech|errors)\.[A-Za-z_.]+)"/g)].map(([, key]) => key as string);

    for (const locale of SUPPORTED_LOCALES) {
      for (const key of keys) {
        expect(lookupKey(CATALOGUES[locale] as Catalogue, key), `${locale} ${key}`).toBeTruthy();
      }
    }
  });
});
