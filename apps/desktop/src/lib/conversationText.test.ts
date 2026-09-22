import { describe, expect, it } from "vitest";
import { formatConversation, hasCopyableConversation } from "./conversationText";

const label = (role: "user" | "assistant") => (role === "user" ? "Vous" : "Assistant");

describe("hasCopyableConversation", () => {
  it("is false with no entries", () => {
    expect(hasCopyableConversation([], null)).toBe(false);
  });

  it("is false with only a question and no answer yet", () => {
    const entries = [{ id: "q1", role: "user" as const, content: "Bonjour" }];
    expect(hasCopyableConversation(entries, null)).toBe(false);
  });

  it("is true once a question has a settled answer", () => {
    const entries = [
      { id: "q1", role: "user" as const, content: "Bonjour" },
      { id: "a1", role: "assistant" as const, content: "Bonjour, comment puis-je aider ?" },
    ];
    expect(hasCopyableConversation(entries, null)).toBe(true);
  });

  it("counts an interrupted (incomplete) answer as copyable, since it stays on screen", () => {
    const entries = [
      { id: "q1", role: "user" as const, content: "Bonjour" },
      { id: "a1", role: "assistant" as const, content: "Voici le début" },
    ];
    expect(hasCopyableConversation(entries, null)).toBe(true);
  });

  it("is false while the only answer is still the one streaming", () => {
    const entries = [
      { id: "q1", role: "user" as const, content: "Bonjour" },
      { id: "a1", role: "assistant" as const, content: "En cours d'écriture" },
    ];
    expect(hasCopyableConversation(entries, "a1")).toBe(false);
  });

  it("is true when an earlier pair is settled even while a new answer streams", () => {
    const entries = [
      { id: "q1", role: "user" as const, content: "Bonjour" },
      { id: "a1", role: "assistant" as const, content: "Bonjour" },
      { id: "q2", role: "user" as const, content: "Et ensuite ?" },
      { id: "a2", role: "assistant" as const, content: "En cours" },
    ];
    expect(hasCopyableConversation(entries, "a2")).toBe(true);
  });
});

describe("formatConversation", () => {
  it("labels and joins each settled turn, in order", () => {
    const entries = [
      { id: "q1", role: "user" as const, content: "Bonjour" },
      { id: "a1", role: "assistant" as const, content: "Bonjour, comment puis-je aider ?" },
    ];
    expect(formatConversation(entries, null, label)).toBe(
      "Vous: Bonjour\n\nAssistant: Bonjour, comment puis-je aider ?",
    );
  });

  it("drops the entry currently streaming", () => {
    const entries = [
      { id: "q1", role: "user" as const, content: "Bonjour" },
      { id: "a1", role: "assistant" as const, content: "Bonjour" },
      { id: "q2", role: "user" as const, content: "Et ensuite ?" },
      { id: "a2", role: "assistant" as const, content: "En cours" },
    ];
    expect(formatConversation(entries, "a2", label)).toBe("Vous: Bonjour\n\nAssistant: Bonjour\n\nVous: Et ensuite ?");
  });
});
