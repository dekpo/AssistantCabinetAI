import { describe, expect, it } from "vitest";
import {
  canGenerateOne,
  initialDecisions,
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
  const plan = { dataFile: "donnees.csv", keyValue: "CMD-2026-002" };

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

  it("offers 'this order only' only when the question named one", () => {
    expect(canGenerateOne({ keyValue: "CMD-2026-002" })).toBe(true);
    expect(canGenerateOne({ keyValue: null })).toBe(false);
  });
});
