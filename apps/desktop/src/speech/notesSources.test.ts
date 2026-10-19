import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { CATALOGUES, SUPPORTED_LOCALES } from "../i18n/catalogues";
import { lookupKey, type Catalogue } from "../i18n/translate";

const read = (relative: string) => readFileSync(new URL(relative, import.meta.url), "utf8");

function withoutComments(source: string): string {
  return source.replace(/\/\*[\s\S]*?\*\//g, " ").replace(/^\s*\/\/.*$/gm, " ");
}

/** The files that hold or draw the notes of the Notes section. */
const NOTES_FILES = [
  "../lib/notesDrafts.ts",
  "../lib/notesSession.ts",
  "../lib/notesFlow.ts",
  "../lib/centreSection.ts",
  "../lib/waveform.ts",
  "../components/notes/NotesPanel.tsx",
  "../components/notes/NotesDialogs.tsx",
  "../components/notes/CentreSwitch.tsx",
  "../components/notes/DictationPill.tsx",
  "../components/notes/LiveDictation.tsx",
];

describe("the notes are memory only (decision D10)", () => {
  it.each(NOTES_FILES)("%s keeps nothing on disk", (file) => {
    const code = withoutComments(read(file));

    expect(code).not.toMatch(/localStorage|sessionStorage|indexedDB|IndexedDB|document\.cookie/);
    expect(code).not.toMatch(/writeFile|plugin-fs|createWritable|showSaveFilePicker/);
  });

  it.each(NOTES_FILES)("%s opens no connection of its own", (file) => {
    const code = withoutComments(read(file));

    expect(code).not.toMatch(/\bfetch\(|XMLHttpRequest|WebSocket|EventSource|sendBeacon/);
    expect(code).not.toMatch(/@tauri-apps\/api\/core/);
  });

  it("writes only through the two notes commands, behind the user's click on the plan", () => {
    const app = withoutComments(read("../App.tsx"));

    expect(app).toContain("preview: notesPreview");
    expect(app).toContain("save: notesSave");
    expect((app.match(/notesSave/g) ?? []).length).toBe(2);
  });
});

describe("the Notes section and the voice master switch", () => {
  it("is governed by its own switch only", () => {
    const app = withoutComments(read("../App.tsx"));

    expect(app).toContain("snapshot?.settings.speech.notesEnabled");
    expect(app).not.toMatch(/speech\.enabled|speech\.dictationEnabled/);
  });

  it("does not call the speech engine: the long dictation is wired outside the notes files", () => {
    for (const file of NOTES_FILES) {
      expect(withoutComments(read(file)), file).not.toMatch(/speechTranscribe|speechCancel|useVoiceSession/);
    }
    const app = withoutComments(read("../App.tsx"));
    expect(app).toContain("dictation: notesDictation");
    expect(app).toContain("useLongDictation(");
  });
});

describe("the sentences of the Notes section", () => {
  /* Keys built from a template, which a scan of the sources cannot see. */
  const BUILT_KEYS = [
    "notes.phase.previewing",
    "notes.phase.saving",
    "notes.phase.asking",
    "notes.phase.analysing",
    "notes.report.analysis.done",
    "notes.report.analysis.declined",
  ];
  const written = NOTES_FILES.filter((file) => file.endsWith(".tsx")).flatMap((file) =>
    [...read(file).matchAll(/"((?:notes|centre)\.[A-Za-z.]+|errors\.notes_[a-z_]+)"/g)].map(
      ([, key]) => key as string,
    ),
  );
  const keys = [...new Set([...written, ...BUILT_KEYS, "notes.toggle"])].sort();

  it("finds the keys the sources name", () => {
    expect(keys).toContain("notes.plan.title");
    expect(keys).toContain("centre.switchLabel");
  });

  it.each(SUPPORTED_LOCALES)("%s has a sentence for each of them", (locale) => {
    const catalogue = CATALOGUES[locale] as Catalogue;

    for (const key of keys) {
      expect(lookupKey(catalogue, key), key).toBeTruthy();
    }
  });
});

describe("the question before a note is lost", () => {
  it("shows a line break of the sentence as a line break", () => {
    const styles = read("../styles.css");

    expect(styles).toMatch(/\.confirm__body\s*\{[^}]*white-space:\s*pre-line;/);
  });
});

describe("the close request of the window", () => {
  const capability = JSON.parse(read("../../src-tauri/capabilities/default.json")) as {
    description: string;
    permissions: string[];
  };

  it("is allowed to destroy the window, and the file says so", () => {
    expect(capability.permissions).toContain("core:window:allow-destroy");
    expect(capability.description).toMatch(/window may be destroyed/);
  });

  it("grants no other window power", () => {
    const windowPermissions = capability.permissions.filter((name) => name.startsWith("core:window:"));

    expect(windowPermissions).toStrictEqual(["core:window:allow-destroy"]);
  });

  it("is asked in one place, which closes only after the user has answered", () => {
    const code = withoutComments(read("../lib/windowClose.ts"));

    expect(code).toContain("onCloseRequested");
    expect(code).toContain("preventDefault");
    expect(code).toContain(".destroy()");
    expect(withoutComments(read("../App.tsx"))).toContain("guardWindowClose");
  });
});
