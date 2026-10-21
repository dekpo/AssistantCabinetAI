import { memo, useState, type ReactNode } from "react";
import { useTranslation } from "../i18n/I18nProvider";
import type { Translator } from "../i18n/translate";
import { durationText } from "../lib/analysisLines";
import { errorMessage, normaliseError, type AppError } from "../lib/errors";
import type { ExplorerEntry, ExplorerState, Family } from "../lib/explorerModel";
import { knowledgeLines } from "../lib/knowledgeSummary";
import {
  type CheckItem,
  type CheckReason,
  type FamilyFigures,
  type OverviewBlock,
  type OverviewPage as OverviewPageModel,
  type TimePartId,
} from "../lib/overviewModel";
import { CAPABILITY_KEYS } from "../lib/passSummary";
import { counted, pluralise } from "../lib/plural";
import { ErrorBanner } from "./ErrorBanner";
import { FamilyFolderGlyph, StateGlyph } from "./ExplorerGlyphs";
import { KnowledgeLink } from "./FolderCardParts";

/** What the page can ask of the application. Each is a plain action; the page decides nothing. */
export interface OverviewActions {
  /** « Afficher les N dans la liste de gauche »: sets the explorer's state filter. */
  onShowUnreadable: () => void;
  onOpenKnowledge: () => void;
  onOpenFile: (entry: ExplorerEntry) => void;
  /** Shows the file in the file manager; a failure is handed to `onFailure` to be said. */
  onRevealFile: (entry: ExplorerEntry, onFailure: (error: AppError) => void) => void;
}

const FAMILY_NAME: Record<Family, string> = {
  documents: "explorer.familyDocuments",
  tables: "explorer.familyTables",
};

const FAMILY_TYPES: Record<Family, string> = {
  documents: "explorer.typesDocuments",
  tables: "explorer.typesTables",
};

const TIME_LABEL: Record<TimePartId, string> = {
  read: "overview.timeRead",
  recognition: "overview.timeRecognition",
  cut: "overview.timeCut",
  meaning: "overview.timeMeaning",
  write: "overview.timeWrite",
  inventory: "overview.timeInventory",
};

const COUNTERS: readonly {
  state: ExplorerState;
  field: "analysed" | "toAnalyse" | "unreadable";
  one: string;
  many: string;
}[] = [
  { state: "analysed", field: "analysed", one: "workFolder.analysedOne", many: "workFolder.analysedMany" },
  { state: "toAnalyse", field: "toAnalyse", one: "explorer.toAnalyseOne", many: "explorer.toAnalyseMany" },
  { state: "unreadable", field: "unreadable", one: "workFolder.unreadableOne", many: "workFolder.unreadableMany" },
];

function CheckMark() {
  return (
    <svg className="overview__check" viewBox="0 0 16 16" aria-hidden="true">
      <path
        d="M3 8.5 L6.5 12 L13 4.5"
        fill="none"
        stroke="currentColor"
        strokeWidth="2"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </svg>
  );
}

function FamilyTag({ family }: { family: Family }) {
  const { t } = useTranslation();
  return (
    <p className={`overview__family overview__family--${family}`}>
      <FamilyFolderGlyph family={family} />
      <span>{t(FAMILY_NAME[family])}</span>
    </p>
  );
}

/** The folder's whole path, as the settings hold it. */
function FolderPath({ path }: { path: string }) {
  return (
    <p className="overview__path" title={path}>
      {path}
    </p>
  );
}

function reasonText(t: Translator, reason: CheckReason): string {
  switch (reason.kind) {
    case "error":
      return errorMessage(t, normaliseError({ code: reason.code, data: reason.data }));
    case "empty":
      return t("overview.reasonEmpty");
    case "lowConfidence":
      return t("overview.reasonLowConfidence");
    case "noTable":
      return t("overview.reasonNoTable");
    case "recognised":
      return t("overview.reasonRecognised");
  }
}

/** « 14 mars 2026 à 09:12 »: the date of a pass in the user's locale. */
function whenText(locale: string, at: number | null): string {
  return at === null
    ? ""
    : new Intl.DateTimeFormat(locale, { dateStyle: "medium", timeStyle: "short" }).format(at);
}

