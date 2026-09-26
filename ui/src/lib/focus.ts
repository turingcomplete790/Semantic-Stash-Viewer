import { onCleanup, onMount } from "solid-js";

const FOCUSABLE =
  'button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), a[href], [tabindex]:not([tabindex="-1"])';

/**
 * Keep keyboard focus inside a dialog while it's open (Tab / Shift+Tab wrap around), focus the
 * first control on open, and give focus back to whatever had it before on close.
 *
 * `container` is read on every Tab press, so a nested dialog (e.g. a confirmation inside a
 * manager) can take over by returning its own element.
 */
export function useFocusTrap(container: () => HTMLElement | undefined): void {
  const previous = document.activeElement as HTMLElement | null;

  const onKeyDown = (e: KeyboardEvent) => {
    if (e.key !== "Tab") return;
    const root = container();
    if (!root) return;
    const items = Array.from(root.querySelectorAll<HTMLElement>(FOCUSABLE));
    if (items.length === 0) return;
    const first = items[0];
    const last = items[items.length - 1];
    const active = document.activeElement as HTMLElement | null;
    if (e.shiftKey && (active === first || !root.contains(active))) {
      e.preventDefault();
      last.focus();
    } else if (!e.shiftKey && (active === last || !root.contains(active))) {
      e.preventDefault();
      first.focus();
    }
  };

  onMount(() => {
    const root = container();
    if (root && !root.contains(document.activeElement)) {
      root.querySelector<HTMLElement>(FOCUSABLE)?.focus();
    }
    document.addEventListener("keydown", onKeyDown);
  });
  onCleanup(() => {
    document.removeEventListener("keydown", onKeyDown);
    if (previous && document.contains(previous)) previous.focus();
  });
}
