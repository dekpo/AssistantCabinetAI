import { describe, expect, it } from "vitest";
import { TEXT_VIEW_LIMIT, prepareViewerText } from "./viewerText";

const encode = (text: string) => new TextEncoder().encode(text);

describe("prepareViewerText", () => {
  it("shows a short text as it is stored", () => {
    const view = prepareViewerText(encode("a\nb\n\nc"));
    expect(view).toStrictEqual({ text: "a\nb\n\nc", truncated: false, shownCharacters: 6 });
  });

  it("turns CRLF and a lone CR into one line break each", () => {
    expect(prepareViewerText(encode("1\r\n00:00:01,000 --> 00:00:02,000\r\ntext\rend")).text).toBe(
      "1\n00:00:01,000 --> 00:00:02,000\ntext\nend",
    );
  });

  it("decodes accents and keeps every header line of a mail on its own line", () => {
    const mail = "From: a\nTo: b\nSubject: été à œ\n\nbody";
    expect(prepareViewerText(encode(mail)).text).toBe(mail);
  });

  it("cuts a long text at a line end and says so", () => {
    const lines = Array.from({ length: 50 }, (_, index) => `line ${String(index).padStart(2, "0")}`);
    const view = prepareViewerText(encode(lines.join("\n")), 100);
    expect(view.truncated).toBe(true);
    expect(view.text.length).toBeLessThanOrEqual(100);
    expect(view.text.endsWith("\n")).toBe(false);
    expect(lines.join("\n").startsWith(view.text)).toBe(true);
    expect(view.text.split("\n").every((line) => /^line \d\d$/.test(line))).toBe(true);
    expect(view.shownCharacters).toBe(view.text.length);
  });

  it("cuts one huge line at the limit", () => {
    const view = prepareViewerText(encode("x".repeat(1000)), 100);
    expect(view.truncated).toBe(true);
    expect(view.text).toBe("x".repeat(100));
  });

  it("decodes only a bounded part of a very large file, without a broken character at the cut", () => {
    const big = encode("é".repeat(10_000));
    const view = prepareViewerText(big, 50);
    expect(view.truncated).toBe(true);
    expect(view.text).toBe("é".repeat(50));
    expect(view.text.includes("�")).toBe(false);
  });

  it("accepts an ArrayBuffer as well as a Uint8Array", () => {
    const bytes = encode("ok");
    expect(prepareViewerText(bytes.buffer.slice(0) as ArrayBuffer).text).toBe("ok");
  });

  it("has a limit the window can lay out", () => {
    expect(TEXT_VIEW_LIMIT).toBe(500_000);
  });
});
