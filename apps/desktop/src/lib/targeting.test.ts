import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";
import { CATALOGUES, REFERENCE_LOCALE } from "../i18n/catalogues";
import { createTranslator, type Catalogue } from "../i18n/translate";
import type { TargetingCandidate, TargetingReport } from "./ipc";
import {
  carryOverIds,
  confirmedNames,
  FOLLOW_UP_MAX_WORDS,
  isShortFollowUp,
  reductionApplied,
  targetingNote,
  targetingNoteText,
  targetingOffers,
  targetingRequestFor,
} from "./targeting";

const reference = CATALOGUES[REFERENCE_LOCALE] as Catalogue;
const english = createTranslator(reference, reference);
const french = createTranslator(CATALOGUES["fr-FR"] as Catalogue, reference);

function report(overrides: Partial<TargetingReport> = {}): TargetingReport {
  return {
    strategy: "none",
    reason: null,
    requestedBy: "auto",
    widened: "none",
    selectedDocuments: 40,
    candidateDocuments: 40,
    selectedWorkbooks: 0,
    candidateWorkbooks: 0,
    reductionPercent: 0,
    entities: [],
    candidatePaths: [],
    workbookPaths: [],
    ambiguities: [],
    possibles: [],
    chosenWorkbook: null,
    ...overrides,
  };
}

const dupont = {
  entityId: 7,
  display: "Jean Dupont",
  typeId: "person",
  confidence: 0.9,
  method: "canonical_name",
};

function applied(count = 7, overrides: Partial<TargetingReport> = {}): TargetingReport {
  return report({
    strategy: "entity_files",
    candidateDocuments: count,
    reductionPercent: Math.floor(((40 - count) * 100) / 40),
    entities: [dupont],
    candidatePaths: Array.from({ length: count }, (_, index) => `doc-${index}.txt`),
    ...overrides,
  });
}

function candidate(
  entityId: number,
  display: string,
  documents = 1,
  workbooks = 0,
  typeId = "person",
): TargetingCandidate {
  return { entityId, display, typeId, documents, workbooks };
}

