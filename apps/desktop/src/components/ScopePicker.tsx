import { useRef, useState } from "react";
import { useTranslation } from "../i18n/I18nProvider";
import { scopedPaths, toggleScopeFile, wholeFolderScope } from "../lib/analysisScope";
import { workFolderInventory, type AnalysisScope, type FileRecord } from "../lib/ipc";
import { counted } from "../lib/plural";

/**
 * Which documents this conversation may rely on. The default is the whole folder; ticking files
 * narrows it. Only files whose analysis is current are offered, because only they can be searched as they are now, and choosing one
 * copies nothing: it names a file the folder already holds.
 */
export function ScopePicker({
  scope,
  onChange,
  disabled,
}: {
  scope: AnalysisScope;
  onChange: (scope: AnalysisScope) => void;
  /** A question is being answered: changing the scope under it would describe a different answer
   * from the one being written. */
  disabled: boolean;
}) {
  const { t } = useTranslation();
  const [files, setFiles] = useState<FileRecord[] | null>(null);
  const chosen = scopedPaths(scope);
  /* Which read is the latest. Opening, closing and reopening starts overlapping reads, and the one
     that resolves last is not necessarily the one asked for last. */
  const latestLoad = useRef(0);

  /* Read when the disclosure opens, not on mount: a file analysed since the last look must be
     offered, and a picker nobody opens should cost nothing. A failed read leaves the list empty
     rather than wrong. */
  const load = () => {
    const thisLoad = ++latestLoad.current;
    workFolderInventory().then(
      (report) => {
        if (thisLoad === latestLoad.current) {
          setFiles(report.files.filter((file) => file.processingStatus === "indexed"));
        }
      },
      () => {
        if (thisLoad === latestLoad.current) {
          setFiles([]);
        }
      },
    );
  };

  return (
    <details
      className="inventory-detail scope-picker"
      onToggle={(event) => {
        if (event.currentTarget.open) {
          load();
        }
      }}
    >
      <summary className="disclosure">
        {chosen.length === 0
          ? t("chat.scopeSummaryWhole")
          : t("chat.scopeSummaryExplicit", {
              files: counted(chosen.length, t("chat.scopeFileOne"), t("chat.scopeFileMany")),
            })}
      </summary>
      <p className="inventory-detail__summary">{t("chat.scopeHelp")}</p>
      {files !== null && files.length === 0 ? (
        <p className="inventory-detail__summary">{t("chat.scopeEmpty")}</p>
      ) : (
        <ul className="inventory">
          {(files ?? []).map((file) => (
            <li key={file.id + file.relativePath}>
              <label>
                <input
                  type="checkbox"
                  disabled={disabled}
                  checked={chosen.includes(file.relativePath)}
                  onChange={() => onChange(toggleScopeFile(scope, file, Date.now()))}
                />{" "}
                {file.relativePath}
              </label>
            </li>
          ))}
        </ul>
      )}
      {chosen.length === 0 ? null : (
        <button
          type="button"
          className="button button--compact"
          disabled={disabled}
          onClick={() => onChange({ ...wholeFolderScope(scope.createdAt), updatedAt: Date.now() })}
        >
          {t("chat.scopeWholeFolder")}
        </button>
      )}
    </details>
  );
}
