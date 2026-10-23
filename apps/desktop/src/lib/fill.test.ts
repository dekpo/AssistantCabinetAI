import { describe, expect, it } from "vitest";
import {
  canGenerateOne,
  draftBytes,
  initialDecisions,
  isDrafted,
  mergeDecisions,
  MAX_DRAFT_BYTES,
  pendingConfirmations,
  requestOf,
  type FillPlaceholder,
} from "./fill";

const placeholders: FillPlaceholder[] = [
  { name: "Nom", column: "Nom", status: "exact" },
  { name: "Prix_U", column: "Prix_Unitaire", status: "proposed" },
  { name: "Suite", column: null, status: "unresolved" },
];

describe("the starting decisions", () => {
  it("confirm an exact match, wait for a proposal, and have nothing to confirm for a field with no column", () => {
    const decisions = initialDecisions(placeholders);
    expect(decisions["Nom"]).toEqual({ column: "Nom", confirmed: true });
    expect(decisions["Prix_U"]).toEqual({ column: "Prix_Unitaire", confirmed: false });
    expect(decisions["Suite"]).toEqual({ column: null, confirmed: true });
  });

  it("list the proposals still to confirm, and none once she has", () => {
    const decisions = initialDecisions(placeholders);
    expect(pendingConfirmations(decisions)).toEqual(["Prix_U"]);
    decisions["Prix_U"] = { column: "Prix_Unitaire", confirmed: true };
    expect(pendingConfirmations(decisions)).toEqual([]);
  });
});

describe("the request for Rust", () => {
  const plan = { dataFile: "donnees.csv", sheet: "Donnees", keyValue: "CMD-2026-002" };

  it("never fills a proposal she has not confirmed: it is sent as 'leave as written'", () => {
    const request = requestOf(plan, {
      template: "modele.docx",
      all: false,
      keyColumn: "No_Commande",
      decisions: initialDecisions(placeholders),
    });
    expect(request.mapping).toEqual([
      { placeholder: "Nom", column: "Nom" },
      { placeholder: "Prix_U", column: null },
      { placeholder: "Suite", column: null },
    ]);
    expect(request.keyValue).toBe("CMD-2026-002");
    expect(request.all).toBe(false);
    // The sheet the plan was built from travels back, so a workbook of several sheets can be read again.
    expect(request.sheet).toBe("Donnees");
  });

  it("sends a confirmed proposal, or the column she chose instead", () => {
    const decisions = initialDecisions(placeholders);
    decisions["Prix_U"] = { column: "Total_Ligne", confirmed: true };
    const request = requestOf(plan, { template: "modele.docx", all: false, keyColumn: "No_Commande", decisions });
    expect(request.mapping.find((entry) => entry.placeholder === "Prix_U")?.column).toBe("Total_Ligne");
  });

  it("drops the named order when she asks for every order, and keeps the key column she picked", () => {
    const request = requestOf(plan, {
      template: "modele.docx",
      all: true,
      keyColumn: "No_Commande",
      decisions: {},
    });
    expect(request.all).toBe(true);
    expect(request.keyValue).toBeNull();
    expect(request.keyColumn).toBe("No_Commande");
  });

  it("sends no draft for a template file, and the text she edited for a drafted one", () => {
    const state = { template: "modele.docx", all: true, keyColumn: null, decisions: {} };
    expect(requestOf(plan, state).draftText).toBeNull();
    const drafted = requestOf(plan, { ...state, template: "", draftText: "Bonjour {{Nom}}" });
    expect(drafted.draftText).toBe("Bonjour {{Nom}}");
    expect(drafted.template).toBe("");
  });

  it("offers 'this order only' only when the question named one", () => {
    expect(canGenerateOne({ keyValue: "CMD-2026-002" })).toBe(true);
    expect(canGenerateOne({ keyValue: null })).toBe(false);
  });
});

describe("a drafted template", () => {
  it("is recognised by its origin", () => {
    expect(isDrafted({ templateOrigin: "drafted" })).toBe(true);
    expect(isDrafted({ templateOrigin: "file" })).toBe(false);
  });

  it("keeps what she decided for a field that stays, starts a new one as proposed, drops a field that left", () => {
    const current = {
      Nom: { column: "Nom", confirmed: true },
      Prix_U: { column: "Total_Ligne", confirmed: true },
    };
    const merged = mergeDecisions(current, [
      { name: "Prix_U", column: "Prix_Unitaire", status: "proposed" },
      { name: "Suite", column: "Suite", status: "exact" },
    ]);
    expect(merged).toEqual({
      Prix_U: { column: "Total_Ligne", confirmed: true },
      Suite: { column: "Suite", confirmed: true },
    });
  });

  it("returns the same decisions when the fields did not change, so a state update ends there", () => {
    const current = { Nom: { column: "Nom", confirmed: true } };
    expect(mergeDecisions(current, [{ name: "Nom", column: "Nom", status: "exact" }])).toBe(current);
  });

  it("counts the text as Rust does: in bytes, not in characters", () => {
    expect(draftBytes("\u00e9")).toBe(2);
    expect(draftBytes("a".repeat(MAX_DRAFT_BYTES))).toBe(MAX_DRAFT_BYTES);
  });
});