describe("the line under an answer whose search was narrowed", () => {
  it("says how many documents were searched out of how many, and for which name", () => {
    expect(targetingNote(french, applied())?.lines[0]?.text).toBe(
      "Recherche ciblée : 7 documents sur 40 — Jean Dupont",
    );
    expect(targetingNote(english, applied())?.lines[0]?.text).toBe(
      "Targeted search: 7 documents out of 40 — Jean Dupont",
    );
  });

  it("uses the singular for one document", () => {
    expect(targetingNote(french, applied(1))?.lines[0]?.text).toBe(
      "Recherche ciblée : 1 document sur 40 — Jean Dupont",
    );
  });

  it("offers to widen and to see the files", () => {
    const note = targetingNote(french, applied(3));

    expect(note?.lines[0]?.action?.label).toBe("Élargir à ma sélection");
    expect(note?.files).toStrictEqual(["doc-0.txt", "doc-1.txt", "doc-2.txt"]);
  });

  it("tells a narrowing the user asked for with a name in its own words", () => {
    const note = targetingNote(
      french,
      applied(2, { requestedBy: "chip", strategy: "chip_files" }),
    );

    expect(note?.lines[0]).toMatchObject({
      kind: "chip",
      text: "Recherche ciblée à votre demande : 2 documents sur 40 — Jean Dupont",
    });
    expect(note?.lines[0]?.action).not.toBeNull();
  });

  it("says the search was widened by itself, with nothing left to do", () => {
    const note = targetingNote(french, applied(3, { widened: "auto" }));

    expect(note?.lines).toHaveLength(1);
    expect(note?.lines[0]).toMatchObject({
      kind: "widened",
      text: "Recherche élargie à toute votre sélection : peu de résultats dans les documents ciblés.",
      action: null,
    });
    // The files it tried first can still be listed.
    expect(note?.files).toHaveLength(3);
  });

  it("names the workbook the base chose, and offers another", () => {
    const note = targetingNote(
      french,
      report({
        strategy: "entity_files",
        selectedWorkbooks: 3,
        candidateWorkbooks: 1,
        entities: [dupont],
        workbookPaths: ["factures-2026.xlsx"],
        chosenWorkbook: { file: "factures-2026.xlsx", reason: "entity_files" },
      }),
    );

    expect(note?.lines).toHaveLength(1);
    expect(note?.lines[0]).toMatchObject({
      kind: "workbook",
      text: "Classeur choisi : factures-2026.xlsx — Jean Dupont y figure",
    });
    expect(note?.lines[0]?.action?.label).toBe("Choisir un autre classeur");
    expect(targetingNote(english, report({
      selectedWorkbooks: 3,
      candidateWorkbooks: 1,
      entities: [dupont],
      workbookPaths: ["factures-2026.xlsx"],
      chosenWorkbook: { file: "factures-2026.xlsx", reason: "entity_files" },
    }))?.lines[0]?.text).toBe("Workbook chosen: factures-2026.xlsx — Jean Dupont appears in it");
  });

  it("counts workbooks as well when several are left, and joins the two counts", () => {
    const note = targetingNote(
      french,
      applied(7, {
        selectedWorkbooks: 5,
        candidateWorkbooks: 2,
        workbookPaths: ["a.xlsx", "b.xlsx"],
      }),
    );

    expect(note?.lines[0]?.text).toBe(
      "Recherche ciblée : 7 documents sur 40 et 2 classeurs sur 5 — Jean Dupont",
    );
    expect(note?.files).toHaveLength(9);
  });

  it("says nothing for a narrowing that was only measured or skipped", () => {
    // `suggest`: the report exists only because names need a question, and the old search ran.
    const shadow = report({
      strategy: "entity_files",
      candidateDocuments: 40,
      ambiguities: [{ span: "Martin", candidates: [candidate(1, "A Martin"), candidate(2, "B Martin")] }],
    });
    const skipped = report({ reason: "no_entity" });

    expect(reductionApplied(shadow)).toBe(false);
    expect(targetingNote(french, shadow)).toBeNull();
    expect(targetingNote(french, skipped)).toBeNull();
    expect(targetingNote(french, undefined)).toBeNull();
  });

  it("keeps a candidate out of the names, and says no names when there are none", () => {
    const phrase = { ...dupont, entityId: 9, display: "Conseil de l'Ordre", confidence: 0.4 };
    const note = targetingNote(french, applied(2, { entities: [phrase] }));

    expect(confirmedNames(applied(2, { entities: [dupont, phrase, dupont] }))).toStrictEqual([
      "Jean Dupont",
    ]);
    expect(note?.lines[0]?.text).toBe("Recherche ciblée : 2 documents sur 40");
  });

  it("quotes at most three names", () => {
    const names = ["Ada", "Bea", "Cleo", "Dina"].map((display, index) => ({
      ...dupont,
      entityId: index + 1,
      display,
    }));

    expect(targetingNote(french, applied(2, { entities: names }))?.lines[0]?.text).toBe(
      "Recherche ciblée : 2 documents sur 40 — Ada, Bea, Cleo",
    );
  });

  it("gives the copied conversation the same sentences", () => {
    expect(targetingNoteText(french, applied(7))).toStrictEqual([
      "Recherche ciblée : 7 documents sur 40 — Jean Dupont",
    ]);
    expect(targetingNoteText(french, report())).toStrictEqual([]);
  });
});

