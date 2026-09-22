import { describe, expect, it } from "vitest";
import { truncateForRegenerate, truncateForResend } from "./turns";

const entries = [
  { id: "q1", role: "user" as const, content: "First question" },
  { id: "a1", role: "assistant" as const, content: "First answer" },
  { id: "q2", role: "user" as const, content: "Second question" },
  { id: "a2", role: "assistant" as const, content: "Second answer" },
];

describe("truncateForResend", () => {
  it("drops the edited question and everything after it", () => {
    expect(truncateForResend(entries, "q2")).toEqual(entries.slice(0, 2));
  });

  it("drops the whole conversation when the first question is edited", () => {
    expect(truncateForResend(entries, "q1")).toEqual([]);
  });

  it("returns null for an id that is not in the conversation", () => {
    expect(truncateForResend(entries, "missing")).toBeNull();
  });
});

describe("truncateForRegenerate", () => {
  it("drops the answer and its question, keeping the question's text", () => {
    expect(truncateForRegenerate(entries, "a2")).toEqual({
      base: entries.slice(0, 2),
      question: "Second question",
    });
  });

  it("returns null for an id that is not in the conversation", () => {
    expect(truncateForRegenerate(entries, "missing")).toBeNull();
  });

  it("returns null when nothing precedes the answer", () => {
    expect(truncateForRegenerate([entries[3]!], "a2")).toBeNull();
  });

  it("returns null when the previous entry is not a user question", () => {
    const brokenHistory = [entries[1]!, entries[3]!];
    expect(truncateForRegenerate(brokenHistory, "a2")).toBeNull();
  });
});
