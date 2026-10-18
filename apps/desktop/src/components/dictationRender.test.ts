import { createElement, type ReactElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { beforeAll, describe, expect, it, vi } from "vitest";
import { I18nProvider } from "../i18n/I18nProvider";
import type { CombinedScope } from "../lib/ipc";
import type { DictationBridge } from "../speech/useDictation";
import { Composer } from "./Composer";
import { DictationStatus } from "./DictationStatus";
import { MicButton } from "./MicButton";

/* The question box with a dictation: the microphone sits between "@" and the model choice, a quiet
   line sits under the text, and with the bridge taken away the box is the markup it always was
   (`voiceOffBaseline.test.ts` pins that markup itself). Nothing is clicked: static markup. */
vi.mock("@tauri-apps/api/core", () => ({ Channel: class {}, invoke: vi.fn() }));

beforeAll(() => {
  Object.assign(globalThis, { document: { documentElement: {} } });
});

function page(element: ReactElement, locale = "fr-FR"): string {
  return renderToStaticMarkup(createElement(I18nProvider, { locale, children: element }));
}

const scope: CombinedScope = { documents: [], workbooks: [] } as unknown as CombinedScope;
const composerProps = {
  draft: "Bonjour",
  onDraftChange: () => {},
  phase: "idle" as const,
  onSend: () => {},
  onStop: () => {},
  hint: "hint",
  modelAlias: "cabinet-chat",
  aliases: ["cabinet-chat", "other"],
  onModelAliasChange: () => {},
  chips: [],
  onAddChip: () => {},
  onRemoveChip: () => {},
  scope,
  hasSelection: true,
  onAddToSelection: () => {},
  onOpenCard: () => {},
};

function bridge(overrides: Partial<DictationBridge> = {}): DictationBridge {
  return {
    phase: "idle",
    error: null,
    result: null,
    caret: null,
    disabled: false,
    reasonKey: null,
    analysing: false,
    level: { get: () => 0, subscribe: () => () => undefined },
    start: () => {},
    stop: () => {},
    cancel: () => {},
    undo: () => {},
    ...overrides,
  };
}

describe("the question box with the microphone", () => {
  it("draws the microphone between the mention button and the model choice", () => {
    const html = page(createElement(Composer, { ...composerProps, dictation: bridge() }));

    const mention = html.indexOf("composer__mention");
    const mic = html.indexOf("composer__mic");
    const select = html.indexOf("composer__model-select");
    expect(mention).toBeGreaterThan(-1);
    expect(mic).toBeGreaterThan(mention);
    expect(select).toBeGreaterThan(mic);
    expect(html).toContain('aria-label="Dicter"');
  });

  it("is the same box as before once the microphone and its line are taken out", () => {
    const without = page(createElement(Composer, composerProps));
    const withMic = page(createElement(Composer, { ...composerProps, dictation: bridge() }));

    const mic = page(createElement(MicButton, { dictation: bridge(), onStart: () => {} }));
    const status = page(createElement(DictationStatus, { dictation: bridge() }));
    expect(withMic.replace(mic, "").replace(status, "")).toBe(without);
  });

  it("draws neither the microphone nor its line without a dictation", () => {
    const html = page(createElement(Composer, composerProps));

    expect(html).not.toContain("composer__mic");
    expect(html).not.toContain("composer__speech");
  });
});

describe("the microphone button", () => {
  const button = (overrides: Partial<DictationBridge>, locale = "fr-FR") =>
    page(createElement(MicButton, { dictation: bridge(overrides), onStart: () => {} }), locale);

  it("is pressable at rest", () => {
    const html = button({});

    expect(html).toContain('aria-pressed="false"');
    expect(html).not.toContain("disabled");
  });

  it("becomes the stop button while recording, with the level as a ring", () => {
    const html = button({ phase: "recording", level: { get: () => 0.1, subscribe: () => () => undefined } });

    expect(html).toContain('aria-label="Terminer la dictée"');
    expect(html).toContain('aria-pressed="true"');
    expect(html).toContain("--mic-level:0.4");
  });

  it("waits while the microphone opens and while the words are worked out", () => {
    expect(button({ phase: "requesting-permission" })).toContain("disabled");
    expect(button({ phase: "transcribing" })).toContain("disabled");
  });

  it("says why it cannot be used, in the language of the interface", () => {
    const html = button({
      phase: "unavailable",
      disabled: true,
      reasonKey: "errors.speech_model_missing",
    });

    expect(html).toContain("disabled");
    expect(html).toContain("title=\"Les fichiers vocaux ne sont pas encore sur cet ordinateur");
  });

  it("speaks English with the English interface", () => {
    expect(button({}, "en-US")).toContain('aria-label="Dictate"');
  });
});

describe("the line under the box", () => {
  const line = (overrides: Partial<DictationBridge>, locale = "en-US") =>
    page(createElement(DictationStatus, { dictation: bridge(overrides) }), locale);

  it("is an empty polite live region at rest", () => {
    expect(line({})).toBe(
      '<div class="composer__speech"><p class="composer__speech-text" role="status" aria-live="polite"></p></div>',
    );
  });

  it("says each step, with the Cancel button while it works", () => {
    expect(line({ phase: "requesting-permission" })).toContain("Waiting for the microphone");
    const recording = line({ phase: "recording" });
    expect(recording).toContain("Press the microphone again to finish");
    expect(recording).toContain("Cancel");
    expect(line({ phase: "transcribing" })).toContain("Transcribing");
  });

  it("offers the Undo and the checks once the words are in", () => {
    const html = line({
      phase: "done",
      result: { insertion: { text: "x", caret: 1, before: { text: "", caret: 0 } }, numbers: 2, clipped: true },
    });

    expect(html).toContain("Text added to your question");
    expect(html).toContain("2 numbers added, check them");
    expect(html).toContain("distorted");
    expect(html).toContain("Undo");
  });

  it("says a single number in the singular", () => {
    const html = line({
      phase: "done",
      result: { insertion: { text: "x", caret: 1, before: { text: "", caret: 0 } }, numbers: 1, clipped: false },
    });

    expect(html).toContain("1 number added, check it");
  });

  it("says the problem and offers to close it", () => {
    const html = line({ phase: "error", error: { key: "errors.speech_audio_too_short", values: {} } });

    expect(html).toContain("The recording is too short");
    expect(html).toContain("Close");
  });

  it("warns that the computer is busy during an analysis, only while it listens or works", () => {
    expect(line({ phase: "recording", analysing: true })).toContain("An analysis is running");
    expect(line({ phase: "idle", analysing: true })).not.toContain("An analysis is running");
  });
});
