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
  "../audio/segmenter.ts",
  "./longDictation.ts",
  "./noteText.ts",
  "./useLongDictation.ts",
  "../components/notes/LiveDictation.tsx",
];

describe("the audio of a long note is never kept or sent anywhere", () => {
  it.each(FILES)("%s opens no connection, keeps no audio and writes nothing", (file) => {
    expect(code(file)).not.toMatch(
      /https?:|fetch\(|XMLHttpRequest|WebSocket|sendBeacon|localStorage|sessionStorage|indexedDB|createObjectURL|new Blob|writeFile|@tauri-apps/,
    );
  });

  it("the controller lets a passage go as soon as its text or its failure is known", () => {
    const source = code("./longDictation.ts");

    // The only holder of audio is the queue entry, which is shifted out before it is transcribed.
    expect(source).toContain("queue.shift()");
    expect(source).not.toMatch(/history|archive|recordings\.push|saved\w*\.push/);
  });
});

describe("a long dictation only fills the note", () => {
  it.each(FILES)("%s never sends a question or touches the conversation", (file) => {
    expect(code(file)).not.toMatch(/\b(send|onSend|submit|useChat|regenerate|resend)\b/);
  });

  it.each(FILES)("%s never saves, previews or analyses: only the user's click does", (file) => {
    expect(code(file)).not.toMatch(/notesSave|notesPreview|saveAndAnalyse|\.review\(|\.confirm\(|indexing/);
  });

  it("calls Rust only through the speech bridge, for these three commands", () => {
    const calls = [...code("./useLongDictation.ts").matchAll(/\b(speech[A-Z][A-Za-z]+)\(/g)].map(
      ([, name]) => name,
    );

    expect([...new Set(calls)].sort()).toEqual(["speechCancel", "speechTranscribe"]);
    expect(code("./useLongDictation.ts")).toContain("speechApplyDictationCommands");
  });

  it("sends neither a language nor a length: Rust reads them from the settings", () => {
    const source = code("./useLongDictation.ts");

    expect(source).toContain('speechTranscribe(wav, "note")');
    expect(source).not.toMatch(/language|speechTranscribe\([^)]*locale|speechApplyDictationCommands\([^)]*locale/);
  });

  it("is called by App, which sits outside the language provider, so it never reads the language context", () => {
    expect(code("./useLongDictation.ts")).not.toMatch(/useTranslation/);
  });

  it("reaches the name suggestions only through their own hook, in App", () => {
    expect(code("./useLongDictation.ts")).not.toMatch(/speechRepairNames|useRepairSuggestions/);
    expect(code("../App.tsx")).toContain("useRepairSuggestions({");
  });
});

describe("one speech job at a time", () => {
  it("has one call to the engine, inside the loop that empties the queue", () => {
    const source = code("./longDictation.ts");

    expect(source.match(/deps\.transcribe\(/g)).toHaveLength(1);
    expect(source).toMatch(/while \(mine === run && queue\.length > 0\)/);
  });

  it("takes the voice session as the notes, so the question box and the playback cannot overlap", () => {
    const source = code("./longDictation.ts");

    expect(source).toContain('deps.session.claim("notes"');
    expect(source).toContain('deps.session.release("notes")');
  });
});

describe("leaving the Notes section while a dictation runs", () => {
  const app = code("../App.tsx");

  it("is asked about by the one guard, with the recording in its context", () => {
    expect(app).toContain("guardExit(notesSession.getSnapshot().notes, exit, { recording })");
    expect(app).toContain("dictationActiveRef.current");
  });

  it("ends the recording whenever the exit goes through", () => {
    expect(app).toContain("cancelDictationRef.current();");
    expect(app).toMatch(/const keeps = exit\.kind === "switch-section" \|\| exit\.kind === "open-file"/);
  });

  it("asks before the window closes, and ends the recording with it", () => {
    expect(app).toMatch(/!notesSession\.hasUnsaved\(\) && !dictationActiveRef\.current/);
  });

  it("keeps the zone open while a recording runs", () => {
    expect(app).toContain("notesHold: hasUnsaved(notes.notes) || dictationActive");
  });

  it("shows the red dot of the switch from the recording", () => {
    expect(app).toContain("recording: longDictation?.recording ?? false");
  });
});

describe("the sentences of the long dictation", () => {
  const keys = [
    ...new Set(
      FILES.flatMap((file) =>
        [...read(file).matchAll(/"((?:speech|errors|notes|actions)\.[A-Za-z_.]+)"/g)].map(
          ([, key]) => key as string,
        ),
      ),
    ),
  ];
  /* Keys that come from a failure name or a code, which a scan of the sources cannot see. */
  const BUILT = [
    "speech.microphone.denied",
    "speech.microphone.missing",
    "speech.microphone.unsupported",
    "speech.microphone.failed",
    "speech.session.microphoneBusy",
    "errors.speech_unavailable",
    "errors.speech_engine_missing",
    "errors.speech_model_missing",
    "errors.speech_model_corrupt",
    "errors.speech_locale_unsupported",
    "errors.speech_busy",
    "errors.speech_audio_invalid",
    "errors.speech_audio_too_long",
    "errors.speech_timeout",
    "errors.speech_engine_failed",
    "errors.speech_output_rejected",
    "errors.unknown_error",
  ];

  it("finds the keys the sources name", () => {
    expect(keys).toContain("notes.dictation.waitingOne");
    expect(keys).toContain("notes.dictation.discard");
  });

  it.each(SUPPORTED_LOCALES)("%s has a sentence for each of them", (locale) => {
    const catalogue = CATALOGUES[locale] as Catalogue;

    for (const key of [...new Set([...keys, ...BUILT])]) {
      expect(lookupKey(catalogue, key), key).toBeTruthy();
    }
  });
});
