import { createSignal } from "solid-js";
import type { Route } from "./routes";

/**
 * Navigation state for the shell. Foundational version: one current route. US2 turns this into
 * tabs with per-tab history (004 research R1) behind the same `currentRoute`/`navigate` API.
 *
 * `currentRoute` may hold data that isn't a valid route (restored from another viewer version);
 * the shell checks it with `isRoute` before rendering.
 */
const HOME: Route = { kind: "home" };
const [route, setRoute] = createSignal<Route>(HOME);

export function currentRoute(): Route {
  return route();
}

export function navigate(next: Route): void {
  setRoute(next);
}

/** Back to a single Home view (tests, and after leaving a server). */
export function resetTabs(): void {
  setRoute(HOME);
}
