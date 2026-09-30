import { commands } from "../bindings";

let marked = false;

/**
 * Tell the core the app is interactive: the first painted frame with server info (live or
 * cached). Debug builds print the cold start time for the performance harness (003 research
 * R8); release builds ignore it. Only the first call counts.
 */
export function markInteractive(): void {
  if (marked) return;
  marked = true;
  requestAnimationFrame(() =>
    requestAnimationFrame(() => {
      void commands.debugMarkInteractive(document.visibilityState === "visible");
    }),
  );
}
