import { describe, expect, it } from "vitest";
import type { ChatEntry } from "../state/useChat";
import { conversationHistory } from "./history";
import type { TargetingReport } from "./ipc";

describe("conversation history", () => {
  it("keeps each question with its answer, in order", () => {
    expect(
      conversationHistory([
        { role: "user", content: "q1" },
        { role: "assistant", content: "a1" },
        { role: "user", content: "q2" },
        { role: "assistant", content: "a2" },
      ]),
    ).toStrictEqual([
      { role: "user", content: "q1" },
      { role: "assistant", content: "a1" },
      { role: "user", content: "q2" },
      { role: "assistant", content: "a2" },
    ]);
  });

  it("leaves out the software's own message and the question it answered", () => {
    expect(
      conversationHistory([
        { role: "user", content: "q1" },
        { role: "assistant", content: "Analyse your documents first.", needsIndexing: true },
        { role: "user", content: "q2" },
        { role: "assistant", content: "a2" },
      ]),
    ).toStrictEqual([
      { role: "user", content: "q2" },
      { role: "assistant", content: "a2" },
    ]);
  });

  it("leaves out a question that never got an answer", () => {
    expect(
      conversationHistory([
        { role: "user", content: "a question that failed" },
        { role: "user", content: "q2" },
        { role: "assistant", content: "a2" },
        { role: "user", content: "q3" },
        { role: "assistant", content: "" },
      ]),
    ).toStrictEqual([
      { role: "user", content: "q2" },
      { role: "assistant", content: "a2" },
    ]);
  });

  it("leaves out the tabular engine's suggestion of what to ask instead", () => {
    expect(
      conversationHistory([
        { role: "user", content: "q1" },
        { role: "assistant", content: "Its columns are: montant.", tabularNudge: true },
        { role: "user", content: "q2" },
        { role: "assistant", content: "a2" },
      ]),
    ).toStrictEqual([
      { role: "user", content: "q2" },
      { role: "assistant", content: "a2" },
    ]);
  });

  it("is empty for a new conversation", () => {
    expect(conversationHistory([])).toStrictEqual([]);
  });
});

describe("what the model is told of the conversation", () => {
  it("carries nothing of the targeting: not the line, not the names, not the timings", () => {
    const report: TargetingReport = {
      strategy: "entity_files",
      reason: null,
      requestedBy: "chip",
      widened: "auto",
      selectedDocuments: 40,
      candidateDocuments: 3,
      selectedWorkbooks: 0,
      candidateWorkbooks: 0,
      reductionPercent: 92,
      entities: [
        { entityId: 7, display: "Jean Dupont", typeId: "person", confidence: 1, method: "chip" },
      ],
      candidatePaths: ["dupont-bilan.txt", "dupont-lettre.txt", "dupont-note.txt"],
      workbookPaths: [],
      ambiguities: [],
      possibles: [],
      chosenWorkbook: null,
    };
    const entries: ChatEntry[] = [
      { id: "q1", role: "user", content: "Que dit Jean Dupont ?", createdAt: 1 },
      {
        id: "a1",
        role: "assistant",
        content: "Il recommande un bilan.",
        createdAt: 1,
        targeting: report,
        targetingChoice: { entityIds: [7] },
        durationMs: 1200,
        modelAlias: "ministral-3:3b",
      },
      { id: "q2", role: "user", content: "Et son adresse ?", createdAt: 2 },
      { id: "a2", role: "assistant", content: "Elle n'est pas indiquee.", createdAt: 2, targeting: report },
    ];

    const history = conversationHistory(entries);

    expect(history).toStrictEqual([
      { role: "user", content: "Que dit Jean Dupont ?" },
      { role: "assistant", content: "Il recommande un bilan." },
      { role: "user", content: "Et son adresse ?" },
      { role: "assistant", content: "Elle n'est pas indiquee." },
    ]);
    for (const turn of history) {
      expect(Object.keys(turn).sort()).toStrictEqual(["content", "role"]);
    }
    expect(JSON.stringify(history)).not.toContain("dupont-bilan.txt");
    expect(JSON.stringify(history)).not.toContain("entityId");
  });
});