function Steps({
  block,
}: {
  block: Extract<OverviewBlock, { kind: "steps" }>;
}) {
  const { t } = useTranslation();
  const done = [block.chosen === 2, block.filesFound > 0, false];
  const current = done.indexOf(false);
  const files =
    block.filesFound === 0
      ? t("overview.stepFilesNone")
      : block.filesFound === 1
        ? t("overview.stepFilesOne")
        : t("overview.stepFilesMany", { count: block.filesFound });
  const steps: readonly { title: string; detail: string }[] = [
    {
      title: t("explorer.startStepChoose"),
      detail: t("overview.stepFoldersDetail", { chosen: block.chosen }),
    },
    { title: t("explorer.startStepDrop"), detail: files },
    {
      title: t("explorer.startStepAnalyse"),
      detail: block.running ? t("overview.stepAnalyseRunning") : t("overview.stepAnalyseWhat"),
    },
  ];
  return (
    <section className="overview__block" aria-label={t("overview.stepsTitle")}>
      <h3 className="overview__heading">{t("overview.stepsTitle")}</h3>
      <ol className="overview__steps">
        {steps.map((step, index) => (
          <li
            key={index}
            className={done[index] ? "overview__step overview__step--done" : "overview__step"}
            aria-current={index === current ? "step" : undefined}
          >
            <span className="overview__mark" aria-hidden="true">
              {done[index] ? <CheckMark /> : index + 1}
            </span>
            <span className="overview__step-body">
              <span className="overview__step-title">
                {step.title}
                {done[index] ? <span className="overview__sr">{t("overview.stepDone")}</span> : null}
              </span>
              <span className="overview__step-detail">{step.detail}</span>
            </span>
          </li>
        ))}
      </ol>
    </section>
  );
}

function FamilyCard({ block }: { block: Extract<OverviewBlock, { kind: "familyCard" }> }) {
  const { t } = useTranslation();
  const status = block.running
    ? t("overview.stepAnalyseRunning")
    : !block.folderChosen
      ? t("overview.cardNoFolder")
      : block.folderState === "empty"
        ? t("overview.cardEmpty")
        : block.waiting === 0
          ? t("overview.cardNothingToDo")
          : block.waiting === 1
            ? t("overview.cardWaitingOne")
            : t("overview.cardWaitingMany", { count: block.waiting });
  return (
    <article className={`overview__card overview__card--${block.family}`}>
      <FamilyTag family={block.family} />
      {block.folder === null ? null : <FolderPath path={block.folder} />}
      <p className="overview__text">{t("overview.cardFormats", { types: t(FAMILY_TYPES[block.family]) })}</p>
      <p className="overview__text overview__text--strong">{status}</p>
    </article>
  );
}

function Figures({ families }: { families: FamilyFigures[] }) {
  const { t, locale } = useTranslation();
  const number = new Intl.NumberFormat(locale);
  return (
    <section className="overview__block" aria-label={t("overview.figuresTitle")}>
      <h3 className="overview__heading">{t("overview.figuresTitle")}</h3>
      <div className="overview__cards">
        {families.map((figures) => {
          const pass = figures.lastPass;
          const when = pass === null ? "" : whenText(locale, pass.at);
          const outcomeKey =
            pass === null
              ? null
              : pass.outcome === "stopped"
                ? "overview.lastPassStopped"
                : pass.outcome === "nothing"
                  ? "overview.lastPassNothing"
                  : "overview.lastPassDone";
          const passLine =
            outcomeKey === null
              ? null
              : [
                  ...(when === "" ? [] : [t(outcomeKey, { when })]),
                  ...(pass?.durationMs == null ? [] : [durationText(t, pass.durationMs)]),
                ].join(" · ");
          return (
            <article
              key={figures.family}
              className={`overview__card overview__card--${figures.family}`}
            >
              <FamilyTag family={figures.family} />
              {figures.folder === null ? null : <FolderPath path={figures.folder} />}
              {figures.folderState === "empty" ? (
                <p className="overview__text">{t("explorer.emptyFolder")}</p>
              ) : (
                <ul className="overview__counters">
                  {COUNTERS.map(({ state, field, one, many }) => (
                    <li key={state} className="overview__counter">
                      <StateGlyph state={state} label={t(`explorer.state.${state}`)} />
                      <span>
                        {number.format(figures[field])} {pluralise(figures[field], t(one), t(many))}
                      </span>
                    </li>
                  ))}
                </ul>
              )}
              {figures.running ? (
                <p className="overview__text overview__text--strong" role="status">
                  {t("overview.running")}
                </p>
              ) : passLine === null || passLine === "" ? (
                <p className="overview__text">
                  {figures.analysed + figures.unreadable === 0
                    ? t("overview.neverAnalysed")
                    : t("overview.lastPassEarlier")}
                </p>
              ) : (
                <p className="overview__text">{passLine}</p>
              )}
              {pass?.outcome === "stopped" && !figures.running ? (
                <p className="overview__text">{t("overview.stoppedNote")}</p>
              ) : null}
              {figures.meaningPercent === null || figures.running ? null : (
                <p className="overview__text">
                  {t("workFolder.meaningShare", { percent: figures.meaningPercent })}
                </p>
              )}
              {figures.needsAnalyse && !figures.running ? (
                <p className="overview__text">{t("finder.needsAnalyse")}</p>
              ) : null}
            </article>
          );
        })}
      </div>
    </section>
  );
}

