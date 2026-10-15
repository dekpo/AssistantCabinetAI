import { useCallback, useEffect, useState } from "react";
import { useTranslation } from "../i18n/I18nProvider";
import {
  userTypeDelete,
  userTypeSave,
  userTypesList,
  userTypesReevaluate,
  type UserType,
} from "../lib/ipc";
import { textFromPhrases, userTypeFromForm, type UserTypeForm } from "../lib/userTypes";

const EMPTY: UserTypeForm = {
  label: "",
  scope: "documents",
  name: "",
  title: "",
  header: "",
  body: "",
  sheet: "",
};

const PLACES: (keyof Omit<UserTypeForm, "label" | "scope">)[] = ["name", "title", "header", "body", "sheet"];

function formOf(type: UserType): UserTypeForm {
  return {
    label: type.label,
    scope: type.scope,
    name: textFromPhrases(type.name),
    title: textFromPhrases(type.title),
    header: textFromPhrases(type.header),
    body: textFromPhrases(type.body),
    sheet: textFromPhrases(type.sheet),
  };
}

/**
 * The user's own business types: a name and the phrases that are evidence for it in the file name, the title,
 * the header lines, the first words of the text and a workbook's sheets. Kept in the local index; the Analyse
 * never erases them. "Apply now" weighs them against what is already stored, with no file read; the next
 * Analyse completes the reading of the text.
 */
export function BusinessTypesDialog({ onClose }: { onClose: () => void }) {
  const { t } = useTranslation();
  const [types, setTypes] = useState<UserType[]>([]);
  const [form, setForm] = useState<UserTypeForm>(EMPTY);
  const [problem, setProblem] = useState(false);
  const [applied, setApplied] = useState<{ examined: number; changed: number } | null>(null);

  const load = useCallback(() => {
    userTypesList().then(setTypes, () => setTypes([]));
  }, []);

  useEffect(load, [load]);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        onClose();
      }
    };
    document.addEventListener("keydown", onKeyDown);
    return () => document.removeEventListener("keydown", onKeyDown);
  }, [onClose]);

  const save = async () => {
    const rule = userTypeFromForm(form);
    if (rule === null) {
      setProblem(true);
      return;
    }
    setProblem(false);
    try {
      await userTypeSave(rule);
      setForm(EMPTY);
      setApplied(null);
      load();
    } catch {
      setProblem(true);
    }
  };

  return (
    <div className="confirm__backdrop" onMouseDown={onClose}>
      <div
        className="business-types"
        role="dialog"
        aria-modal="true"
        aria-label={t("businessTypes.title")}
        onMouseDown={(event) => event.stopPropagation()}
      >
        <h2 className="confirm__title">{t("businessTypes.title")}</h2>
        <p className="confirm__body">{t("businessTypes.intro")}</p>
        {types.length === 0 ? (
          <p className="business-types__none">{t("businessTypes.none")}</p>
        ) : (
          <ul className="business-types__list">
            {types.map((type) => (
              <li key={type.code} className="business-types__item">
                <span className="business-types__label">{type.label}</span>
                <button
                  type="button"
                  className="button button--compact"
                  onClick={() => setForm(formOf(type))}
                >
                  {t("businessTypes.edit")}
                </button>
                <button
                  type="button"
                  className="button button--compact"
                  onClick={() => void userTypeDelete(type.code).then(load, load)}
                >
                  {t("businessTypes.delete")}
                </button>
              </li>
            ))}
          </ul>
        )}
        <div className="business-types__form">
          <label className="field">
            <span>{t("businessTypes.name")}</span>
            <input
              type="text"
              value={form.label}
              onChange={(event) => setForm({ ...form, label: event.target.value })}
            />
          </label>
          <label className="field">
            <span>{t("businessTypes.scope")}</span>
            <select
              value={form.scope}
              onChange={(event) =>
                setForm({ ...form, scope: event.target.value === "data" ? "data" : "documents" })
              }
            >
              <option value="documents">{t("businessTypes.scopeDocuments")}</option>
              <option value="data">{t("businessTypes.scopeData")}</option>
            </select>
          </label>
          {PLACES.map((place) => (
            <label key={place} className="field">
              <span>{t(`businessTypes.place.${place}`)}</span>
              <textarea
                rows={2}
                value={form[place]}
                onChange={(event) => setForm({ ...form, [place]: event.target.value })}
              />
            </label>
          ))}
          {problem ? <p className="business-types__problem">{t("businessTypes.problem")}</p> : null}
        </div>
        {applied === null ? null : (
          <p className="business-types__result" role="status">
            {t("businessTypes.applied", {
              examined: String(applied.examined),
              changed: String(applied.changed),
            })}
          </p>
        )}
        <div className="confirm__actions">
          <button
            type="button"
            className="button"
            onClick={() => void userTypesReevaluate().then(setApplied, () => setApplied(null))}
          >
            {t("businessTypes.apply")}
          </button>
          <button type="button" className="button button--primary" onClick={() => void save()}>
            {t("businessTypes.save")}
          </button>
          <button type="button" className="button" onClick={onClose}>
            {t("businessTypes.close")}
          </button>
        </div>
      </div>
    </div>
  );
}
