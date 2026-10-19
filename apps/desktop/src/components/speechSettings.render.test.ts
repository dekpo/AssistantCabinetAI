import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { beforeAll, describe, expect, it, vi } from "vitest";
import { I18nProvider } from "../i18n/I18nProvider";
import { DEFAULT_SPEECH_SETTINGS, type AppSettings } from "../lib/ipc";
import type { SpeechStatus } from "../speech/ipc";
import type { ModelJob } from "../speech/modelJob";
import { SpeechSettingsBlock } from "./SpeechSettings";

vi.mock("@tauri-apps/api/core", () => ({ Channel: class {}, invoke: vi.fn() }));

let answered: { status: SpeechStatus | null; failed: boolean } = { status: null, failed: false };
vi.mock("../state/useSpeechStatus", () => ({ useSpeechStatus: () => answered }));

let modelJob: ModelJob = { kind: "idle" };
/* The voice the running job is about; null is the dictation model. */
let modelTarget: string | null = null;
vi.mock("../state/useModelJob", () => ({
  useModelJob: () => ({
    job: modelJob,
    target: modelTarget,
    install: () => {},
    verify: () => {},
    cancel: () => {},
  }),
}));

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
  voices: [],
  languages: [
    { id: "fr-FR", support: "enabled", dictation: true, readAloud: true, voices: [] },
    { id: "de-DE", support: "planned", dictation: false, readAloud: false, voices: [] },
  ],
};