function CheckRow({
  item,
  actions,
  onFailure,
}: {
  item: CheckItem;
  actions: OverviewActions;
  onFailure: (error: AppError | null) => void;
}) {
  const { t } = useTranslation();
  const { entry } = item;
  return (
    <li className="overview__file">
      <span className="overview__file-text">
        <span className="overview__file-name">{entry.name}</span>
        {entry.folder === "" ? null : <span className="overview__file-folder">{entry.folder}</span>}
        <span className="overview__file-reason">{reasonText(t, item.reason)}</span>
      </span>
      <span className="overview__file-actions">
        {item.actions.open ? (
          <button
            type="button"
            className="button button--compact"
            title={t("viewer.openHint")}
            onClick={() => actions.onOpenFile(entry)}
          >
            {t("viewer.open")}
          </button>
        ) : null}
        {item.actions.reveal ? (
          <button
            type="button"
            className="button button--compact"
            title={t("workFolder.revealFileHint")}
            onClick={() => {
              onFailure(null);
              actions.onRevealFile(entry, onFailure);
            }}
          >
            {t("workFolder.reveal")}
          </button>
        ) : null}
      </span>
    </li>
  );
}

function Check({
  block,
  actions,
}: {
  block: Extract<OverviewBlock, { kind: "check" }>;
  actions: OverviewActions;
}) {
  const { t } = useTranslation();
  const [failure, setFailure] = useState<AppError | null>(null);
  return (
    <section className="overview__block overview__block--check" aria-label={t("overview.checkTitle")}>
      <h3 className="overview__heading">{t("overview.checkTitle")}</h3>
      <p className="overview__text">
        {block.total === 1 ? t("overview.checkOne") : t("overview.checkMany", { count: block.total })}
      </p>
      <ul className="overview__files" aria-label={t("overview.checkListLabel")}>
        {block.items.map((item) => (
          <CheckRow key={item.key} item={item} actions={actions} onFailure={setFailure} />
        ))}
      </ul>
      {block.more > 0 ? (
        <p className="overview__text">{t("workFolder.failedMore", { count: block.more })}</p>
      ) : null}
      {block.inList > 0 ? (
        <p className="overview__text">
          <button
            type="button"
            className="button button--quiet button--compact"
            onClick={actions.onShowUnreadable}
          >
            {block.inList === 1
              ? t("overview.showInListOne")
              : t("overview.showInListMany", { count: block.inList })}
          </button>
        </p>
      ) : null}
      {failure === null ? null : <ErrorBanner error={failure} />}
    </section>
  );
}

/** The heading of a kind of block: drawn once, however many families have one. */
const HEADING: Partial<Record<OverviewBlock["kind"], string>> = {
  removed: "overview.removedTitle",
  renames: "overview.renamesTitle",
  ocr: "overview.ocrTitle",
  unreadFormats: "overview.unreadTitle",
  knowledge: "overview.knowledgeTitle",
  capabilities: "overview.capabilitiesTitle",
  time: "overview.timeTitle",
};

