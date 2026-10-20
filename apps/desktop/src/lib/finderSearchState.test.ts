import { describe, expect, it } from "vitest";
import { SEARCH_DELAY_MS } from "./finderField";
import {
  initialSearchState,
  searchReducer,
  type SearchAction,
  type SearchState,
} from "./finderSearchState";

function run(start: SearchState, ...actions: SearchAction[]): SearchState {
  return actions.reduce(searchReducer, start);
}

const ON = initialSearchState(true);
const type = (text: string): SearchAction => ({ type: "textChanged", text });

describe("typing", () => {
  it("asks for a search after a pause, for the trimmed text", () => {
    const state = run(ON, type("  rapport  "));

    expect(state.text).toBe("  rapport  ");
    expect(state.wanted).toStrictEqual({ id: state.request, text: "rapport", delayMs: SEARCH_DELAY_MS });
  });

  it("replaces the pending search at every keystroke: only the last one is asked for", () => {
    const first = run(ON, type("rap"));
    const second = run(first, type("rapp"));

    expect(second.wanted?.text).toBe("rapp");
    expect(second.wanted?.id).toBeGreaterThan(first.wanted?.id ?? 0);
  });

  it("asks for nothing while the text is blank, and ends the filter", () => {
    const found = run(ON, type("rapport"), { type: "answered", id: 1, paths: ["a.pdf"] });
    const cleared = run(found, type("   "));

    expect(cleared.found).toBeNull();
    expect(cleared.wanted).toBeNull();
    expect(cleared.showAll).toBe(false);
    expect(cleared.failed).toBe(false);
  });

  it("asks for nothing when the field is disabled", () => {
    expect(run(initialSearchState(false), type("rapport")).wanted).toBeNull();
  });

  it("keeps the previous results on screen until the new answer arrives", () => {
    const state = run(ON, type("rap"), { type: "answered", id: 1, paths: ["a.pdf"] }, type("rapp"));

    expect(state.found).toStrictEqual(["a.pdf"]);
  });
});

describe("answers", () => {
  it("are kept when they answer the latest request", () => {
    const asked = run(ON, type("rapport"));
    const state = run(asked, { type: "answered", id: asked.request, paths: ["a.pdf", "b.pdf"] });

    expect(state.found).toStrictEqual(["a.pdf", "b.pdf"]);
    expect(state.wanted).toBeNull();
    expect(state.failed).toBe(false);
  });

  it("are dropped when a newer search has been asked for", () => {
    const first = run(ON, type("rap"));
    const second = run(first, type("rapport"));
    const state = run(second, { type: "answered", id: first.request, paths: ["old.pdf"] });

    expect(state.found).toBeNull();
    expect(state).toBe(second);
  });

  it("are dropped when the field was cleared meanwhile", () => {
    const asked = run(ON, type("rapport"));
    const state = run(asked, type(""), { type: "answered", id: asked.request, paths: ["a.pdf"] });

    expect(state.found).toBeNull();
  });

  it("are dropped when the field was disabled meanwhile, and the failure of a stale search too", () => {
    const asked = run(ON, type("rapport"));
    const off = run(asked, { type: "enabledChanged", enabled: false });

    expect(run(off, { type: "answered", id: asked.request, paths: ["a.pdf"] }).found).toBeNull();
    expect(run(off, { type: "failed", id: asked.request }).failed).toBe(false);
  });

  it("are dropped when the folder changed meanwhile", () => {
    const asked = run(ON, type("rapport"));
    const state = run(asked, { type: "folderChanged" }, { type: "answered", id: asked.request, paths: ["a.pdf"] });

    expect(state.found).toBeNull();
  });

  it("say that the search failed, and the next answer clears it", () => {
    const asked = run(ON, type("rapport"));
    const failed = run(asked, { type: "failed", id: asked.request });
    const retried = run(failed, type("rapport "));

    expect(failed.failed).toBe(true);
    expect(failed.found).toBeNull();
    expect(run(retried, { type: "answered", id: retried.request, paths: [] }).failed).toBe(false);
  });
});

describe("the folder", () => {
  it("empties the text and the results when it changes", () => {
    const state = run(
      ON,
      type("rapport"),
      { type: "answered", id: 1, paths: ["a.pdf"] },
      { type: "showAllChanged", showAll: true },
      { type: "folderChanged" },
    );

    expect(state.text).toBe("");
    expect(state.found).toBeNull();
    expect(state.showAll).toBe(false);
    expect(state.wanted).toBeNull();
  });

  it("changes nothing when there was nothing to empty", () => {
    expect(run(ON, { type: "folderChanged" })).toBe(ON);
  });
});

