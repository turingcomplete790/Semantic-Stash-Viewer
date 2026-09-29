import { createSignal } from "solid-js";
import { commands } from "../bindings";
import type { TabSet } from "../bindings";
import { player } from "../player/state";
import { register } from "./keymap";
import { sameView } from "./routes";
import type { Route } from "./routes";

/**
 * Tabs with per-tab history (004 US2; research R1–R2; data-model "Tab", "TabSet").
 *
 * Routes restored from disk may not be valid routes (another viewer version); they're kept as-is
 * and the shell shows them as "unavailable" (`isRoute` in RouteView).
 */
export type HistoryEntry = { route: Route; viewState: Record<string, unknown> | null };
export type TabState = { id: string; history: HistoryEntry[]; index: number };

const MAX_HISTORY = 50;
const MAX_CLOSED = 10;
const VIEW_STATE_SAVE_DELAY_MS = 500;
const HOME: Route = { kind: "home" };

function newId(): string {
  return typeof crypto !== "undefined" && "randomUUID" in crypto
    ? crypto.randomUUID()
    : `tab-${Date.now()}-${Math.random().toString(36).slice(2)}`;
}

function homeTab(): TabState {
  return { id: newId(), history: [{ route: HOME, viewState: null }], index: 0 };
}

const first = homeTab();
const [tabList, setTabList] = createSignal<TabState[]>([first]);
const [selected, setSelected] = createSignal<string>(first.id);
let closedTabs: TabState[] = [];
/** Tabs loaded from disk this session; a restored scene tab waits for Play (research R10). */
const restoredIds = new Set<string>();
let profileId: string | null = null;
let saveTimer: ReturnType<typeof setTimeout> | undefined;

// ---- Reading ----

export function tabs(): TabState[] {
  return tabList();
}

export function selectedId(): string {
  return selected();
}

export function tabById(id: string): TabState | undefined {
  return tabList().find((t) => t.id === id);
}

export function selectedTab(): TabState {
  return tabById(selected()) ?? tabList()[0];
}

export function routeOf(tab: TabState): Route {
  return tab.history[tab.index].route;
}

export function currentRoute(): Route {
  return routeOf(selectedTab());
}

export function canGoBack(): boolean {
  return selectedTab().index > 0;
}

export function canGoForward(): boolean {
  const t = selectedTab();
  return t.index < t.history.length - 1;
}

// ---- Persistence ----

function toTabSet(): TabSet {
  return {
    tabs: tabList().map((t) => ({
      id: t.id,
      history: t.history.map((e) => ({ route: e.route, viewState: e.viewState })),
      index: t.index,
    })),
    selectedTabId: selected(),
  };
}

let savingSuspended = false;

/** Debug bench only: don't persist tab changes (so a bench run leaves saved tabs alone). */
export function suspendSaving(suspended: boolean): void {
  savingSuspended = suspended;
}

function saveNow(): void {
  clearTimeout(saveTimer);
  saveTimer = undefined;
  if (profileId && !savingSuspended) void commands.shellSaveTabs(profileId, toTabSet());
}

function saveSoon(): void {
  clearTimeout(saveTimer);
  saveTimer = setTimeout(saveNow, VIEW_STATE_SAVE_DELAY_MS);
}

/** Write any pending view-state save right away (before switching servers or quitting). */
export function flushSave(): void {
  if (saveTimer !== undefined) saveNow();
}

/** Switch to a profile's tabs: save the current profile's first, then load (FR-014). */
export async function loadTabsFor(id: string): Promise<void> {
  if (profileId && profileId !== id) saveNow();
  else flushSave();
  profileId = id;
  closedTabs = [];
  const saved = await commands.shellLoadTabs(id);
  if (!saved || saved.tabs.length === 0) {
    // Keep an untouched Home tab as it is, so the view already on screen doesn't remount.
    const list = tabList();
    const pristine =
      list.length === 1 && list[0].history.length === 1 && list[0].history[0].route.kind === "home";
    if (!pristine) {
      const t = homeTab();
      setTabList([t]);
      setSelected(t.id);
    }
    saveNow();
    return;
  }
  const restored: TabState[] = saved.tabs.map((t) => ({
    id: t.id,
    history: t.history.map((e) => ({
      route: e.route as Route,
      viewState: (e.viewState as Record<string, unknown> | null) ?? null,
    })),
    index: Math.min(t.index, t.history.length - 1),
  }));
  restoredIds.clear();
  restored.forEach((t) => restoredIds.add(t.id));
  setTabList(restored);
  setSelected(
    restored.some((t) => t.id === saved.selectedTabId) ? saved.selectedTabId : restored[0].id,
  );
}

