import { createElement, type ReactElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { beforeAll, describe, expect, it, vi } from "vitest";
import { I18nProvider } from "../i18n/I18nProvider";
import type { CombinedScope } from "../lib/ipc";
import { Composer } from "./Composer";
import { MessageList } from "./MessageList";
import { TitleBar } from "./TitleBar";

/* The non-regression baseline of the speech programme (decision D13, written before the first edit
   of phase 2): with the voice master switch off, the composer, the answer action row and the title
   bar must draw exactly this markup. The row of buttons under an answer is not a component of its
   own, it is inline in `MessageList`, so the list is drawn with one finished question and answer
   (idle) and with the answer still being written (busy). Every later speech phase keeps these
   expectations passing without edit while `speech.enabled` is false; the one visible change allowed
   is the Notes button, governed by its own switch. Nothing is clicked: the components are drawn to
   static markup with the real catalogues, as in `render.test.ts`. */
vi.mock("@tauri-apps/api/core", () => ({ Channel: class {}, invoke: vi.fn() }));

beforeAll(() => {
  // The provider sets the document language; there is no document in this environment.
  Object.assign(globalThis, { document: { documentElement: {} } });
});

function page(element: ReactElement): string {
  return renderToStaticMarkup(createElement(I18nProvider, { locale: "fr-FR", children: element }));
}

const EXPECTED = {
  composerIdle: "<div class=\"composer\"><textarea class=\"composer__input\" rows=\"3\" placeholder=\"Écrivez votre question...\" aria-label=\"Écrivez votre question...\" aria-autocomplete=\"list\" aria-expanded=\"false\">Bonjour</textarea><div class=\"composer__footer\"><div class=\"composer__footnotes\">hint</div><div class=\"composer__actions\"><button type=\"button\" class=\"button composer__mention\" aria-label=\"Choisir un nom de ma sélection\" title=\"Choisir un nom de ma sélection\">@</button><select class=\"field__control composer__model-select\" aria-label=\"Assistant\"><option value=\"cabinet-chat\" selected=\"\">cabinet-chat</option><option value=\"other\">other</option></select><button type=\"button\" class=\"button button--primary composer__send\">Envoyer</button></div></div></div>",
  composerBusy: "<div class=\"composer\"><textarea class=\"composer__input\" rows=\"3\" placeholder=\"Écrivez votre question...\" aria-label=\"Écrivez votre question...\" aria-autocomplete=\"list\" aria-expanded=\"false\">Bonjour</textarea><div class=\"composer__footer\"><div class=\"composer__footnotes\">hint</div><div class=\"composer__actions\"><button type=\"button\" class=\"button composer__mention\" aria-label=\"Choisir un nom de ma sélection\" title=\"Choisir un nom de ma sélection\" disabled=\"\">@</button><select class=\"field__control composer__model-select\" aria-label=\"Assistant\"><option value=\"cabinet-chat\" selected=\"\">cabinet-chat</option><option value=\"other\">other</option></select><button type=\"button\" class=\"button button--primary composer__send\">Stop</button></div></div></div>",
  composerEmpty: "<div class=\"composer\"><textarea class=\"composer__input\" rows=\"3\" placeholder=\"Écrivez votre question...\" aria-label=\"Écrivez votre question...\" aria-autocomplete=\"list\" aria-expanded=\"false\"></textarea><div class=\"composer__footer\"><div class=\"composer__footnotes\">hint</div><div class=\"composer__actions\"><button type=\"button\" class=\"button composer__mention\" aria-label=\"Choisir un nom de ma sélection\" title=\"Choisir un nom de ma sélection\">@</button><select class=\"field__control composer__model-select\" aria-label=\"Assistant\"><option value=\"cabinet-chat\" selected=\"\">cabinet-chat</option><option value=\"other\">other</option></select><button type=\"button\" class=\"button button--primary composer__send\" disabled=\"\">Envoyer</button></div></div></div>",
  composerNeedsFolder: "<div class=\"composer\"><textarea class=\"composer__input\" rows=\"3\" disabled=\"\" placeholder=\"Choisissez d&#x27;abord un dossier de documents, dans le panneau de gauche.\" aria-label=\"Choisissez d&#x27;abord un dossier de documents, dans le panneau de gauche.\"></textarea><div class=\"composer__footer\"><div class=\"composer__footnotes\">hint</div><div class=\"composer__actions\"><button type=\"button\" class=\"button composer__mention\" aria-label=\"Choisir un nom de ma sélection\" title=\"Choisir un nom de ma sélection\" disabled=\"\">@</button><select class=\"field__control composer__model-select\" aria-label=\"Assistant\"><option value=\"cabinet-chat\" selected=\"\">cabinet-chat</option><option value=\"other\">other</option></select><button type=\"button\" class=\"button button--primary composer__send\" disabled=\"\">Envoyer</button></div></div></div>",
  listIdle: "<div class=\"messages-pane\"><div class=\"messages\"><article class=\"message message--user\"><p class=\"message__timing message__timing--asked\">18-10-2026 09:05:03</p><div class=\"message__bubble\"><p class=\"message__body message__body--clamped\">Question ?</p></div><p class=\"message__actions message__actions--turn\"><button type=\"button\" class=\"button button--compact\">Copier</button><button type=\"button\" class=\"button button--compact\">Modifier</button></p></article><article class=\"message message--assistant\"><p class=\"message__author\">Assistant</p><div class=\"message__body message__body--markdown\"><p>Reponse.</p></div><p class=\"message__actions message__actions--turn\"><button type=\"button\" class=\"button button--compact\">Copier</button><button type=\"button\" class=\"button button--compact\">Régénérer</button></p></article><div></div></div></div>",
  listBusy: "<div class=\"messages-pane\"><div class=\"messages\"><article class=\"message message--user\"><p class=\"message__timing message__timing--asked\">18-10-2026 09:05:03</p><div class=\"message__bubble\"><p class=\"message__body message__body--clamped\">Question ?</p></div></article><article class=\"message message--assistant\"><p class=\"message__author\">Assistant</p><div class=\"message__body message__body--markdown\"><p>Reponse.</p></div><p class=\"message__actions\"><button type=\"button\" class=\"button button--compact message__stop\">Stop</button></p></article><div></div></div></div>",
  titleBar: "<header class=\"titlebar\"><div><p class=\"titlebar__name\">Assistant Cabinet AI</p><p class=\"titlebar__subtitle\">Aide administrative sur votre ordinateur. Vérifiez les réponses : la médecin reste seule décideuse.</p></div><div class=\"titlebar__actions\"><button type=\"button\" class=\"status\" title=\"Vérifier l&#x27;IA\" aria-label=\"Vérifier l&#x27;IA\"><span class=\"status__dot status__dot--online\" aria-hidden=\"true\"></span><span class=\"status__value\">IA Disponible</span></button><button type=\"button\" class=\"button button--icon\" aria-pressed=\"true\" aria-label=\"Espace de travail\" title=\"Espace de travail\"><svg class=\"glyph\" viewBox=\"0 0 20 20\" aria-hidden=\"true\"><rect x=\"2.5\" y=\"3.5\" width=\"15\" height=\"13\" rx=\"1.5\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"1.4\" stroke-linejoin=\"round\"></rect><path d=\"M7.5 3.75 V16.25 M12.5 3.75 V16.25\" fill=\"none\" stroke=\"currentColor\" stroke-width=\"1.4\"></path></svg></button><button type=\"button\" class=\"button\">Réglages</button></div></header>",
};

const scope: CombinedScope = { documents: [], workbooks: [] } as unknown as CombinedScope;

const composerProps = {
  draft: "",
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

describe("the composer with the voice switch off", () => {
  it("is drawn as before when idle with a draft", () => {
    expect(page(createElement(Composer, { ...composerProps, draft: "Bonjour" }))).toBe(EXPECTED.composerIdle);
  });

  it("offers Stop and locks the mention button while a question is busy", () => {
    expect(
      page(createElement(Composer, { ...composerProps, draft: "Bonjour", phase: "thinking" })),
    ).toBe(EXPECTED.composerBusy);
  });

  it("disables Send on an empty draft", () => {
    expect(page(createElement(Composer, composerProps))).toBe(EXPECTED.composerEmpty);
  });

  it("is locked, with its own sentence, until a documents folder is chosen", () => {
    expect(page(createElement(Composer, { ...composerProps, needsFolder: true }))).toBe(
      EXPECTED.composerNeedsFolder,
    );
  });
});

describe("the conversation with the voice switch off", () => {
  // Local wall-clock parts, so the timestamp reads the same in every time zone.
  const createdAt = new Date(2026, 9, 18, 9, 5, 3).getTime();
  const entries = [
    { id: "q1", role: "user" as const, content: "Question ?", createdAt },
    { id: "a1", role: "assistant" as const, content: "Reponse.", createdAt },
  ];
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

  it("keeps Copy and Regenerate as the only buttons under a finished answer", () => {
    expect(page(createElement(MessageList, listProps))).toBe(EXPECTED.listIdle);
  });

  it("keeps Stop as the only button under an answer being written", () => {
    expect(
      page(createElement(MessageList, { ...listProps, phase: "writing", streamingId: "a1" })),
    ).toBe(EXPECTED.listBusy);
  });
});

describe("the title bar with the notes button absent", () => {
  it("shows the status, the workspace toggle and the settings button, and nothing else", () => {
    const html = page(
      createElement(TitleBar, {
        connection: "online",
        onRefresh: () => {},
        onOpenSettings: () => {},
        workspaceOpen: true,
        onToggleWorkspace: () => {},
      }),
    );

    expect(html).toBe(EXPECTED.titleBar);
  });
});
