import { useEffect, useState } from "react";
import { useTranslation } from "../../i18n/I18nProvider";
import { normaliseError, type AppError } from "../../lib/errors";
import { knowledgeTypesAndRoles, type TypesAndRoles } from "../../lib/ipc";
import { typeLabel, type ManageApi } from "../../lib/knowledgeManage";
import { counted } from "../../lib/plural";
import { ErrorBanner } from "../ErrorBanner";

/** The pack's name in the user's language, or the pack id for a module this build does not label. */
function packLabel(t: (key: string) => string, id: string): string {
  const key = `manage.pack.${id}`;
  const text = t(key);
  return text === key ? id : text;
}

/**
 * "Types and roles": a read-only description of the vocabulary - the types of entity with how many
 * the base holds, the roles a module knows by label, and the modules in use. Nothing here changes
 * the base; the modules are switched on and off in the settings.
 */
export function TypesTab({ api }: { api: ManageApi }) {
  const { t } = useTranslation();
  const [found, setFound] = useState<TypesAndRoles | null>(null);
  const [error, setError] = useState<AppError | null>(null);

  useEffect(() => {
    let current = true;
    knowledgeTypesAndRoles().then(
      (read) => {
        if (current) {
          setFound(read);
          setError(null);
        }
      },
      (raw: unknown) => {
        if (current) {
          setError(normaliseError(raw));
        }
      },
    );
    return () => {
      current = false;
    };
  }, [api.revision]);

  const loaded = new Set((found?.packs ?? []).map((pack) => pack.id));

  return (
    <div className="knowledge__review">
      <p className="field__description">{t("manage.types.intro")}</p>
      {error === null ? null : <ErrorBanner error={error} />}
      {found === null ? null : (
        <>
          <section className="sheet__section">
            <h3 className="sheet__heading">{t("manage.types.typesTitle")}</h3>
            <ul className="sheet__aliases">
              {found.types.map((type) => (
                <li key={type.id} className="sheet__alias">
                  <span className="sheet__alias-name">{typeLabel(t, type.id)}</span>
                  <span className="sheet__meta">
                    {counted(
                      type.entities,
                      t("manage.types.entitiesOne"),
                      t("manage.types.entitiesMany"),
                    )}
                  </span>
                  {type.builtin ? <span className="tag">{t("manage.types.builtin")}</span> : null}
                </li>
              ))}
            </ul>
          </section>

          <section className="sheet__section">
            <h3 className="sheet__heading">{t("manage.types.rolesTitle")}</h3>
            {found.roles.length === 0 ? (
              <p className="sheet__meta">{t("manage.types.rolesNone")}</p>
            ) : (
              <ul className="sheet__aliases">
                {found.roles.map((role) => (
                  <li key={role.id} className="sheet__alias">
                    <span className="sheet__alias-name">{t(role.labelKey)}</span>
                  </li>
                ))}
              </ul>
            )}
          </section>

          <section className="sheet__section">
            <h3 className="sheet__heading">{t("manage.types.packsTitle")}</h3>
            <p className="field__description">{t("manage.types.packsActive")}</p>
            <ul className="sheet__aliases">
              {found.packs.map((pack) => (
                <li key={pack.id} className="sheet__alias">
                  <span className="sheet__alias-name">{packLabel(t, pack.id)}</span>
                </li>
              ))}
            </ul>
            <p className="field__description">{t("manage.types.packsAvailable")}</p>
            <ul className="sheet__aliases">
              {found.availablePacks.map((id) => (
                <li key={id} className="sheet__alias">
                  <span className="sheet__alias-name">{packLabel(t, id)}</span>
                  <span className="tag">
                    {loaded.has(id) ? t("manage.pack.active") : t("manage.pack.inactive")}
                  </span>
                </li>
              ))}
            </ul>
          </section>
        </>
      )}
    </div>
  );
}
