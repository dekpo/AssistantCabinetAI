import { useTranslation } from "../../i18n/I18nProvider";
import { counted } from "../../lib/plural";

/** What the mounted document lets the application offer: nothing is invented, a control exists only
 * where the library handle has the matching member. */
export interface ViewerCapabilities {
  canZoom: boolean;
  canFit: boolean;
  pages: number | null;
  sheets: string[] | null;
  /** How many slides a presentation has, when it is shown one slide at a time. */
  slides: number | null;
}

/**
 * The application's own controls, in the interface language. The library draws a toolbar of its
 * own with labels in one fixed language; it is hidden and these replace it.
 */
export function ViewerControls({
  capabilities,
  zoom,
  sheetIndex,
  slideIndex,
  currentPage,
  onZoomIn,
  onZoomOut,
  onFit,
  onSheet,
  onSlide,
}: {
  capabilities: ViewerCapabilities;
  zoom: number | null;
  sheetIndex: number;
  slideIndex: number;
  /** The page being read, 1-based. */
  currentPage: number;
  onZoomIn: () => void;
  onZoomOut: () => void;
  onFit: () => void;
  onSheet: (index: number) => void;
  /** -1 for the previous slide, 1 for the next one. */
  onSlide: (step: -1 | 1) => void;
}) {
  const { t } = useTranslation();
  const { canZoom, canFit, pages, sheets, slides } = capabilities;
  if (!canZoom && !canFit && pages === null && sheets === null && slides === null) {
    return null;
  }

  return (
    <div className="viewer__controls" role="toolbar" aria-label={t("viewer.controls")}>
      {slides === null ? null : (
        <>
          <button
            type="button"
            className="button button--compact"
            disabled={slideIndex <= 0}
            onClick={() => onSlide(-1)}
          >
            {t("viewer.previousSlide")}
          </button>
          <span className="viewer__pages">
            {t("viewer.slideOf", { index: slideIndex + 1, count: slides })}
          </span>
          <button
            type="button"
            className="button button--compact"
            disabled={slideIndex >= slides - 1}
            onClick={() => onSlide(1)}
          >
            {t("viewer.nextSlide")}
          </button>
        </>
      )}
      {canZoom ? (
        <>
          <button
            type="button"
            className="button button--compact"
            title={t("viewer.zoomOut")}
            aria-label={t("viewer.zoomOut")}
            onClick={onZoomOut}
          >
            -
          </button>
          {zoom === null ? null : (
            <span className="viewer__zoom">
              {t("viewer.zoomLevel", { percent: Math.round(zoom * 100) })}
            </span>
          )}
          <button
            type="button"
            className="button button--compact"
            title={t("viewer.zoomIn")}
            aria-label={t("viewer.zoomIn")}
            onClick={onZoomIn}
          >
            +
          </button>
        </>
      ) : null}
      {canFit ? (
        <button type="button" className="button button--compact" onClick={onFit}>
          {t("viewer.fitWidth")}
        </button>
      ) : null}
      {sheets === null ? null : (
        <label className="viewer__sheet">
          <span>{t("viewer.sheet")}</span>
          <select value={sheetIndex} onChange={(event) => onSheet(Number(event.target.value))}>
            {sheets.map((name, index) => (
              <option key={`${index}-${name}`} value={index}>
                {name}
              </option>
            ))}
          </select>
        </label>
      )}
      {pages === null ? null : (
        <span className="viewer__pages">
          {pages > 1
            ? t("viewer.pageOf", { current: Math.min(currentPage, pages), total: pages })
            : counted(pages, t("viewer.pageOne"), t("viewer.pageMany"))}
        </span>
      )}
    </div>
  );
}
