import {
  useEffect,
  useId,
  useRef,
  useState,
  type CSSProperties,
  type KeyboardEvent,
  type Ref,
} from "react";
import { useTranslation } from "../i18n/I18nProvider";
import { menuPlacement, nextMenuIndex, type MenuPlacement } from "../lib/explorerMenu";

export interface FamilyMenuItem {
  id: string;
  label: string;
  onSelect: () => void;
}

/** The entries of the open menu, drawn on their own so that a test can read them. */
export function ExplorerMenuList({
  id,
  label,
  items,
  placement,
  listRef,
  onSelect,
  onKeyDown,
}: {
  id: string;
  label: string;
  items: readonly FamilyMenuItem[];
  placement: MenuPlacement | null;
  listRef?: Ref<HTMLDivElement>;
  onSelect: (item: FamilyMenuItem) => void;
  onKeyDown?: (event: KeyboardEvent<HTMLDivElement>) => void;
}) {
  const style: CSSProperties | undefined =
    placement === null ? undefined : { top: placement.top, right: placement.right };
  return (
    <div
      ref={listRef}
      id={id}
      className="explorer-menu"
      role="menu"
      aria-label={label}
      style={style}
      onKeyDown={onKeyDown}
    >
      {items.map((item) => (
        <button
          key={item.id}
          type="button"
          role="menuitem"
          className="explorer-menu__item"
          onClick={() => onSelect(item)}
        >
          {item.label}
        </button>
      ))}
    </div>
  );
}

/**
 * The « ⋯ » button of a family and its menu. The menu is closed by a click anywhere else, by Escape
 * (the focus goes back to the button) and by leaving it with the keyboard; the arrow keys, Home and
 * End move between its entries. It sits in fixed coordinates so the box's clipping never cuts it.
 * Absent altogether while the family is being analysed: the caller does not draw it.
 */
export function ExplorerFamilyMenu({
  label,
  items,
}: {
  /** What a screen reader says for the button and for the menu: it names the family. */
  label: string;
  items: readonly FamilyMenuItem[];
}) {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  const [placement, setPlacement] = useState<MenuPlacement | null>(null);
  const button = useRef<HTMLButtonElement>(null);
  const list = useRef<HTMLDivElement>(null);
  const menuId = useId();

  const close = (returnFocus: boolean) => {
    setOpen(false);
    if (returnFocus) {
      button.current?.focus();
    }
  };
  const toggle = () => {
    if (open) {
      close(false);
      return;
    }
    const rect = button.current?.getBoundingClientRect();
    if (rect !== undefined) {
      setPlacement(
        menuPlacement(rect, { width: window.innerWidth, height: window.innerHeight }, items.length),
      );
    }
    setOpen(true);
  };

  const entries = () =>
    Array.from(list.current?.querySelectorAll<HTMLElement>("[role=menuitem]") ?? []);
  /* The first entry takes the focus when the menu opens, so the keyboard can use it at once. */
  useEffect(() => {
    if (open) {
      entries()[0]?.focus();
    }
  }, [open]);

  useEffect(() => {
    if (!open) {
      return;
    }
    const onKey = (event: globalThis.KeyboardEvent) => {
      if (event.key === "Escape") {
        close(true);
      }
    };
    const onPointer = (event: MouseEvent) => {
      const target = event.target as Node;
      if (!list.current?.contains(target) && !button.current?.contains(target)) {
        close(false);
      }
    };
    const onMove = () => close(false);
    document.addEventListener("keydown", onKey);
    document.addEventListener("mousedown", onPointer);
    window.addEventListener("resize", onMove);
    window.addEventListener("scroll", onMove, true);
    return () => {
      document.removeEventListener("keydown", onKey);
      document.removeEventListener("mousedown", onPointer);
      window.removeEventListener("resize", onMove);
      window.removeEventListener("scroll", onMove, true);
    };
  }, [open]);

  const onListKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    const all = entries();
    const next = nextMenuIndex(
      all.indexOf(document.activeElement as HTMLElement),
      all.length,
      event.key,
    );
    if (next !== null) {
      event.preventDefault();
      all[next]?.focus();
    } else if (event.key === "Tab") {
      close(false);
    }
  };

  return (
    <>
      <button
        ref={button}
        type="button"
        className="explorer-family__menu"
        aria-haspopup="menu"
        aria-expanded={open}
        aria-controls={open ? menuId : undefined}
        aria-label={label}
        title={t("explorer.menuHint")}
        onClick={toggle}
      >
        <svg viewBox="0 0 20 20" aria-hidden="true" className="menu-glyph">
          <circle cx="4.5" cy="10" r="1.5" fill="currentColor" />
          <circle cx="10" cy="10" r="1.5" fill="currentColor" />
          <circle cx="15.5" cy="10" r="1.5" fill="currentColor" />
        </svg>
      </button>
      {open ? (
        <ExplorerMenuList
          id={menuId}
          label={label}
          items={items}
          placement={placement}
          listRef={list}
          onKeyDown={onListKeyDown}
          onSelect={(item) => {
            close(true);
            item.onSelect();
          }}
        />
      ) : null}
    </>
  );
}
