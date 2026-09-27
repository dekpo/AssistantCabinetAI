import { describe, expect, it } from "vitest";
import { conversationHistory } from "./history";

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

  it("is empty for a new conversation", () => {
    expect(conversationHistory([])).toStrictEqual([]);
  });
});
