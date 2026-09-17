import { describe, expect, it } from "vitest";
import { CATALOGUES, REFERENCE_LOCALE } from "../i18n/catalogues";
import { createTranslator, type Catalogue } from "../i18n/translate";
import { errorMessage, normaliseError, UNKNOWN_ERROR_CODE } from "./errors";

const french = createTranslator(CATALOGUES["fr-FR"] as Catalogue, CATALOGUES[REFERENCE_LOCALE] as Catalogue);
const english = createTranslator(
  CATALOGUES[REFERENCE_LOCALE] as Catalogue,
  CATALOGUES[REFERENCE_LOCALE] as Catalogue,
);

describe("normaliseError", () => {
  it("keeps a code and its data", () => {
    const error = normaliseError({ code: "work_folder_not_found", data: { path: "D:\\gone" } });

    expect(error).toStrictEqual({ code: "work_folder_not_found", data: { path: "D:\\gone" } });
  });

  it("flattens a list so it can be shown in a sentence", () => {
    const error = normaliseError({
      code: "model_alias_not_allowed",
      data: { requested: "gpt-4o", allowed: ["cabinet-chat", "cabinet-rapide"] },
    });

    expect(error.data.allowed).toBe("cabinet-chat, cabinet-rapide");
  });

  it("turns anything unrecognisable into the unknown code", () => {
    // An exception message is English developer prose: it must never reach the screen.
    expect(normaliseError(new Error("thread 'main' panicked")).code).toBe(UNKNOWN_ERROR_CODE);
    expect(normaliseError("boom").code).toBe(UNKNOWN_ERROR_CODE);
    expect(normaliseError(undefined).code).toBe(UNKNOWN_ERROR_CODE);
    expect(normaliseError({ code: "" }).code).toBe(UNKNOWN_ERROR_CODE);
  });
});

describe("errorMessage", () => {
  it("localises a code and interpolates its data", () => {
    const message = errorMessage(french, {
      code: "work_folder_not_found",
      data: { path: "D:\\travail" },
    });

    expect(message).toContain("D:\\travail");
    expect(message).not.toContain("work_folder_not_found");
  });

  it("gives the same code two different sentences", () => {
    const error = { code: "server_unreachable", data: { url: "http://mac-mini.local:8080" } };

    expect(errorMessage(french, error)).not.toBe(errorMessage(english, error));
    expect(errorMessage(french, error)).toContain("http://mac-mini.local:8080");
  });

  it("falls back to the generic sentence for a code it does not know", () => {
    const message = errorMessage(french, { code: "invented_tomorrow", data: {} });

    expect(message).toBe(errorMessage(french, { code: UNKNOWN_ERROR_CODE, data: {} }));
  });
});