/** True for a tab restored from disk that hasn't navigated since. */
export function wasRestored(tabId: string): boolean {
  return restoredIds.has(tabId);
}

/** Back to a single Home tab with no profile (disconnect, tests). */
export function resetTabs(): void {
  flushSave();
  profileId = null;
  closedTabs = [];
  restoredIds.clear();
  const t = homeTab();
  setTabList([t]);
  setSelected(t.id);
}

// ---- Changing ----

function updateTab(id: string, change: (t: TabState) => TabState): void {
  setTabList((list) => list.map((t) => (t.id === id ? change(t) : t)));
}

/** Show `route` in the current tab, or in a new tab after it (FR-009). */
export function navigate(route: Route, options: { newTab?: boolean } = {}): void {
  if (options.newTab) {
    const tab: TabState = { id: newId(), history: [{ route, viewState: null }], index: 0 };
    setTabList((list) => {
      const at = list.findIndex((t) => t.id === selected());
      const next = [...list];
      next.splice(at + 1, 0, tab);
      return next;
    });
    setSelected(tab.id);
    saveNow();
    return;
  }
  navigateIn(selected(), route);
}

/** Show `route` in a specific tab (e.g. a scene view leaving itself while in the background). */
export function navigateIn(tabId: string, route: Route): void {
  const tab = tabById(tabId);
  if (!tab || sameView(routeOf(tab), route)) return;
  restoredIds.delete(tabId);
  updateTab(tabId, (t) => {
    let history = [...t.history.slice(0, t.index + 1), { route, viewState: null }];
    if (history.length > MAX_HISTORY) history = history.slice(history.length - MAX_HISTORY);
    return { ...t, history, index: history.length - 1 };
  });
  saveNow();
}

export function back(): void {
  const t = selectedTab();
  if (t.index === 0) return;
  updateTab(t.id, (x) => ({ ...x, index: x.index - 1 }));
  saveNow();
}

export function forward(): void {
  const t = selectedTab();
  if (t.index >= t.history.length - 1) return;
  updateTab(t.id, (x) => ({ ...x, index: x.index + 1 }));
  saveNow();
}

/** Select a tab by id, or by position (0-based; -1 is the last tab). */
export function select(target: string | number): void {
  const list = tabList();
  const tab =
    typeof target === "string"
      ? list.find((t) => t.id === target)
      : target === -1
        ? list[list.length - 1]
        : list[target];
  if (!tab || tab.id === selected()) return;
  setSelected(tab.id);
  saveNow();
}

/** Select the tab `step` places away, wrapping around. */
export function cycle(step: number): void {
  const list = tabList();
  const at = list.findIndex((t) => t.id === selected());
  select(list[(at + step + list.length) % list.length].id);
}

/** True while `tab` shows the scene loaded in the player. */
export function isPlayingTab(tab: TabState): boolean {
  const route = routeOf(tab);
  const snap = player.snapshot();
  return player.isOpen() && route.kind === "scene" && route.sceneId === snap.sceneId;
}

/** Close a tab. Closing the playing scene's tab stops playback; the window is never empty. */
export function close(id: string): void {
  const list = tabList();
  const at = list.findIndex((t) => t.id === id);
  if (at < 0) return;
  const tab = list[at];
  if (isPlayingTab(tab)) void commands.playerClose();
  closedTabs = [tab, ...closedTabs].slice(0, MAX_CLOSED);
  const rest = list.filter((t) => t.id !== id);
  if (rest.length === 0) {
    const home = homeTab();
    setTabList([home]);
    setSelected(home.id);
  } else {
    setTabList(rest);
    if (selected() === id) setSelected((rest[at] ?? rest[at - 1]).id);
  }
  saveNow();
}

