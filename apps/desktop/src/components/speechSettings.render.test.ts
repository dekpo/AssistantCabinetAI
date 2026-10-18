import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { beforeAll, describe, expect, it, vi } from "vitest";
import { I18nProvider } from "../i18n/I18nProvider";
import { DEFAULT_SPEECH_SETTINGS, type AppSettings } from "../lib/ipc";
import type { SpeechStatus } from "../speech/ipc";
import { SpeechSettingsBlock } from "./SpeechSettings";

vi.mock("@tauri-apps/api/core", () => ({ Channel: class {}, invoke: vi.fn() }));

let answered: { status: SpeechStatus | null; failed: boolean } = { status: null, failed: false };
vi.mock("../state/useSpeechStatus", () => ({ useSpeechStatus: () => answered }));

beforeAll(() => {
  Object.assign(globalThis, { document: { documentElement: {} } });
});

function settings(speech: Partial<typeof DEFAULT_SPEECH_SETTINGS> = {}): AppSettings {
  return {
    locale: "fr-FR",
    workFolder: null,
    speech: { ...DEFAULT_SPEECH_SETTINGS, ...speech },
  } as unknown as AppSettings;
}

function draw(app: AppSettings, locale = "fr-FR"): string {
  return renderToStaticMarkup(
    createElement(I18nProvider, {
      locale,
      children: createElement(SpeechSettingsBlock, { settings: app, onSave: () => {} }),
    }),
  );
}

const MISSING_MODEL: SpeechStatus = {
  enabled: false,
  locale: "fr-FR",
  features: {
    dictation: { available: false, reason: "speech_unavailable" },
    readAloud: { available: false, reason: "speech_unavailable" },
    notes: { available: false, reason: "notes_folder_unavailable" },
  },
  sttModel: "missing",
  languages: [
    { id: "fr-FR", support: "enabled", dictation: true, readAloud: true, voices: [] },
    { id: "de-DE", support: "planned", dictation: false, readAloud: false, voices: [] },
  ],
};

describe("the voice block of the settings", () => {
  it("is one details block that starts closed", () => {
    answered = { status: null, failed: false };
    const markup = draw(settings());

    expect(markup.startsWith('<details class="field settings-voice">')).toBe(true);
    expect(markup).not.toContain("<details open");
    expect(markup.match(/<details/g)).toHaveLength(1);
  });

  it("says the model is missing and offers Install, disabled, with the reason", () => {
    answered = { status: MISSING_MODEL, failed: false };
    const markup = draw(settings());

    expect(markup).toContain("Non installé.");
    expect(markup).toMatch(/<button[^>]*disabled=""[^>]*>Installer<\/button>/);
    expect(markup).toContain("n&#x27;est pas encore disponible");
  });

  it("says a model is damaged or ready in words", () => {
    answered = { status: { ...MISSING_MODEL, sttModel: "corrupt" }, failed: false };
    expect(draw(settings())).toContain("Endommagé");
    answered = { status: { ...MISSING_MODEL, sttModel: "present" }, failed: false };
    expect(draw(settings())).toContain("Prêt.");
  });

  it("shows the address Rust holds, and none when there is none", () => {
    answered = { status: MISSING_MODEL, failed: false };

    expect(draw(settings({ speechModelsUrl: "https://example.test/models/" }))).toContain(
      "https://example.test/models/",
    );
    expect(draw(settings())).not.toContain("téléchargé que lorsque");
  });

  it("lists what stops each switched-on feature, one line each, and says nothing when voice is off", () => {
    answered = { status: MISSING_MODEL, failed: false };
    expect(draw(settings())).not.toContain("Dictée dans la conversation :");

    answered = {
      status: {
        ...MISSING_MODEL,
        enabled: true,
        features: {
          dictation: { available: false, reason: "speech_engine_missing" },
          readAloud: { available: false, reason: "tts_no_local_voice" },
          notes: { available: true, reason: null },
        },
      },
      failed: false,
    };
    const markup = draw(settings({ enabled: true }));

    expect(markup).toMatch(
      /<div class="field"><span class="field__description">Dictée dans la conversation : Le composant vocal[^<]*<\/span><span class="field__description">Lire les réponses à voix haute : Aucune voix[^<]*<\/span><\/div>/,
    );
  });

  it("keeps the notes switch editable and the voice features disabled while voice is off", () => {
    answered = { status: MISSING_MODEL, failed: false };
    const markup = draw(settings());
    const boxes = [...markup.matchAll(/<input type="checkbox"[^>]*>/g)].map(([box]) => box);

    // Master, dictation, read aloud, notes, voice commands.
    expect(boxes).toHaveLength(5);
    expect(boxes[0]).not.toContain("disabled");
    expect(boxes[1]).toContain("disabled");
    expect(boxes[2]).toContain("disabled");
    expect(boxes[3]).not.toContain("disabled");
  });

  it("offers the enabled languages only, after the interface language", () => {
    answered = { status: MISSING_MODEL, failed: false };
    const markup = draw(settings());

    expect(markup).toContain('<option value="" selected="">La même que l&#x27;interface</option>');
    expect(markup).toContain('<option value="fr-FR">');
    expect(markup).not.toContain('value="de-DE"');
  });

  it("shows the read-aloud choices disabled until the button exists", () => {
    answered = { status: MISSING_MODEL, failed: false };
    const markup = draw(settings());

    expect(markup).toContain("La lecture à voix haute n&#x27;est pas encore disponible");
    expect(markup.match(/<select[^>]*disabled=""/g)).toHaveLength(2);
  });

  it("gives its number fields the bounds Rust keeps", () => {
    answered = { status: MISSING_MODEL, failed: false };
    const markup = draw(settings());

    expect(markup).toMatch(/min="10" max="600"/);
    expect(markup).toMatch(/min="60" max="3600"/);
    expect(markup).toMatch(/min="0.5" max="2"/);
  });

  it("says the same in English", () => {
    answered = { status: MISSING_MODEL, failed: false };

    expect(draw(settings(), "en-US")).toContain("Not installed.");
  });

  it("says so when the state could not be read", () => {
    answered = { status: null, failed: true };

    expect(draw(settings())).toContain("n&#x27;a pas pu être lu");
  });
});
