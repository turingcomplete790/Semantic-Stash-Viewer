import type { Component } from "solid-js";
import { HomeIcon, ScenesIcon } from "./icons";
import type { Route } from "./routes";

/**
 * Top-level sections in the navigation bar (constitution Principle IX; 004 FR-002, FR-003).
 *
 * Only sections that exist are listed; there are no placeholders. Later features add theirs in
 * the Stash web UI's order and with its `g` shortcuts (observed in Stash's
 * `ui/v2.5/src/components/MainNavbar.tsx`, research R7):
 *   Scenes `g s`, Images `g i`, Groups `g v`, Markers `g k`, Galleries `g l`,
 *   Performers `g p`, Studios `g u`, Tags `g t`. Settings is `g z`.
 * Home (`g h`) and the notification centre (`g n`) are the viewer's own.
 */
export type Section = {
  id: string;
  label: string;
  icon: Component<{ title?: string; class?: string }>;
  /** Two-key sequence, e.g. "g s". */
  shortcut: string;
  route: Route;
  /** Hidden while no server is connected (FR-006). */
  needsServer: boolean;
  /** Position in the navigation bar; follows the Stash web UI order. */
  order: number;
};

export const SECTIONS: readonly Section[] = [
  {
    id: "home",
    label: "Home",
    icon: HomeIcon,
    shortcut: "g h",
    route: { kind: "home" },
    needsServer: true,
    order: 0,
  },
  {
    id: "scenes",
    label: "Scenes",
    icon: ScenesIcon,
    shortcut: "g s",
    route: { kind: "scenes" },
    needsServer: true,
    order: 10,
  },
];

/** The section a route belongs to, for highlighting (FR-004). */
export function sectionFor(route: Route): Section | undefined {
  switch (route.kind) {
    case "home":
      return SECTIONS.find((s) => s.id === "home");
    case "scenes":
    case "scene":
      return SECTIONS.find((s) => s.id === "scenes");
    case "settings":
      return undefined;
  }
}
