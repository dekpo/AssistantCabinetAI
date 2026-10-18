import { createElement, type ReactElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { beforeAll, describe, expect, it, vi } from "vitest";
import { I18nProvider } from "../i18n/I18nProvider";
import { ReadAloudContext, type ReadAloudValue } from "../speech/output/ReadAloudContext";
import type { ReadAloud, ReadAloudState } from "../speech/output/readAloud";
import type { ChatEntry } from "../state/useChat";
import { MessageList } from "./MessageList";
import { ReadAloudButton } from "./ReadAloudButton";

vi.mock("@tauri-apps/api/core", () => ({ Channel: class {}, invoke: vi.fn() }));

beforeAll(() => {
  Object.assign(globalThis, { document: { documentElement: {} } });
});

const IDLE: ReadAloudState = { entryId: null, phase: "idle", progress: null, problem: null };
const controller = {} as ReadAloud;

function withReading(element: ReactElement, value: ReadAloudValue | null, locale = "fr-FR"): string {
  const inside = value === null ? element : createElement(ReadAloudContext.Provider, { value }, element);
  return renderToStaticMarkup(createElement(I18nProvider, { locale, children: inside }));
}

const createdAt = new Date(2026, 9, 18, 9, 5, 3).getTime();
const answer: ChatEntry = { id: "a1", role: "assistant", content: "Reponse.", createdAt };
const entries: ChatEntry[] = [{ id: "q1", role: "user", content: "Question ?", createdAt }, answer];
const listProps = {
  entries,
  phase: "idle" as const,
  streamingId: null,
  onStop: () => {},
  onResend: () => {},
  onRegenerate: () => {},
  onAnalyse: () => {},
  analysing: false,
  onRetarget: () => {},
  showDiagnostics: false,
  copySources: false,
};

describe("the read-aloud button", () => {
  it("draws nothing outside the provider and nothing while it is not visible", () => {
    expect(withReading(createElement(ReadAloudButton, { entry: answer }), null)).toBe("");
    expect(
      withReading(createElement(ReadAloudButton, { entry: answer }), {
        controller,
        state: IDLE,
        visible: false,
      }),
    ).toBe("");
  });

  it("offers to read when visible, with the words of the language", () => {
    const html = withReading(createElement(ReadAloudButton, { entry: answer }), {
      controller,
      state: IDLE,
      visible: true,
    });

    expect(html).toContain("Lire à voix haute");
    expect(html).toContain('aria-pressed="false"');
    expect(html).not.toContain("glyph--speaking");
    expect(
      withReading(
        createElement(ReadAloudButton, { entry: answer }),
        { controller, state: IDLE, visible: true },
        "en-US",
      ),
    ).toContain("Read aloud");
  });

  it("offers to stop, with the speaking indicator, on the answer being read only", () => {
    const speaking: ReadAloudState = {
      entryId: "a1",
      phase: "speaking",
      progress: { done: 1, total: 3 },
      problem: null,
    };
    const mine = withReading(createElement(ReadAloudButton, { entry: answer }), {
      controller,
      state: speaking,
      visible: true,
    });
    const other = withReading(
      createElement(ReadAloudButton, { entry: { ...answer, id: "a2" } }),
      { controller, state: speaking, visible: true },
    );

    expect(mine).toContain("Arrêter la lecture");
    expect(mine).toContain('aria-pressed="true"');
    expect(mine).toContain("glyph--speaking");
    expect(other).toContain("Lire à voix haute");
    expect(other).not.toContain("glyph--speaking");
  });

  it("says why nothing is read, beside the answer it concerns and nowhere else", () => {
    const refused: ReadAloudState = {
      ...IDLE,
      problem: { entryId: "a1", key: "errors.tts_no_local_voice", values: { locale: "fr-FR" } },
    };
    const value = { controller, state: refused, visible: true };

    const mine = withReading(createElement(ReadAloudButton, { entry: answer }), value);
    const other = withReading(
      createElement(ReadAloudButton, { entry: { ...answer, id: "a2" } }),
      value,
    );

    expect(mine).toContain('role="alert"');
    expect(mine).toContain("Aucune voix fonctionnant uniquement sur cet ordinateur");
    expect(other).not.toContain('role="alert"');
  });
});

describe("the conversation with the voice switch off", () => {
  it("draws exactly the same markup inside a provider that is not visible as without one", () => {
    const without = withReading(createElement(MessageList, listProps), null);
    const hidden = withReading(createElement(MessageList, listProps), {
      controller,
      state: IDLE,
      visible: false,
    });

    expect(hidden).toBe(without);
    expect(without).not.toContain("message__read");
  });

  it("adds the button after Copy and Regenerate, in the same row, when it is visible", () => {
    const html = withReading(createElement(MessageList, listProps), {
      controller,
      state: IDLE,
      visible: true,
    });

    const row = html.slice(html.lastIndexOf("message__actions message__actions--turn"));
    expect(row.indexOf("Copier")).toBeLessThan(row.indexOf("Régénérer"));
    expect(row.indexOf("Régénérer")).toBeLessThan(row.indexOf("Lire à voix haute"));
  });

  it("draws no button under an answer being written", () => {
    const html = withReading(
      createElement(MessageList, { ...listProps, phase: "writing", streamingId: "a1" }),
      { controller, state: IDLE, visible: true },
    );

    expect(html).not.toContain("message__read");
  });

  it("draws no button under an answer that asks for the analysis", () => {
    const needsIndexing: ChatEntry = { ...answer, needsIndexing: true };
    const html = withReading(
      createElement(MessageList, { ...listProps, entries: [entries[0] as ChatEntry, needsIndexing] }),
      { controller, state: IDLE, visible: true },
    );

    expect(html).not.toContain("message__read");
  });
});
