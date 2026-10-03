import { register } from "../shell/keymap";

/**
 * Keyboard navigation in a page of scenes (005 research R8, FR-006). The grid handles these keys
 * itself while a card has focus (roving focus: one card is focusable at a time); the registry
 * entries below only list them in the `?` overlay and Settings → Keyboard.
 */

export type GridKeyContext = {
  /** The focused card's position on the page (0-based). */
  index: number;
  /** Cards on this page. */
  count: number;
  cols: number;
  ctrl: boolean;
};

export type PageDirection = "next" | "previous";

export type GridKeyAction =
  | { move: number }
  | { edge: PageDirection }
  | { page: PageDirection }
  | { open: number; newTab: boolean }
  | null;

/** What a key does on a page: move focus, cross to a neighbouring page, change page, or open. */
export function nextIndex(key: string, c: GridKeyContext): GridKeyAction {
  const last = Math.max(0, c.count - 1);
  const step = (delta: number): GridKeyAction => {
    const target = c.index + delta;
    if (target > last) return { edge: "next" };
    if (target < 0) return { edge: "previous" };
    return { move: target };
  };
  switch (key) {
    case "ArrowRight":
      return step(1);
    case "ArrowLeft":
      return step(-1);
    case "ArrowDown":
      return step(c.cols);
    case "ArrowUp":
      return step(-c.cols);
    case "]":
      return { page: "next" };
    case "[":
      return { page: "previous" };
    case "Home":
      return { move: 0 };
    case "End":
      return { move: last };
    case "Enter":
      return { open: c.index, newTab: c.ctrl };
    default:
      return null;
  }
}

/** List the grid's keys (display-only). Returns a function that removes them. */
export function registerSceneGridKeys(): () => void {
  const entries = [
    {
      id: "scenes-grid.move",
      keys: ["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown"],
      description: "Move between scenes",
    },
    { id: "scenes-grid.next-page", keys: ["]"], description: "Next page" },
    { id: "scenes-grid.previous-page", keys: ["["], description: "Previous page" },
    {
      id: "scenes-grid.ends",
      keys: ["Home", "End"],
      description: "Go to the first or last scene on the page",
    },
    { id: "scenes-grid.open", keys: ["Enter"], description: "Open the scene" },
    { id: "scenes-grid.open-tab", keys: ["Ctrl+Enter"], description: "Open in a new tab" },
  ];
  const removers = entries.map((e) => register({ ...e, scope: "scenes-grid" }));
  return () => removers.forEach((remove) => remove());
}
