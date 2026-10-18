import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { CATALOGUES, SUPPORTED_LOCALES } from "../i18n/catalogues";
import { lookupKey, type Catalogue } from "../i18n/translate";

const read = (relative: string) => readFileSync(new URL(relative, import.meta.url), "utf8");

const FILES = [
  "../components/SpeechSettings.tsx",
  "./availability.ts",
  "./voiceSession.ts",
  "../audio/recorder.ts",
];

/** Keys built from a template, listed here because a scan of the sources cannot see their values. */
const BUILT_KEYS = [
  "speech.model.missing",
  "speech.model.present",
  "speech.model.corrupt",
  "speech.engine.piper",
  "speech.engine.system",
  "speech.gender.female",
  "speech.gender.male",
  "speech.gender.mixed",
  "speech.microphone.denied",
  "speech.microphone.missing",
  "speech.microphone.unsupported",
  "speech.microphone.failed",
];

describe("the sentences of the voice settings block and the capture library", () => {
  const written = FILES.flatMap((file) =>
    [...read(file).matchAll(/"(speech\.[A-Za-z.]+)"/g)].map(([, key]) => key as string),
  );
  const keys = [...new Set([...written, ...BUILT_KEYS])].sort();

  it("finds the keys the sources name", () => {
    expect(keys).toContain("speech.groupLabel");
    expect(keys).toContain("speech.session.microphoneBusy");
  });

  it.each(SUPPORTED_LOCALES)("%s has a sentence for each of them", (locale) => {
    const catalogue = CATALOGUES[locale] as Catalogue;

    for (const key of keys) {
      expect(lookupKey(catalogue, key), key).toBeTruthy();
    }
  });
});

describe("the capture worklet and the content security policy", () => {
  const config = JSON.parse(read("../../src-tauri/tauri.conf.json")) as {
    app: { security: { csp: string; devCsp: string } };
  };

  it.each(["csp", "devCsp"] as const)("%s gives blob: to no script, worker or media", (name) => {
    const directives = config.app.security[name].split(";").map((part) => part.trim());

    for (const directive of directives) {
      const [kind] = directive.split(/\s+/);
      if (kind === "script-src" || kind === "worker-src" || kind === "media-src") {
        expect(directive, directive).not.toContain("blob:");
      }
    }
    expect(directives.some((directive) => directive.startsWith("script-src "))).toBe(true);
  });

  it("is a static file of the application, loaded from its own origin", () => {
    expect(read("../audio/recorder.ts")).toContain('"/speech/capture-worklet.js"');
    expect(read("../../public/speech/capture-worklet.js")).toContain(
      'registerProcessor("capture-processor"',
    );
  });

  it("reaches nothing: no address, no request, no storage, no import", () => {
    const worklet = read("../../public/speech/capture-worklet.js")
      .split("\n")
      .filter((line) => !line.trim().startsWith("//"))
      .join("\n");

    expect(worklet).not.toMatch(
      /https?:|fetch|XMLHttpRequest|WebSocket|importScripts|import\s|localStorage|indexedDB/,
    );
  });
});