describe("what is sent with a question", () => {
  const SHORT = "Quand reçoit-il ?";

  it("is nothing for an ordinary question after an answer that named nobody", () => {
    expect(targetingRequestFor(undefined, undefined, SHORT)).toBeUndefined();
    expect(targetingRequestFor(undefined, report(), SHORT)).toBeUndefined();
  });

  it("is the old search for the widen action, with no name carried", () => {
    expect(targetingRequestFor({ mode: "off" }, applied(), SHORT)).toStrictEqual({ mode: "off" });
  });

  it("sends the names she picked", () => {
    expect(targetingRequestFor({ entityIds: [4, 5] }, undefined, SHORT)).toStrictEqual({
      entityIds: [4, 5],
    });
  });

  it("carries the previous answer's confirmed names into a short follow-up", () => {
    const phrase = { ...dupont, entityId: 9, confidence: 0.4 };
    const previous = applied(2, { entities: [dupont, phrase] });

    expect(carryOverIds(previous)).toStrictEqual([7]);
    expect(targetingRequestFor(undefined, previous, SHORT)).toStrictEqual({ carryOver: [7] });
    expect(targetingRequestFor({ entityIds: [4] }, previous, SHORT)).toStrictEqual({
      entityIds: [4],
      carryOver: [7],
    });
  });

  it("carries nothing into a question that stands on its own", () => {
    const previous = applied(2);
    const long = "Que dit le Docteur Dupond du bilan sanguin de la semaine derniere ?";

    expect(isShortFollowUp(long)).toBe(false);
    expect(targetingRequestFor(undefined, previous, long)).toBeUndefined();
    // The names she picked travel whatever the length.
    expect(targetingRequestFor({ entityIds: [4] }, previous, long)).toStrictEqual({
      entityIds: [4],
    });
  });

  it("counts words the way Rust does: separated by whitespace, punctuation included", () => {
    expect(isShortFollowUp("Que dit le Docteur Dupond du bilan ?")).toBe(true); // 8
    expect(isShortFollowUp("Que dit le Docteur Dupond du bilan sanguin ?")).toBe(false); // 9
    expect(isShortFollowUp("  Et   son  adresse ?  ")).toBe(true);
    expect(isShortFollowUp("")).toBe(true);
  });

  it("uses the same limit as the Rust conversation module, read from its source", () => {
    const source = readFileSync(
      resolve(fileURLToPath(new URL(".", import.meta.url)), "../../src-tauri/src/conversation.rs"),
      "utf8",
    );

    expect(source).toContain(`pub const FOLLOW_UP_MAX_WORDS: usize = ${FOLLOW_UP_MAX_WORDS};`);
  });

  it("carries nothing out of an answer that has no report", () => {
    expect(carryOverIds(undefined)).toStrictEqual([]);
  });
});

