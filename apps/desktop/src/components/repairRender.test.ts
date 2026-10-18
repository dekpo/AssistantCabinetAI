import { createElement, type ReactElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { beforeAll, describe, expect, it, vi } from "vitest";
import { I18nProvider } from "../i18n/I18nProvider";
import type { CombinedScope } from "../lib/ipc";
import type { RepairChip } from "../speech/repairChips";
import type { RepairBridge } from "../speech/useRepairSuggestions";
import { Composer } from "./Composer";
import { RepairChips } from "./RepairChips";

/* The name suggestions under the question box, and the box itself with them taken away. Static
   markup: nothing is clicked here (the actions are pure functions, tested in `repairChips.test.ts`). */
vi.mock("@tauri-apps/api/core", () => ({ Channel: class {}, invoke: vi.fn() }));

beforeAll(() => {
  Object.assign(globalThis, { document: { documentElement: {} } });
});

function page(element: ReactElement, locale = "fr-FR"): string {
  return renderToStaticMarkup(createElement(I18nProvider, { locale, children: element }));
}

const DUPONT = { entityId: 7, display: "Jean Dupont" };
const DURAND = { entityId: 8, display: "Marc Durand" };
const DURANT = { entityId: 9, display: "Eric Durant" };

const suggest: RepairChip = {
  id: "1-0",
  start: 5,
  end: 11,
  heard: "Dupond",
  action: "suggest",
  candidates: [DUPONT],
};
const ask: RepairChip = {
  id: "1-1",
  start: 20,
  end: 25,
  heard: "Duran",
  action: "ask",
  candidates: [DURAND, DURANT],
};

function bridge(overrides: Partial<RepairBridge> = {}): RepairBridge {
  return {
    chips: [],
    hiddenCount: 0,
    accept: () => {},
    ignore: () => {},
    reveal: () => {},
    ...overrides,
  };
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
  aliases: ["cabinet-chat"],
  onModelAliasChange: () => {},
  chips: [],
  onAddChip: () => {},
  onRemoveChip: () => {},
  scope,
  hasSelection: true,
  onAddToSelection: () => {},
  onOpenCard: () => {},
};

describe("the suggestions", () => {
  it("draw nothing when there is nothing to say", () => {
    expect(page(createElement(RepairChips, { repair: bridge() }))).toBe("");
  });

  it("show what was heard, the name it may have meant, and the two ways out", () => {
    const html = page(createElement(RepairChips, { repair: bridge({ chips: [suggest] }) }));

    expect(html).toContain("Dupond → Jean Dupont");
    expect(html).toContain("Remplacer");
    expect(html).toContain("Ignorer");
    expect(html).toContain('aria-label="Remplacer Dupond par Jean Dupont"');
    expect(html).toContain('role="group"');
  });

  it("list the alternatives of a question as buttons, in the order given", () => {
    const html = page(createElement(RepairChips, { repair: bridge({ chips: [ask] }) }));

    expect(html).toContain("Duran : quel nom ?");
    expect(html.indexOf("Marc Durand")).toBeGreaterThan(-1);
    expect(html.indexOf("Eric Durant")).toBeGreaterThan(html.indexOf("Marc Durand"));
    expect(html).toContain("Ignorer");
  });

  it("speak English with the English interface", () => {
    const html = page(createElement(RepairChips, { repair: bridge({ chips: [suggest] }) }), "en-US");

    expect(html).toContain("Dupond → Jean Dupont");
    expect(html).toContain("Replace");
    expect(html).toContain("Ignore");
  });

  it("collapse to one line, with no name in it, while names are hidden", () => {
    const html = page(createElement(RepairChips, { repair: bridge({ hiddenCount: 3 }) }));

    expect(html).toContain("3 suggestions de noms sont masquées.");
    expect(html).toContain("Afficher les noms");
    expect(html).not.toContain("repair__list");
    expect(html).not.toContain("Dupont");
  });

  it("say one suggestion in the singular", () => {
    const html = page(createElement(RepairChips, { repair: bridge({ hiddenCount: 1 }) }), "en-US");

    expect(html).toContain("1 name suggestion is hidden.");
  });

  it("escape what was heard, which is text from the engine", () => {
    const hostile: RepairChip = { ...suggest, heard: "<b>x</b>" };

    const html = page(createElement(RepairChips, { repair: bridge({ chips: [hostile] }) }));

    expect(html).not.toContain("<b>x</b>");
    expect(html).toContain("&lt;b&gt;x&lt;/b&gt;");
  });
});

describe("the question box with the suggestions", () => {
  it("is the same box as before when there is no bridge", () => {
    const html = page(createElement(Composer, composerProps));

    expect(html).not.toContain("repair");
  });

  it("is the same box as before when the bridge has nothing to say", () => {
    const without = page(createElement(Composer, composerProps));
    const quiet = page(createElement(Composer, { ...composerProps, repair: bridge() }));

    expect(quiet).toBe(without);
  });

  it("draws the suggestions once, under the text and before the buttons", () => {
    const html = page(
      createElement(Composer, { ...composerProps, repair: bridge({ chips: [suggest] }) }),
    );

    expect(html.match(/class="repair"/g)).toHaveLength(1);
    expect(html.indexOf("composer__input")).toBeLessThan(html.indexOf("repair"));
    expect(html.indexOf("repair")).toBeLessThan(html.indexOf("composer__footer"));
  });
});
