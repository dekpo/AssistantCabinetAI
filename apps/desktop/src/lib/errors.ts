import type { TranslationValues, Translator } from "../i18n/translate";

/**
 * Rust and the gateway both answer with a machine code plus structured data. Turning that into
 * a sentence happens here and nowhere else, which is what keeps the native layers free of
 * user-facing prose.
 */

export const UNKNOWN_ERROR_CODE = "unknown_error";

export interface AppError {
  code: string;
  data: TranslationValues;
}

function readValues(raw: unknown): TranslationValues {
  if (typeof raw !== "object" || raw === null) {
    return {};
  }
  const values: TranslationValues = {};
  for (const [key, value] of Object.entries(raw)) {
    if (typeof value === "string" || typeof value === "number") {
      values[key] = value;
    } else if (Array.isArray(value)) {
      values[key] = value.map((item) => String(item)).join(", ");
    }
  }
  return values;
}

/**
 * Accept whatever crossed the bridge. Anything that is not a recognisable code becomes
 * `unknown_error`: an English exception message must never end up on screen.
 */
export function normaliseError(raw: unknown): AppError {
  if (typeof raw === "object" && raw !== null && "code" in raw) {
    const candidate = raw as { code: unknown; data?: unknown };
    if (typeof candidate.code === "string" && candidate.code.length > 0) {
      return { code: candidate.code, data: readValues(candidate.data) };
    }
  }
  return { code: UNKNOWN_ERROR_CODE, data: {} };
}

export function errorMessage(t: Translator, error: AppError): string {
  const message = t(`errors.${error.code}`, error.data);
  // A code with no catalogue entry would otherwise show the key itself.
  return message === `errors.${error.code}` ? t(`errors.${UNKNOWN_ERROR_CODE}`) : message;
}
