import { describe, expect, it } from "vitest";
import { CATALOGUES, REFERENCE_LOCALE } from "../../i18n/catalogues";
import { createTranslator, type Catalogue } from "../../i18n/translate";
import type { TabularAnswer } from "../../lib/ipc";
import type { ChatEntry } from "../../state/useChat";
import { holdsMarkdown, readingParts } from "./readingParts";

const reference = CATALOGUES[REFERENCE_LOCALE] as Catalogue;
const french = createTranslator(CATALOGUES["fr-FR"] as Catalogue, reference);

const total: TabularAnswer = {
  kind: "value",
  file: "factures.xlsx",
  value: { kind: "sum", value: { value: 1450, unit: null, unparsed: 0 } },
  locator: {
    sheet: "Factures",
    headerRow: 0,
    column: "montant",
    rowRange: [0, 7],
    filters: [{ kind: "equals", column: "fournisseur", value: "MedSupply" }],
  },
  derivation: { kind: "computed", operation: "sum", row_count: 3 },
};

function entry(extra: Partial<ChatEntry>): ChatEntry {
  return { id: "a", role: "assistant", content: "Le total est **1 450 euros**.", createdAt: 0, ...extra };
}

const texts = (parts: ReturnType<typeof readingParts>) => parts.map((part) => part.text);

describe("what is read of an answer", () => {
  it("is the answer as it is shown, sent as Markdown and not cleaned here", () => {
    expect(readingParts(french, "fr-FR", entry({}))).toStrictEqual([
      { kind: "markdown", text: "Le total est **1 450 euros**." },
    ]);
  });

  it("reads the engine's figure after the answer, and the caution notes after that, one line each", () => {
    const parts = readingParts(
      french,
      "fr-FR",
      entry({
        mixed: true,
        mixedTable: total,
        withoutDocuments: true,
        unverifiedNumbers: ["80"],
        mixedCorrections: [{ claimed: "1 480", correct: 1450 }],
      }),
    );

    expect(parts[0]?.text).toContain("Le total est");
    expect(parts[1]?.text).toContain(french("chat.mixedComputedTitle"));
    expect(parts.slice(2).map((part) => part.kind)).toStrictEqual(["plain", "plain", "plain"]);
    expect(texts(parts).slice(2)).toStrictEqual([
      french("chat.withoutDocuments"),
      french("chat.mixedCorrection", { claimed: "1 480", correct: new Intl.NumberFormat("fr-FR").format(1450) }),
      french("chat.unverifiedNumber", { number: "80" }),
    ]);
  });

  it("includes the caution notes but not the timings, the model, the sources or the coverage", () => {
    const loud = entry({
      durationMs: 44000,
      modelAlias: "gemma2:2b",
      deterministic: true,
      documentsNotNeeded: true,
      modelAttemptAlias: "ministral",
      modelAttemptDurationMs: 3000,
      unanalysedFiles: 2,
      interrupted: "stopped",
    });

    const read = texts(readingParts(french, "fr-FR", loud)).join("\n");

    expect(read).toContain("document");
    expect(read).toContain(french("chat.interrupted"));
    expect(read).not.toContain("gemma2:2b");
    expect(read).not.toContain("ministral");
    expect(read).not.toContain(french("chat.deterministic"));
    expect(read).not.toContain(french("chat.documentsNotNeeded"));
  });

  it("reads which side of a mixed question could not be established, not the variant that says it was not asked", () => {
    const established = texts(
      readingParts(
        french,
        "fr-FR",
        entry({ mixedTableUnavailable: "no_evidence", mixedDocumentsUnavailable: "gateway_unavailable" }),
      ),
    );
    const notAsked = texts(
      readingParts(
        french,
        "fr-FR",
        entry({ mixedTableUnavailable: "not_asked_about", mixedDocumentsUnavailable: "not_asked_about" }),
      ),
    );

    expect(established).toContain(french("chat.mixedTableUnavailable.no_evidence"));
    expect(established).toContain(french("chat.mixedDocumentsUnavailable.gateway_unavailable"));
    expect(notAsked).toStrictEqual(["Le total est **1 450 euros**."]);
  });

  it("reads a rejected citation and an answer that stopped or timed out", () => {
    const read = texts(
      readingParts(
        french,
        "fr-FR",
        entry({ mixedRejectedCitations: ["[3]"], interrupted: "timedOut" }),
      ),
    );

    expect(read).toContain(french("chat.timedOut"));
    expect(read).toContain(french("chat.mixedRejectedCitation", { citation: "[3]" }));
  });

  it("gives no part for an empty answer without notes", () => {
    expect(readingParts(french, "fr-FR", entry({ content: "   " }))).toStrictEqual([]);
  });

  it("sends a table as Markdown and a plain sentence as plain, so an underscore in a name stays", () => {
    expect(holdsMarkdown("| a | b |\n| - | - |\n| 1 | 2 |")).toBe(true);
    expect(holdsMarkdown("- un\n- deux")).toBe(true);
    expect(holdsMarkdown("Total : 1450 euros")).toBe(false);
    expect(holdsMarkdown("Le fichier facture_2026_10.xlsx")).toBe(false);
  });
});
