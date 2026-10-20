import { describe, expect, it } from "vitest";
import { NO_CHOICES } from "./explorerFilters";
import { NO_VIEW, type ExplorerView } from "./explorerView";
import {
  addView,
  BUILT_IN_VIEWS,
  cleanViewName,
  deleteView,
  isEmptyView,
  MAX_SAVED_VIEWS,
  MAX_VIEW_NAME,
  missingEntities,
  nameProblem,
  newViewId,
  renameView,
  sameView,
  savedFromView,
  viewFromSaved,
} from "./explorerViews";
import type { SavedViewSetting } from "./ipc";

const rich: ExplorerView = {
  family: "documents",
  choices: {
    ...NO_CHOICES,
    formats: new Set(["pdf", "docx"]),
    states: new Set(["unreadable", "analysed"]),
    folders: new Set(["documents:Invoices"]),
    people: new Set(["12", "14:author"]),
    organisations: new Set(["40"]),
    types: new Set(["invoice"]),
    years: new Set(["2026", "2025"]),
    recent: true,
    inConversation: false,
  },
};

const saved = (id: string, name: string): SavedViewSetting => savedFromView(NO_VIEW, id, name);

describe("a saved view", () => {
  it("is opened as the filters it was saved with", () => {
    const reopened = viewFromSaved(savedFromView(rich, "v1", "Mine"));

    expect(reopened.family).toBe("documents");
    expect([...reopened.choices.formats].sort()).toStrictEqual(["docx", "pdf"]);
    expect([...reopened.choices.people].sort()).toStrictEqual(["12", "14:author"]);
    expect(reopened.choices.recent).toBe(true);
    expect(sameView(reopened, rich)).toBe(true);
  });

  it("holds filters only: no file, no selection, no typed text", () => {
    const keys = Object.keys(savedFromView(rich, "v1", "Mine")).sort();

    expect(keys).toStrictEqual(
      [
        "family",
        "folders",
        "formats",
        "id",
        "inConversation",
        "name",
        "organisations",
        "people",
        "recent",
        "states",
        "types",
        "years",
      ].sort(),
    );
  });

  it("is written in a stable order, whatever the order of the choices", () => {
    const first = savedFromView(rich, "v1", "Mine");
    const shuffled: ExplorerView = {
      ...rich,
      choices: {
        ...rich.choices,
        formats: new Set(["docx", "pdf"]),
        years: new Set(["2025", "2026"]),
        states: new Set(["analysed", "unreadable"]),
      },
    };

    expect(savedFromView(shuffled, "v1", "Mine")).toStrictEqual(first);
    expect(first.states).toStrictEqual(["analysed", "unreadable"]);
  });

  it("drops a state it does not know when opened", () => {
    const odd = { ...saved("v1", "Odd"), states: ["analysed", "bogus"] } as unknown as SavedViewSetting;

    expect([...viewFromSaved(odd).choices.states]).toStrictEqual(["analysed"]);
  });
});

describe("the built-in views", () => {
  const byId = (id: string) => BUILT_IN_VIEWS.find((entry) => entry.id === id)?.view;

  it("are the three the owner asked for, each one set of filters", () => {
    expect(BUILT_IN_VIEWS.map((entry) => entry.id)).toStrictEqual([
      "recent",
      "toAnalyse",
      "inConversation",
    ]);
    expect(byId("recent")?.choices.recent).toBe(true);
    expect([...(byId("toAnalyse")?.choices.states ?? [])]).toStrictEqual(["toAnalyse"]);
    expect(byId("inConversation")?.choices.inConversation).toBe(true);
  });

  it("each narrow or order nothing else", () => {
    for (const { view } of BUILT_IN_VIEWS) {
      expect(view.family).toBe("all");
      expect(view.choices.people.size + view.choices.types.size + view.choices.folders.size).toBe(0);
    }
  });
});

describe("naming a view", () => {
  it("cleans the name and cuts it at the limit", () => {
    expect(cleanViewName("  Factures   2026 ")).toBe("Factures 2026");
    expect(cleanViewName("x".repeat(200))).toHaveLength(MAX_VIEW_NAME);
  });

  it("refuses an empty name and a name another view has, case and accents ignored", () => {
    const views = [saved("a", "Factures")];

    expect(nameProblem(views, "   ")).toBe("empty");
    expect(nameProblem(views, "FACTURES")).toBe("taken");
    expect(nameProblem(views, "facturées")).toBeNull();
    expect(nameProblem(views, "Autre")).toBeNull();
  });

  it("lets a view keep its own name when it is renamed", () => {
    const views = [saved("a", "Factures"), saved("b", "Courriers")];

    expect(nameProblem(views, "factures", "a")).toBeNull();
    expect(nameProblem(views, "factures", "b")).toBe("taken");
  });
});

describe("the list of saved views", () => {
  it("adds a view once, under a name nobody has", () => {
    const one = addView([], saved("a", "Factures"));
    const twice = addView(one, saved("b", "FACTURES"));
    const two = addView(one, saved("b", "Courriers"));

    expect(one).toHaveLength(1);
    expect(twice).toHaveLength(1);
    expect(two.map((view) => view.name)).toStrictEqual(["Factures", "Courriers"]);
  });

  it("adds nothing past the limit", () => {
    const full = Array.from({ length: MAX_SAVED_VIEWS }, (_, n) => saved(`v${n}`, `View ${n}`));

    expect(addView(full, saved("more", "One more"))).toHaveLength(MAX_SAVED_VIEWS);
  });

  it("renames one view and leaves the others and its filters alone", () => {
    const views = [savedFromView(rich, "a", "Factures"), saved("b", "Courriers")];
    const renamed = renameView(views, "a", "  Mes factures ");

    expect(renamed[0]?.name).toBe("Mes factures");
    expect(renamed[0]?.people).toStrictEqual(views[0]?.people);
    expect(renamed[1]).toStrictEqual(views[1]);
    expect(renameView(views, "a", "courriers")).toStrictEqual(views);
  });

  it("deletes one view only", () => {
    const views = [saved("a", "Factures"), saved("b", "Courriers")];

    expect(deleteView(views, "a").map((view) => view.id)).toStrictEqual(["b"]);
    expect(deleteView(views, "nope")).toHaveLength(2);
  });

  it("gives each new view an id no other has", () => {
    const views = [saved("vabc", "A")];
    const id = newViewId(views, parseInt("abc", 36));

    expect(id).not.toBe("vabc");
    expect(views.some((view) => view.id === id)).toBe(false);
  });
});

describe("what a view holds", () => {
  it("is empty only when nothing is chosen", () => {
    expect(isEmptyView(NO_VIEW)).toBe(true);
    expect(isEmptyView({ ...NO_VIEW, family: "tables" })).toBe(false);
    expect(isEmptyView(rich)).toBe(false);
    expect(isEmptyView({ family: "all", choices: { ...NO_CHOICES, recent: true } })).toBe(false);
  });

  it("names the people and organisations the files no longer link to", () => {
    expect(missingEntities(rich, new Set([12, 14, 40]))).toStrictEqual([]);
    expect(missingEntities(rich, new Set([12]))).toStrictEqual([14, 40]);
    expect(missingEntities(NO_VIEW, new Set())).toStrictEqual([]);
  });
});