describe("the question the answer asks back", () => {
  const martins = report({
    ambiguities: [
      {
        span: "Dr Martin",
        candidates: [candidate(1, "Paul Martin", 2), candidate(2, "Pierre Martin", 1)],
      },
    ],
  });

  it("asks which person was meant, with the number of files of each", () => {
    const offers = targetingOffers(french, martins);

    expect(offers).toHaveLength(1);
    expect(offers[0]?.text).toBe("Plusieurs personnes correspondent à « Dr Martin » :");
    expect(offers[0]?.options).toStrictEqual([
      { entityId: 1, label: "Paul Martin (2 fichiers)" },
      { entityId: 2, label: "Pierre Martin (1 fichier)" },
    ]);
    expect(targetingOffers(english, martins)[0]?.text).toBe(
      "Several people match “Dr Martin”:",
    );
  });

  it("names the kind of thing when they are all organisations or places", () => {
    const orgs = report({
      ambiguities: [
        {
          span: "Caisse",
          candidates: [
            candidate(1, "Caisse Nord", 1, 0, "organization"),
            candidate(2, "Caisse Sud", 1, 0, "organization"),
          ],
        },
      ],
    });
    const mixed = report({
      ambiguities: [
        {
          span: "Rhone",
          candidates: [
            candidate(1, "Cpam du Rhone", 1, 0, "organization"),
            candidate(2, "Rhone", 1, 0, "location"),
          ],
        },
      ],
    });

    expect(targetingOffers(french, orgs)[0]?.text).toContain("Plusieurs organisations");
    expect(targetingOffers(french, mixed)[0]?.text).toContain("Plusieurs noms");
  });

  it("falls back to a sentence without a name when the words are not known", () => {
    const unnamed = report({
      ambiguities: [
        { span: "  ", candidates: [candidate(1, "A Martin"), candidate(2, "B Martin")] },
      ],
    });

    expect(targetingOffers(french, unnamed)[0]?.text).toBe(
      "Plusieurs noms correspondent à votre question :",
    );
  });

  it("offers a did-you-mean for a possible match, one button per name", () => {
    const offers = targetingOffers(
      french,
      report({
        possibles: [
          {
            span: "Dupond",
            candidates: [candidate(7, "Jean Dupont", 2)],
            reason: "phonetic",
            score: 0.9,
          },
        ],
      }),
    );

    expect(offers[0]).toMatchObject({
      kind: "possible",
      text: "Vouliez-vous dire :",
      options: [{ entityId: 7, label: "Jean Dupont (2 fichiers)" }],
    });
  });

  it("counts a workbook as a file", () => {
    const offers = targetingOffers(
      french,
      report({
        possibles: [
          { span: "Dupond", candidates: [candidate(7, "Jean Dupont", 1, 2)], reason: "phonetic", score: 1 },
        ],
      }),
    );

    expect(offers[0]?.options[0]?.label).toBe("Jean Dupont (3 fichiers)");
  });

  it("never renders a name that is in none of the selected files (invariant I7)", () => {
    // Rust restricts the candidates to the selection; this is the second line of defence.
    const leaked = report({
      ambiguities: [
        {
          span: "Martin",
          candidates: [
            candidate(1, "Paul Martin", 2),
            candidate(2, "Pierre Martin", 1),
            candidate(3, "Marc Martin", 0, 0),
          ],
        },
      ],
      possibles: [
        { span: "Dupond", candidates: [candidate(4, "Jean Dupont", 0, 0)], reason: "phonetic", score: 1 },
      ],
    });

    const offers = targetingOffers(french, leaked);

    expect(offers).toHaveLength(1);
    expect(offers[0]?.options.map((option) => option.label)).toStrictEqual([
      "Paul Martin (2 fichiers)",
      "Pierre Martin (1 fichier)",
    ]);
    expect(JSON.stringify(offers)).not.toContain("Marc");
    expect(JSON.stringify(offers)).not.toContain("Jean Dupont");
  });

  it("is no longer a question once only one name of the selection is left", () => {
    const lone = report({
      ambiguities: [
        { span: "Martin", candidates: [candidate(1, "Paul Martin", 2), candidate(2, "Pierre Martin", 0)] },
      ],
    });

    expect(targetingOffers(french, lone)).toStrictEqual([]);
  });

  it("asks nothing more once she has picked one of the names on this answer", () => {
    // Rust still reports that the words fit three people; she has since said which.
    const three = report({
      ambiguities: [
        {
          span: "Dr Martin",
          candidates: [candidate(1, "Claire Martin", 2), candidate(2, "Paul Martin"), candidate(3, "Pierre Martin")],
        },
      ],
      possibles: [
        { span: "Marten", candidates: [candidate(2, "Paul Martin")], reason: "phonetic", score: 1 },
      ],
    });

    expect(targetingOffers(french, three, [2])).toStrictEqual([]);
    expect(targetingOffers(french, martins, [1])).toStrictEqual([]);
    expect(targetingOffers(french, martins, [9])).toHaveLength(1);
  });

  it("offers a name once even if two groups list it", () => {
    const twice = report({
      ambiguities: [
        { span: "Martin", candidates: [candidate(1, "Paul Martin"), candidate(2, "Pierre Martin")] },
      ],
      possibles: [
        { span: "Marten", candidates: [candidate(1, "Paul Martin")], reason: "phonetic", score: 1 },
      ],
    });

    const offers = targetingOffers(french, twice);

    expect(offers).toHaveLength(1);
    expect(offers[0]?.kind).toBe("ambiguous");
  });

  it("offers at most six names in a row", () => {
    const many = report({
      ambiguities: [
        {
          span: "Martin",
          candidates: Array.from({ length: 9 }, (_, index) => candidate(index + 1, `Martin ${index}`)),
        },
      ],
    });

    expect(targetingOffers(french, many)[0]?.options).toHaveLength(6);
  });

  it("offers nothing without a report", () => {
    expect(targetingOffers(french, undefined)).toStrictEqual([]);
  });
});