/** Reopen the most recently closed tab (session only, up to 10). */
export function reopenClosed(): void {
  const [tab, ...rest] = closedTabs;
  if (!tab) return;
  closedTabs = rest;
  setTabList((list) => {
    const at = list.findIndex((t) => t.id === selected());
    const next = [...list];
    next.splice(at + 1, 0, tab);
    return next;
  });
  setSelected(tab.id);
  saveNow();
}

export function move(from: number, to: number): void {
  setTabList((list) => {
    if (from < 0 || from >= list.length || to < 0 || to >= list.length) return list;
    const next = [...list];
    const [tab] = next.splice(from, 1);
    next.splice(to, 0, tab);
    return next;
  });
  saveNow();
}

/** Merge `patch` into a tab's current history entry's view state (saved after 500 ms). */
export function setViewState(tabId: string, patch: Record<string, unknown>): void {
  updateTab(tabId, (t) => {
    const history = t.history.map((e, i) =>
      i === t.index ? { ...e, viewState: { ...(e.viewState ?? {}), ...patch } } : e,
    );
    return { ...t, history };
  });
  saveSoon();
}

export function viewStateOf(tabId: string): Record<string, unknown> {
  const t = tabById(tabId);
  return (t && t.history[t.index].viewState) ?? {};
}

// ---- Shortcuts (research R7) ----

/** Register the tab and history shortcuts; returns an unregister function. */
export function registerTabShortcuts(): () => void {
  const shell = "shell" as const;
  const removers = [
    register({
      id: "tab:new",
      keys: ["Ctrl+T"],
      scope: shell,
      description: "New tab",
      run: () => navigate(HOME, { newTab: true }),
    }),
    register({
      id: "tab:close",
      keys: ["Ctrl+W"],
      scope: shell,
      description: "Close tab",
      run: () => close(selected()),
    }),
    register({
      id: "tab:reopen",
      keys: ["Ctrl+Shift+T"],
      scope: shell,
      description: "Reopen closed tab",
      run: reopenClosed,
    }),
    register({
      id: "tab:next",
      keys: ["Ctrl+Tab", "Ctrl+PageDown"],
      scope: shell,
      description: "Next tab",
      run: () => cycle(1),
    }),
    register({
      id: "tab:previous",
      keys: ["Ctrl+Shift+Tab", "Ctrl+PageUp"],
      scope: shell,
      description: "Previous tab",
      run: () => cycle(-1),
    }),
    register({
      id: "tab:number",
      keys: ["Ctrl+1", "Ctrl+2", "Ctrl+3", "Ctrl+4", "Ctrl+5", "Ctrl+6", "Ctrl+7", "Ctrl+8"],
      scope: shell,
      description: "Go to tab 1–8",
    }),
    register({
      id: "tab:last",
      keys: ["Ctrl+9"],
      scope: shell,
      description: "Go to the last tab",
      run: () => select(-1),
    }),
    register({
      id: "history:back",
      keys: ["Alt+ArrowLeft"],
      scope: shell,
      description: "Back",
      run: back,
    }),
    register({
      id: "history:forward",
      keys: ["Alt+ArrowRight"],
      scope: shell,
      description: "Forward",
      run: forward,
    }),
    ...Array.from({ length: 8 }, (_, i) =>
      register({
        id: `tab:${i + 1}`,
        keys: [`Ctrl+${i + 1}`],
        scope: shell,
        description: "",
        run: () => select(i),
      }),
    ),
  ];
  return () => removers.forEach((remove) => remove());
}

/** Mouse back/forward buttons (3 and 4) walk the current tab's history. */
export function installMouseNavigation(): () => void {
  const onMouseUp = (e: MouseEvent) => {
    if (e.button === 3) {
      e.preventDefault();
      back();
    } else if (e.button === 4) {
      e.preventDefault();
      forward();
    }
  };
  window.addEventListener("mouseup", onMouseUp);
  return () => window.removeEventListener("mouseup", onMouseUp);
}
