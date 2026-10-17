import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "../../i18n/I18nProvider";
import {
  mountDocument,
  visiblePage,
  type MountedViewer,
  type ViewerFormat,
} from "../../lib/viewerEngine";
import { ViewerControls, type ViewerCapabilities } from "./ViewerControls";

/** The wait after the last resize before the width is fitted again. */
const REFIT_DELAY_MS = 150;

/** Pages and pictures are fitted to the width, on open and after a resize. A grid is not: fitted to
 * a wide zone it opens at 200 % and more, which shows fewer cells than at its natural size. */
const FITS_TO_WIDTH: ReadonlySet<ViewerFormat> = new Set(["docx", "odt", "odp", "rtf", "image"]);

/** The step of a zoom button for a handle that has `setZoom` but no `zoomIn` / `zoomOut`. */
const ZOOM_STEP = 1.25;
const ZOOM_MIN = 0.1;
const ZOOM_MAX = 8;
/** Measured in the real window: a picture grows with its zoom up to 150 % and not beyond, the
 * number would only keep climbing. A picture already shown larger (a wide zone) keeps its size. */
const IMAGE_ZOOM_MAX = 1.5;

/**
 * Draws one file with the library, through the single adapter. The library is plain JavaScript that
 * fills an element, so this component only owns that element and the handle it returns:
 * - it mounts once the element has a width (the centre is 0 px wide at its first render and gets
 *   its width one frame later) and destroys the handle on every replace and on unmount, so a fast
 *   file switch or React's double effect leaves no second viewer behind;
 * - it passes a copy of the bytes, because the library empties the buffer it receives;
 * - after a resize it fits the width again, unless the user chose a zoom (the PDF refits itself).
 */
