import { createContext, useContext } from "solid-js";
import type { Accessor } from "solid-js";
import { setViewState, viewStateOf } from "./tabs";

/** Which tab a view is rendered in, and whether that tab is showing (004 research R2). */
export type TabContextValue = { tabId: string; isActive: Accessor<boolean> };

export const TabContext = createContext<TabContextValue>();

/** The surrounding tab, or a stand-in for views rendered outside tabs (tests, disconnected). */
export function useTab(): TabContextValue {
  return useContext(TabContext) ?? { tabId: "", isActive: () => true };
}

/**
 * Restorable state for the current view, stored on its tab's history entry (so it survives tab
 * switches, unmounting, back/forward, and relaunch). Keep it small: at most 16 KB per entry.
 */
export function useViewState<T>(key: string, initial: T): [Accessor<T>, (value: T) => void] {
  const { tabId } = useTab();
  const read = (): T => {
    if (!tabId) return initial;
    const stored = viewStateOf(tabId)[key];
    return stored === undefined ? initial : (stored as T);
  };
  const write = (value: T) => {
    if (tabId) setViewState(tabId, { [key]: value });
  };
  return [read, write];
}