describe("a search asked again", () => {
  it("runs at once, for the same text, when a pass finished or the inventory was read again", () => {
    const answered = run(ON, type("rapport"), { type: "answered", id: 1, paths: ["old-name.pdf"] });
    const state = run(answered, { type: "refresh" });

    expect(state.wanted).toStrictEqual({ id: state.request, text: "rapport", delayMs: 0 });
    // The results on screen stay until the new answer replaces them.
    expect(state.found).toStrictEqual(["old-name.pdf"]);
    expect(run(state, { type: "answered", id: state.request, paths: ["new-name.pdf"] }).found).toStrictEqual([
      "new-name.pdf",
    ]);
  });

  it("makes the answer of the previous request stale", () => {
    const asked = run(ON, type("rapport"));
    const state = run(asked, { type: "refresh" }, { type: "answered", id: asked.request, paths: ["x.pdf"] });

    expect(state.found).toBeNull();
  });

  it("does nothing when no search is active, or while the field is disabled", () => {
    expect(run(ON, { type: "refresh" })).toBe(ON);
    const off = run(ON, type("rapport"), { type: "enabledChanged", enabled: false });
    expect(run(off, { type: "refresh" })).toBe(off);
  });

  it("runs at once when the field is enabled again with a text", () => {
    const off = run(ON, type("rapport"), { type: "enabledChanged", enabled: false });
    const on = run(off, { type: "enabledChanged", enabled: true });

    expect(on.wanted).toStrictEqual({ id: on.request, text: "rapport", delayMs: 0 });
  });
});

describe("the view controls", () => {
  it("show all and show the results only without a new search", () => {
    const answered = run(ON, type("rapport"), { type: "answered", id: 1, paths: ["a.pdf"] });
    const all = run(answered, { type: "showAllChanged", showAll: true });

    expect(all.showAll).toBe(true);
    expect(all.found).toStrictEqual(["a.pdf"]);
    expect(all.request).toBe(answered.request);
  });

  it("have nothing to say about a selection: the state holds none", () => {
    const state = run(ON, type("rapport"), { type: "answered", id: 1, paths: ["a.pdf"] }, type(""));

    expect(Object.keys(state).sort()).toStrictEqual(
      [
        "enabled",
        "failed",
        "found",
        "reasons",
        "request",
        "showAll",
        "text",
        "typedFormats",
        "wanted",
      ].sort(),
    );
  });
});

describe("the reasons and the typed formats of an answer", () => {
  const reasons = { "a.pdf": [{ kind: "word" as const, detail: "name" }] };

  it("are kept with the paths when the answer is the latest", () => {
    const asked = run(ON, type("rapport .pdf"));
    const state = run(asked, {
      type: "answered",
      id: asked.request,
      paths: ["a.pdf"],
      reasons,
      typedFormats: ["pdf"],
    });

    expect(state.reasons).toStrictEqual(reasons);
    expect(state.typedFormats).toStrictEqual(["pdf"]);
  });

  it("are empty when the answer carries none", () => {
    const asked = run(ON, type("rapport"));
    const state = run(asked, { type: "answered", id: asked.request, paths: ["a.pdf"] });

    expect(state.reasons).toStrictEqual({});
    expect(state.typedFormats).toStrictEqual([]);
  });

  it("are dropped with a stale answer, and when the search ends or fails", () => {
    const asked = run(ON, type("rapport"));
    const stale = run(asked, { type: "answered", id: asked.request - 1, paths: ["a.pdf"], reasons, typedFormats: ["pdf"] });
    expect(stale.reasons).toStrictEqual({});

    const answered = run(asked, { type: "answered", id: asked.request, paths: ["a.pdf"], reasons, typedFormats: ["pdf"] });
    expect(run(answered, type("")).reasons).toStrictEqual({});
    expect(run(answered, type("")).typedFormats).toStrictEqual([]);
    const retyped = run(answered, type("rapports"));
    expect(run(retyped, { type: "failed", id: retyped.request }).reasons).toStrictEqual({});
  });

  it("stay on screen until the next answer replaces them", () => {
    const answered = run(ON, type("rap"), { type: "answered", id: 1, paths: ["a.pdf"], reasons, typedFormats: [] });

    expect(run(answered, type("rapp")).reasons).toStrictEqual(reasons);
  });
});
