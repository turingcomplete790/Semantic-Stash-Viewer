import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  commands: {
    shellLoadTabs: vi.fn(),
    shellSaveTabs: vi.fn(() => Promise.resolve({ status: "ok", data: null })),
    playerClose: vi.fn(() => Promise.resolve({ status: "ok", data: null })),
  },
  events: { playerState: { listen: vi.fn(() => Promise.resolve(() => {})) } },
}));
vi.mock("../../bindings", () => ({ commands: mocks.commands, events: mocks.events }));

import {
  back,
  close,
  currentRoute,
  forward,
  loadTabsFor,
  navigate,
  reopenClosed,
  resetTabs,
  select,
  selectedId,
  setViewState,
  tabs,
} from "../../shell/tabs";

const HOME = { kind: "home" } as const;
const SCENES = { kind: "scenes" } as const;
const SETTINGS = { kind: "settings", page: "servers" } as const;

beforeEach(() => {
  vi.clearAllMocks();
  vi.useFakeTimers();
  resetTabs();
});
afterEach(() => vi.useRealTimers());

describe("tab store", () => {
  it("starts with one Home tab", () => {
    expect(tabs()).toHaveLength(1);
    expect(currentRoute()).toEqual(HOME);
  });

  it("navigating the current tab pushes history; back and forward walk it", () => {
    navigate(SCENES);
    navigate(SETTINGS);
    expect(tabs()[0].history.map((e) => e.route)).toEqual([HOME, SCENES, SETTINGS]);
    back();
    expect(currentRoute()).toEqual(SCENES);
    back();
    expect(currentRoute()).toEqual(HOME);
    back(); // nothing further back
    expect(currentRoute()).toEqual(HOME);
    forward();
    expect(currentRoute()).toEqual(SCENES);
    // Navigating from the middle drops the forward entries.
    navigate({ kind: "scene", sceneId: "1", title: "One" });
    expect(tabs()[0].history.map((e) => e.route)).toEqual([
      HOME,
      SCENES,
      { kind: "scene", sceneId: "1", title: "One" },
    ]);
  });

  it("doesn't push the view that's already shown", () => {
    navigate(HOME);
    expect(tabs()[0].history).toHaveLength(1);
  });

  it("caps history at 50, dropping the oldest", () => {
    for (let i = 0; i < 60; i += 1) navigate({ kind: "scene", sceneId: String(i), title: "" });
    const history = tabs()[0].history;
    expect(history).toHaveLength(50);
    expect(history[0].route).toEqual({ kind: "scene", sceneId: "10", title: "" });
    expect(tabs()[0].index).toBe(49);
  });

  it("opens a new tab after the current one and selects it", () => {
    navigate(SCENES, { newTab: true });
    navigate(SETTINGS, { newTab: true });
    select(0);
    navigate({ kind: "scene", sceneId: "2", title: "" }, { newTab: true });
    expect(tabs().map((t) => t.history[t.index].route)).toEqual([
      HOME,
      { kind: "scene", sceneId: "2", title: "" },
      SCENES,
      SETTINGS,
    ]);
    expect(selectedId()).toBe(tabs()[1].id);
  });

  it("selects by position, with -1 for the last tab", () => {
    navigate(SCENES, { newTab: true });
    navigate(SETTINGS, { newTab: true });
    select(0);
    expect(currentRoute()).toEqual(HOME);
    select(-1);
    expect(currentRoute()).toEqual(SETTINGS);
    select(7); // out of range: ignored
    expect(currentRoute()).toEqual(SETTINGS);
  });

  it("closing the selected tab selects its neighbour; closing the last leaves Home", () => {
    navigate(SCENES, { newTab: true });
    navigate(SETTINGS, { newTab: true });
    select(1);
    close(selectedId());
    expect(currentRoute()).toEqual(SETTINGS);
    close(selectedId());
    expect(currentRoute()).toEqual(HOME);
    close(selectedId());
    expect(tabs()).toHaveLength(1);
    expect(currentRoute()).toEqual(HOME);
  });

  it("reopens closed tabs, most recent first, up to 10", () => {
    for (let i = 0; i < 12; i += 1) {
      navigate({ kind: "scene", sceneId: String(i), title: "" }, { newTab: true });
    }
    for (let i = 0; i < 12; i += 1) close(selectedId());
    reopenClosed();
    expect(currentRoute()).toEqual({ kind: "scene", sceneId: "0", title: "" });
    let reopened = 1;
    while (tabs().length < 20 && reopened < 20) {
      const before = tabs().length;
      reopenClosed();
      if (tabs().length === before) break;
      reopened += 1;
    }
    expect(reopened).toBe(10);
  });

  it("saves structure at once and view state after 500 ms", async () => {
    mocks.commands.shellLoadTabs.mockResolvedValue(null);
    await loadTabsFor("p1");
    mocks.commands.shellSaveTabs.mockClear();
    navigate(SCENES);
    expect(mocks.commands.shellSaveTabs).toHaveBeenCalledTimes(1);
    const saved = (mocks.commands.shellSaveTabs.mock.calls[0] as unknown[])[1] as {
      tabs: { history: { route: unknown }[] }[];
    };
    expect(saved.tabs[0].history.map((e) => e.route)).toEqual([HOME, SCENES]);

    setViewState(selectedId(), { scroll: 120 });
    setViewState(selectedId(), { scroll: 240 });
    expect(mocks.commands.shellSaveTabs).toHaveBeenCalledTimes(1);
    vi.advanceTimersByTime(500);
    expect(mocks.commands.shellSaveTabs).toHaveBeenCalledTimes(2);
  });

  it("loads a profile's saved tabs, keeping unknown routes for the unavailable view", async () => {
    mocks.commands.shellLoadTabs.mockResolvedValue({
      tabs: [
        { id: "a", history: [{ route: HOME, viewState: null }], index: 0 },
        { id: "b", history: [{ route: { kind: "galleries" }, viewState: null }], index: 0 },
      ],
      selectedTabId: "b",
    });
    await loadTabsFor("p1");
    expect(mocks.commands.shellLoadTabs).toHaveBeenCalledWith("p1");
    expect(tabs().map((t) => t.id)).toEqual(["a", "b"]);
    expect(selectedId()).toBe("b");
    expect(currentRoute()).toEqual({ kind: "galleries" });
  });

  it("starts with one Home tab when a profile has nothing saved", async () => {
    navigate(SCENES, { newTab: true });
    mocks.commands.shellLoadTabs.mockResolvedValue(null);
    await loadTabsFor("p2");
    expect(tabs()).toHaveLength(1);
    expect(currentRoute()).toEqual(HOME);
  });

  it("saves the previous profile's tabs before switching", async () => {
    mocks.commands.shellLoadTabs.mockResolvedValue(null);
    await loadTabsFor("p1");
    navigate(SCENES);
    setViewState(selectedId(), { scroll: 9 });
    mocks.commands.shellSaveTabs.mockClear();
    await loadTabsFor("p2");
    expect(mocks.commands.shellSaveTabs).toHaveBeenCalledWith("p1", expect.anything());
  });
});