function Detail({
  block,
  heading,
  actions,
}: {
  block: Extract<
    OverviewBlock,
    { kind: "removed" | "renames" | "ocr" | "unreadFormats" | "knowledge" | "capabilities" | "time" }
  >;
  heading: boolean;
  actions: OverviewActions;
}) {
  const { t } = useTranslation();
  let body: ReactNode;
  switch (block.kind) {
    case "removed":
      body = (
        <p className="overview__text">
          {block.family === "documents"
            ? counted(block.count, t("workFolder.removedOne"), t("workFolder.removedMany"))
            : counted(block.count, t("dataFolder.removedOne"), t("dataFolder.removedMany"))}
          .
        </p>
      );
      break;
    case "renames":
      body = (
        <>
          {block.items.length === 0 ? null : (
            <>
              <p className="overview__text">
                {counted(block.items.length, t("workFolder.renamedOne"), t("workFolder.renamedMany"))}.
              </p>
              <ul className="overview__list">
                {block.items.map((item, index) => (
                  <li key={index}>{t("workFolder.renamedItem", { from: item.from, to: item.to })}</li>
                ))}
              </ul>
            </>
          )}
          {block.failed === 0 ? null : (
            <p className="overview__text">
              {counted(block.failed, t("workFolder.renameFailedOne"), t("workFolder.renameFailedMany"))}.
            </p>
          )}
        </>
      );
      break;
    case "ocr":
      body = (
        <>
          <p className="overview__text">
            {counted(block.files.length, t("workFolder.ocrOne"), t("workFolder.ocrMany"))}.
          </p>
          <ul className="overview__list">
            {block.files.map((file, index) => (
              <li key={index}>{file}</li>
            ))}
          </ul>
        </>
      );
      break;
    case "unreadFormats":
      body = (
        <>
          <p className="overview__text">
            {counted(block.total, t("workFolder.unreadOne"), t("workFolder.unreadMany"))}.
          </p>
          <p className="overview__chips">
            {block.formats.map((format) => (
              <span
                key={format.extension}
                className="overview__chip"
                title={t("overview.unreadChipHint")}
              >
                {t("workFolder.unreadFormat", { ...format })}
              </span>
            ))}
          </p>
        </>
      );
      break;
    case "knowledge": {
      const lines = knowledgeLines(t, block.summary, block.family === "documents" ? "documents" : "data");
      if (lines.length === 0) {
        return null;
      }
      body = (
        <>
          <ul className="overview__list">
            {lines.map((line, index) => (
              <li key={index}>{line}</li>
            ))}
          </ul>
          <p className="overview__text">
            <KnowledgeLink onOpen={actions.onOpenKnowledge} />
          </p>
        </>
      );
      break;
    }
    case "capabilities":
      body = (
        <ul className="overview__list">
          {block.codes.map((code) => (
            <li key={code}>{t(CAPABILITY_KEYS[code] ?? "")}</li>
          ))}
        </ul>
      );
      break;
    case "time":
      body = (
        <>
          <p className="overview__text">{t("overview.timeTotal", { time: durationText(t, block.totalMs) })}</p>
          {block.parts.length === 0 ? null : (
            <ul className="overview__list">
              {block.parts.map((part) => (
                <li key={part.id}>
                  {t(TIME_LABEL[part.id])} : {durationText(t, part.ms)}
                </li>
              ))}
            </ul>
          )}
        </>
      );
      break;
  }
  const title = HEADING[block.kind];
  return (
    <section className="overview__block">
      {heading && title !== undefined ? <h3 className="overview__heading">{t(title)}</h3> : null}
      {block.kind === "capabilities" ? null : <FamilyTag family={block.family} />}
      {body}
    </section>
  );
}

function PassFailed({ block }: { block: Extract<OverviewBlock, { kind: "passFailed" }> }) {
  const { t } = useTranslation();
  return (
    <section className="overview__block overview__block--alert" role="alert">
      <FamilyTag family={block.family} />
      <p className="overview__text">
        {t("overview.passFailed", { reason: errorMessage(t, block.error) })}
      </p>
    </section>
  );
}

/**
 * The « Aperçu » section of the centre: the state of the workspace before the first analysis, then
 * the report of the last analysis of each family, on one page that each pass updates. It decides
 * nothing: the blocks come from `overviewModel`, the words from the catalogues, and what a button
 * does is an action given by `App`. The page's own scroll bar is the only one.
 */
export const OverviewPage = memo(function OverviewPage({
  page,
  actions,
}: {
  page: OverviewPageModel;
  actions: OverviewActions;
}) {
  const { t } = useTranslation();
  return (
    <div className="overview">
      {page.blocks.map((block, index) => {
        const previous = page.blocks[index - 1];
        const first = previous?.kind !== block.kind;
        switch (block.kind) {
          case "firstStart":
            return (
              <section key={index} className="overview__block" aria-label={t("explorer.startTitle")}>
                <h3 className="overview__heading">{t("explorer.startTitle")}</h3>
                <ol className="overview__steps">
                  {(
                    ["explorer.startStepChoose", "explorer.startStepDrop", "explorer.startStepAnalyse"] as const
                  ).map((key, step) => (
                    <li key={key} className="overview__step">
                      <span className="overview__mark" aria-hidden="true">
                        {step + 1}
                      </span>
                      <span className="overview__step-body">
                        <span className="overview__step-title">{t(key)}</span>
                      </span>
                    </li>
                  ))}
                </ol>
                <p className="overview__text">{t("overview.firstStartPointer")}</p>
              </section>
            );
          case "steps":
            return <Steps key={index} block={block} />;
          case "familyCard":
            return <FamilyCard key={index} block={block} />;
          case "figures":
            return <Figures key={index} families={block.families} />;
          case "passFailed":
            return <PassFailed key={index} block={block} />;
          case "check":
            return <Check key={index} block={block} actions={actions} />;
          default:
            return <Detail key={index} block={block} heading={first} actions={actions} />;
        }
      })}
    </div>
  );
});
