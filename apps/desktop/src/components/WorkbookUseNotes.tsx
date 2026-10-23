import { useTranslation } from "../i18n/I18nProvider";
import { foldedLines, type WorkbookUseLine } from "../lib/workbookUse";

/**
 * The lines about the workbooks of an answer (`lib/workbookUse.ts`): what was done with each file the
 * question named or the user ticked. Long lists fold behind one title; a line that warns the reader
 * a file did not serve stays in view. The text is the one the copy button and the read-aloud use.
 */
export function WorkbookUseNotes({ lines }: { lines: readonly WorkbookUseLine[] }) {
  const { t } = useTranslation();
  const { shown, folded } = foldedLines(lines);
  return (
    <>
      {shown.map((line, index) => (
        <p className="message__timing" key={`workbook-${index}`}>
          {line.text}
        </p>
      ))}
      {folded.length === 0 ? null : (
        <details className="message__sources">
          <summary className="disclosure">{t("workbookUse.foldTitle", { count: folded.length })}</summary>
          <ul className="message__sources-list">
            {folded.map((line, index) => (
              <li key={`workbook-folded-${index}`} className="message__source">
                {line.text}
              </li>
            ))}
          </ul>
        </details>
      )}
    </>
  );
}