export function DocumentViewer({ bytes, format }: { bytes: ArrayBuffer; format: ViewerFormat }) {
  const { t } = useTranslation();
  const hostRef = useRef<HTMLDivElement>(null);
  const viewerRef = useRef<MountedViewer | null>(null);
  /* Set by the user's own zoom buttons: from then on a resize does not undo her choice. */
  const userZoomed = useRef(false);
  const [sheetIndex, setSheetIndex] = useState(0);
  const [slideIndex, setSlideIndex] = useState(0);
  const [currentPage, setCurrentPage] = useState(1);
  /* Set while a paged document is on screen: reads the page again (after a zoom, which moves the pages
     without a scroll). */
  const refreshPage = useRef<() => void>(() => undefined);
  const [capabilities, setCapabilities] = useState<ViewerCapabilities | null>(null);
  const [zoom, setZoom] = useState<number | null>(null);
  const [failed, setFailed] = useState(false);

  useEffect(() => {
    const host = hostRef.current;
    if (host === null) {
      return;
    }
    let cancelled = false;
    let started = false;
    let timer: number | undefined;
    userZoomed.current = false;
    setSheetIndex(0);
    setSlideIndex(0);
    setCurrentPage(1);
    setCapabilities(null);
    setFailed(false);

    const start = () => {
      started = true;
      mountDocument(host, bytes.slice(0), {
        format,
        height: "100%",
        useWorker: true,
        onZoom: (value) => setZoom(value),
      })
        .then((mounted) => {
          if (cancelled) {
            mounted.destroy();
            return;
          }
          viewerRef.current = mounted;
          if (FITS_TO_WIDTH.has(format)) {
            mounted.fitWidth?.();
          }
          setZoom(mounted.zoom ?? null);
          setCapabilities({
            canZoom:
              (typeof mounted.zoomIn === "function" && typeof mounted.zoomOut === "function") ||
              (typeof mounted.setZoom === "function" && typeof mounted.zoom === "number"),
            canFit: typeof mounted.fitWidth === "function",
            pages: typeof mounted.pageCount === "number" ? mounted.pageCount : null,
            sheets:
              Array.isArray(mounted.sheetNames) && mounted.sheetNames.length > 1
                ? mounted.sheetNames
                : null,
            slides:
              typeof mounted.slideCount === "number" &&
              mounted.slideCount > 0 &&
              typeof mounted.next === "function" &&
              typeof mounted.prev === "function"
                ? mounted.slideCount
                : null,
          });
        })
        .catch(() => {
          if (!cancelled) {
            setFailed(true);
          }
        });
    };

    const observer = new ResizeObserver(() => {
      if (!started) {
        if (host.clientWidth > 0) {
          start();
        }
        return;
      }
      if (!FITS_TO_WIDTH.has(format) || userZoomed.current) {
        return;
      }
      window.clearTimeout(timer);
      timer = window.setTimeout(() => viewerRef.current?.fitWidth?.(), REFIT_DELAY_MS);
    });
    observer.observe(host);

    return () => {
      cancelled = true;
      observer.disconnect();
      window.clearTimeout(timer);
      viewerRef.current?.destroy();
      viewerRef.current = null;
      host.replaceChildren();
    };
  }, [bytes, format]);

  const pages = capabilities?.pages ?? null;
  /* The page number of a long document follows the scroll: the library does not report it, so it is
     read from where the pages sit, at most once per frame. */
  useEffect(() => {
    const host = hostRef.current;
    if (host === null || pages === null || pages < 2) {
      return;
    }
    let frame = 0;
    const update = () => {
      frame = 0;
      const page = visiblePage(host);
      if (page !== null) {
        setCurrentPage(page);
      }
    };
    const schedule = () => {
      if (frame === 0) {
        frame = requestAnimationFrame(update);
      }
    };
    /* A scroll does not bubble, so it is listened to while it goes down to the scrolling element. */
    host.addEventListener("scroll", schedule, true);
    refreshPage.current = schedule;
    schedule();
    return () => {
      host.removeEventListener("scroll", schedule, true);
      cancelAnimationFrame(frame);
      refreshPage.current = () => undefined;
    };
  }, [pages]);

  const refreshZoom = () => {
    setZoom(viewerRef.current?.zoom ?? null);
    /* The pages move once the library has applied the zoom. */
    window.setTimeout(() => refreshPage.current(), 150);
  };
  /* Through the handle's own step when it has one, otherwise one step of ours through `setZoom`. */
  const zoomBy = (direction: 1 | -1) => {
    const viewer = viewerRef.current;
    if (viewer === null) {
      return;
    }
    userZoomed.current = true;
    const own = direction === 1 ? viewer.zoomIn : viewer.zoomOut;
    if (own !== undefined) {
      own.call(viewer);
    } else if (viewer.setZoom !== undefined && typeof viewer.zoom === "number") {
      const next = direction === 1 ? viewer.zoom * ZOOM_STEP : viewer.zoom / ZOOM_STEP;
      const ceiling = format === "image" ? Math.max(IMAGE_ZOOM_MAX, viewer.zoom) : ZOOM_MAX;
      viewer.setZoom(Math.min(ceiling, Math.max(ZOOM_MIN, next)));
    }
    refreshZoom();
  };
  const zoomIn = () => zoomBy(1);
  const zoomOut = () => zoomBy(-1);
  const chooseSheet = (index: number) => {
    setSheetIndex(index);
    viewerRef.current?.setSheet?.(index);
  };
  const moveSlide = (step: -1 | 1) => {
    const viewer = viewerRef.current;
    if (step === 1) {
      viewer?.next?.();
    } else {
      viewer?.prev?.();
    }
    setSlideIndex(viewer?.slideIndex ?? 0);
  };
  const fit = useCallback(() => {
    userZoomed.current = false;
    viewerRef.current?.fitWidth?.();
    refreshZoom();
  }, []);

  return (
    <div className="viewer">
      {capabilities === null ? null : (
        <ViewerControls
          capabilities={capabilities}
          zoom={zoom}
          sheetIndex={sheetIndex}
          slideIndex={slideIndex}
          currentPage={currentPage}
          onZoomIn={zoomIn}
          onZoomOut={zoomOut}
          onFit={fit}
          onSheet={chooseSheet}
          onSlide={moveSlide}
        />
      )}
      <div className="viewer__body">
        <div ref={hostRef} className="viewer__host" />
        {failed ? (
          <p className="viewer__message" role="alert">
            {t("errors.viewer_render_failed")}
          </p>
        ) : capabilities === null ? (
          <p className="viewer__message" role="status">
            {t("viewer.loading")}
          </p>
        ) : null}
      </div>
    </div>
  );
}
