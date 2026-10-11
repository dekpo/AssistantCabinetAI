import { beforeEach, describe, expect, it, vi } from "vitest";

/* The bridge is replaced by a recorder: the tests read what the webview asks Rust for, and answer
   the stream the way Rust does. Nothing reaches a real window. */
vi.mock("@tauri-apps/api/core", () => {
  class Channel<Message> {
    onmessage: ((message: Message) => void) | null = null;
  }
  return { Channel, invoke: vi.fn() };
});

import { invoke } from "@tauri-apps/api/core";
import {
  askWithSources,
  searchEntities,
  type AskAnswer,
  type ChatStreamEvent,
  type CombinedScope,
  type TargetingReport,
} from "./ipc";

const scope: CombinedScope = {
  mode: { kind: "whole_folder" },
  dataMode: { kind: "explicit", entries: [] },
  createdAt: 1,
  updatedAt: 1,
};

const report: TargetingReport = {
  strategy: "entity_files",
  reason: null,
  requestedBy: "auto",
  widened: "none",
  selectedDocuments: 40,
  candidateDocuments: 7,
  selectedWorkbooks: 0,
  candidateWorkbooks: 0,
  reductionPercent: 82,
  entities: [],
  candidatePaths: ["a.txt"],
  workbookPaths: [],
  ambiguities: [],
  possibles: [],
  chosenWorkbook: null,
};

const answer = { answer: "ok" } as AskAnswer;

/** What Rust sends over the channel, in order, before the call resolves. */
function streams(...events: ChatStreamEvent[]) {
  vi.mocked(invoke).mockImplementation(async (_command, args) => {
    const channel = (args as { onEvent: { onmessage: (m: ChatStreamEvent) => void } }).onEvent;
    for (const event of events) {
      channel.onmessage(event);
    }
    return answer;
  });
}

describe("asking a question", () => {
  /* A block, not an arrow that returns the mock: a function returned from a hook is run as its
     cleanup, which would call the bridge with no arguments after every test. */
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
  });

  it("sends the widen action as the mode `off`, with the question and the selection", async () => {
    streams();

    await askWithSources("Que dit Dupont ?", () => {}, () => {}, undefined, false, scope, [], {
      mode: "off",
    });

    expect(invoke).toHaveBeenCalledWith(
      "ask_with_sources",
      expect.objectContaining({
        question: "Que dit Dupont ?",
        scope,
        targeting: { mode: "off" },
      }),
    );
  });

  it("sends the names she picked as ids, never as words of the question", async () => {
    streams();

    await askWithSources("Que dit Dupont ?", () => {}, () => {}, undefined, false, scope, [], {
      entityIds: [7],
      carryOver: [3],
    });

    const sent = vi.mocked(invoke).mock.calls[0]?.[1] as { question: string; targeting: unknown };
    expect(sent.question).toBe("Que dit Dupont ?");
    expect(sent.targeting).toStrictEqual({ entityIds: [7], carryOver: [3] });
  });

  it("sends no targeting at all for an ordinary question", async () => {
    streams();

    await askWithSources("Bonjour", () => {}, () => {});

    const sent = vi.mocked(invoke).mock.calls[0]?.[1] as { targeting?: unknown };
    expect(sent.targeting).toBeUndefined();
  });

  it("hands the targeting event over before the sources, so a stopped answer still has it", async () => {
    streams(
      { event: "targeting", report },
      { event: "sources", sources: [], coverage: null },
      { event: "delta", text: "Il" },
    );
    const order: string[] = [];

    await askWithSources(
      "Que dit Dupont ?",
      () => order.push("delta"),
      () => order.push("sources"),
      undefined,
      false,
      scope,
      [],
      undefined,
      (found) => {
        order.push("targeting");
        expect(found).toStrictEqual(report);
      },
    );

    expect(order).toStrictEqual(["targeting", "sources", "delta"]);
  });

  it("ignores a targeting event when nobody listens for it", async () => {
    streams({ event: "targeting", report }, { event: "delta", text: "Il" });
    const deltas: string[] = [];

    await askWithSources("Que dit Dupont ?", (text) => deltas.push(text), () => {});

    expect(deltas).toStrictEqual(["Il"]);
  });
});

describe("searching the names of the selection", () => {
  /* A block, not an arrow that returns the mock: a function returned from a hook is run as its
     cleanup, which would call the bridge with no arguments after every test. */
  beforeEach(() => {
    vi.mocked(invoke).mockReset();
  });

  it("asks Rust with the typed letters and the selection, and returns what it found", async () => {
    const found = { entities: [], outsideSelection: [] };
    vi.mocked(invoke).mockResolvedValue(found);

    const result = await searchEntities("Dup", scope, 8);

    expect(invoke).toHaveBeenCalledWith("knowledge_search_entities", {
      prefix: "Dup",
      scope,
      limit: 8,
    });
    expect(result).toBe(found);
  });
});
