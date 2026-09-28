import type { Component } from "solid-js";
import { HomeIcon, ScenesIcon, SettingsIcon } from "./icons";

/** Settings pages (004 FR-024). */
export const SETTINGS_PAGES = ["servers", "keyboard", "troubleshooting", "about"] as const;
export type SettingsPage = (typeof SETTINGS_PAGES)[number];

/**
 * A view a tab can show (004 research R1). Owned by the UI; the core stores routes as opaque
 * JSON. Later features add variants (images, gallery, …).
 */
export type Route =
  | { kind: "home" }
  | { kind: "scenes" }
  | { kind: "scene"; sceneId: string; title: string }
  | { kind: "settings"; page: SettingsPage };

export function routeTitle(route: Route): string {
  switch (route.kind) {
    case "home":
      return "Home";
    case "scenes":
      return "Scenes";
    case "scene":
      return route.title.trim() || `Scene ${route.sceneId}`;
    case "settings":
      return "Settings";
  }
}

export function routeIcon(route: Route): Component<{ title?: string; class?: string }> {
  switch (route.kind) {
    case "home":
      return HomeIcon;
    case "scenes":
    case "scene":
      return ScenesIcon;
    case "settings":
      return SettingsIcon;
  }
}

/** Validates data restored from disk, which may come from another version of the viewer. */
export function isRoute(value: unknown): value is Route {
  if (typeof value !== "object" || value === null) return false;
  const v = value as Record<string, unknown>;
  switch (v.kind) {
    case "home":
    case "scenes":
      return true;
    case "scene":
      return typeof v.sceneId === "string" && v.sceneId !== "" && typeof v.title === "string";
    case "settings":
      return SETTINGS_PAGES.includes(v.page as SettingsPage);
    default:
      return false;
  }
}

/** True when both routes show the same thing (titles may differ). */
export function sameView(a: Route, b: Route): boolean {
  if (a.kind !== b.kind) return false;
  if (a.kind === "scene" && b.kind === "scene") return a.sceneId === b.sceneId;
  if (a.kind === "settings" && b.kind === "settings") return a.page === b.page;
  return true;
}