const WITH_VOICES: SpeechStatus = {
  ...MISSING_MODEL,
  enabled: true,
  languages: [
    {
      id: "fr-FR",
      support: "enabled",
      dictation: true,
      readAloud: true,
      voices: [
        { gender: "female", id: "siwis" },
        { gender: "male", id: "css10" },
      ],
    },
  ],
  voices: [
    { id: "siwis", state: "present", rating: "review", bytes: 67_428_923 },
    { id: "css10", state: "missing", rating: "attribution", bytes: 65_346_319 },
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

  it("says the model is missing, offers Install and keeps Verify off", () => {
    answered = { status: MISSING_MODEL, failed: false };
    modelJob = { kind: "idle" };
    const markup = draw(settings());

    expect(markup).toContain("Non installé.");
    expect(markup).toMatch(/<button(?![^>]*disabled)[^>]*>Installer<\/button>/);
    expect(markup).toMatch(/<button[^>]*disabled=""[^>]*>Vérifier<\/button>/);
    expect(markup).not.toContain("installation du modèle");
    expect(markup).not.toContain("Annuler");
  });

  it("offers Verify, not Install, for a model placed by hand, and neither for a ready one", () => {
    modelJob = { kind: "idle" };
    answered = { status: { ...MISSING_MODEL, sttModel: "unverified" }, failed: false };
    let markup = draw(settings());
    expect(markup).toContain("Fichiers trouvés, pas encore vérifiés");
    expect(markup).toMatch(/<button[^>]*disabled=""[^>]*>Installer<\/button>/);
    expect(markup).toMatch(/<button(?![^>]*disabled)[^>]*>Vérifier<\/button>/);

    answered = { status: { ...MISSING_MODEL, sttModel: "corrupt" }, failed: false };
    markup = draw(settings());
    expect(markup).toMatch(/<button(?![^>]*disabled)[^>]*>Installer<\/button>/);

    answered = { status: { ...MISSING_MODEL, sttModel: "present" }, failed: false };
    markup = draw(settings());
    expect(markup).toMatch(/<button[^>]*disabled=""[^>]*>Installer<\/button>/);
  });

  it("shows the progress of a running install with a Cancel button, and nothing else to press", () => {
    answered = { status: MISSING_MODEL, failed: false };
    modelJob = { kind: "installing", phase: "download", percent: 42 };
    const markup = draw(settings());

    expect(markup).toContain("Téléchargement : 42 %");
    expect(markup).toContain(">Annuler</button>");
    expect(markup).toMatch(/<button[^>]*disabled=""[^>]*>Installer<\/button>/);
    expect(markup).toMatch(/<button[^>]*disabled=""[^>]*>Vérifier<\/button>/);

    modelJob = { kind: "verifying" };
    expect(draw(settings())).toContain("Vérification des fichiers");
    modelJob = { kind: "idle" };
  });

  it("says why a download failed, with the folder for the copy by hand", () => {
    answered = { status: MISSING_MODEL, failed: false };
    modelJob = {
      kind: "failed",
      error: { code: "speech_model_download_failed", data: { folder: "C:/data/speech-models/m" } },
    };
    const markup = draw(settings());

    expect(markup).toContain("C:/data/speech-models/m");
    expect(markup).toContain("copiez les fichiers à la main");
    modelJob = { kind: "idle" };
  });

  it("lists the voices of the language with their state, their size and their licence", () => {
    modelJob = { kind: "idle" };
    modelTarget = null;
    answered = { status: WITH_VOICES, failed: false };
    const markup = draw(settings());

    expect(markup).toContain("Voix installées sur cet ordinateur");
    expect(markup).toContain("Voix féminine (siwis), 67 Mo");
    expect(markup).toContain("Voix masculine (css10), 65 Mo");
    expect(markup).toContain("Prêt.");
    expect(markup).toContain("Non installé.");
    // The licence is never hidden: the prototype voice says so, the other asks for a credit.
    expect(markup).toContain("Voix de prototype : sa licence doit être vérifiée");
    expect(markup).toContain("Libre d&#x27;usage, avec mention de ses auteurs.");
    // The reading list carries the rating too.
    expect(markup).toContain("Voix féminine (siwis, prototype)");
    expect(markup).toContain("Voix masculine (css10, mention des auteurs requise)");
    // One Install per voice, usable only for a voice that is not there.
    expect(markup.match(/>Installer<\/button>/g)).toHaveLength(3);
  });

  it("shows the progress of a voice under that voice, and not under the dictation model", () => {
    answered = { status: WITH_VOICES, failed: false };
    modelJob = { kind: "installing", phase: "download", percent: 17 };
    modelTarget = "css10";
    const markup = draw(settings());

    expect(markup.match(/Téléchargement : 17 %/g)).toHaveLength(1);
    expect(markup.match(/>Annuler<\/button>/g)).toHaveLength(1);
    const afterCss10 = markup.slice(markup.indexOf("css10), 65 Mo"));
    expect(afterCss10).toContain("Téléchargement : 17 %");

    modelJob = { kind: "idle" };
    modelTarget = null;
  });

  it("says why the install of a voice failed under that voice only", () => {
    answered = { status: WITH_VOICES, failed: false };
    modelJob = {
      kind: "failed",
      error: { code: "speech_model_download_failed", data: { folder: "C:/data/speech-models/css10" } },
    };
    modelTarget = "css10";
    const markup = draw(settings());

    expect(markup.match(/C:\/data\/speech-models\/css10/g)).toHaveLength(1);
    modelJob = { kind: "idle" };
    modelTarget = null;
  });

  it("lets the user choose among the system voices, not the manifest voices, when the engine is the system's", () => {
    answered = { status: WITH_VOICES, failed: false };

    const piper = draw(settings({ ttsEngine: "piper" }));
    expect(piper).toContain("Voix de lecture");
    expect(piper).not.toContain("Voix du système</span>");

    const system = draw(
      settings({ ttsEngine: "system", ttsSystemVoice: "Microsoft Paul - French (France)" }),
    );
    expect(system).toContain("Voix du système</span>");
    // A voice chosen earlier stays in the list even before the system has named its voices.
    expect(system).toContain(
      '<option value="Microsoft Paul - French (France)" selected="">Microsoft Paul - French (France)</option>',
    );
    expect(system).not.toContain("<span class=\"field__label\">Voix de lecture</span>");
  });

  it("says a language has no reading voice when the table offers none", () => {
    answered = { status: MISSING_MODEL, failed: false };

    expect(draw(settings())).toContain("Aucune voix de lecture n&#x27;est proposée pour cette langue.");
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

  it("offers the read-aloud choices now that the button exists", () => {
    answered = { status: MISSING_MODEL, failed: false };
    const markup = draw(settings());

    expect(markup).not.toContain("pas encore disponible dans cette version");
    expect(markup.match(/<select[^>]*disabled=""/g)).toBeNull();
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
