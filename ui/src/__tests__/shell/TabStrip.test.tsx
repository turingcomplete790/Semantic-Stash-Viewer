import { cleanup, fireEvent, render, screen, within } from "@solidjs/testing-library";
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

import KeyboardHelp from "../../shell/KeyboardHelp";
import { installKeymap, register, resetKeymap } from "../../shell/keymap";
import TabStrip from "../../shell/TabStrip";
import {
  currentRoute,
  installMouseNavigation,
  navigate,
  registerTabShortcuts,
  resetTabs,
  selectedId,
  tabs,
} from "../../shell/tabs";

const SCENES = { kind: "scenes" } as const;
const SETTINGS = { kind: "settings", page: "servers" } as const;

let teardown: Array<() => void> = [];

beforeEach(() => {
  vi.clearAllMocks();
  resetTabs();
  teardown = [installKeymap(), registerTabShortcuts(), installMouseNavigation()];
});
afterEach(() => {
  cleanup();
  teardown.forEach((t) => t());
  resetKeymap();
});

function chord(key: string, mods: { ctrl?: boolean; shift?: boolean; alt?: boolean } = {}) {
  fireEvent.keyDown(document.body, {
    key,
    ctrlKey: !!mods.ctrl,
    shiftKey: !!mods.shift,
    altKey: !!mods.alt,
  });
}

function threeTabs() {
  navigate(SCENES, { newTab: true });
  navigate(SETTINGS, { newTab: true });
}

describe("TabStrip", () => {
  it("shows each tab's title and marks the selected one", () => {
    threeTabs();
    render(() => <TabStrip />);
    const list = screen.getByRole("tablist");
    const tabEls = within(list).getAllByRole("tab");
    expect(tabEls.map((t) => t.textContent)).toEqual(["Home", "Scenes", "Settings"]);
    expect(tabEls[2]).toHaveAttribute("aria-selected", "true");
    expect(tabEls[0]).toHaveAttribute("aria-selected", "false");
  });

  it("selects on click, and closes with middle-click or the close button", () => {
    threeTabs();
    render(() => <TabStrip />);
    fireEvent.click(screen.getByRole("tab", { name: "Home" }));
    expect(currentRoute()).toEqual({ kind: "home" });
    fireEvent(
      screen.getByRole("tab", { name: "Scenes" }),
      new MouseEvent("auxclick", { button: 1, bubbles: true }),
    );
    expect(tabs()).toHaveLength(2);
    fireEvent.click(screen.getByRole("button", { name: "Close Settings" }));
    expect(tabs()).toHaveLength(1);
  });

  it("opens a new Home tab from the + button", () => {
    render(() => <TabStrip />);
    fireEvent.click(screen.getByRole("button", { name: "New tab" }));
    expect(tabs()).toHaveLength(2);
  });

  it("supports the browser tab shortcuts", () => {
    chord("t", { ctrl: true });
    expect(tabs()).toHaveLength(2);
    chord("w", { ctrl: true });
    expect(tabs()).toHaveLength(1);
    chord("T", { ctrl: true, shift: true }); // reopen
    expect(tabs()).toHaveLength(2);

    resetTabs();
    threeTabs(); // Home, Scenes, Settings (selected)
    chord("Tab", { ctrl: true });
    expect(currentRoute()).toEqual({ kind: "home" }); // wraps
    chord("Tab", { ctrl: true, shift: true });
    expect(currentRoute()).toEqual(SETTINGS);
    chord("PageUp", { ctrl: true });
    expect(currentRoute()).toEqual(SCENES);
    chord("PageDown", { ctrl: true });
    expect(currentRoute()).toEqual(SETTINGS);
    chord("1", { ctrl: true });
    expect(currentRoute()).toEqual({ kind: "home" });
    chord("9", { ctrl: true });
    expect(currentRoute()).toEqual(SETTINGS);
  });

  it("walks history with Alt+Left/Right and the mouse back/forward buttons", () => {
    navigate(SCENES);
    navigate(SETTINGS);
    chord("ArrowLeft", { alt: true });
    expect(currentRoute()).toEqual(SCENES);
    fireEvent(window, new MouseEvent("mouseup", { button: 3 }));
    expect(currentRoute()).toEqual({ kind: "home" });
    fireEvent(window, new MouseEvent("mouseup", { button: 4 }));
    expect(currentRoute()).toEqual(SCENES);
    chord("ArrowRight", { alt: true });
    expect(currentRoute()).toEqual(SETTINGS);
  });

  it("has back and forward buttons that reflect the tab's history", () => {
    render(() => <TabStrip />);
    expect(screen.getByRole("button", { name: "Back" })).toBeDisabled();
    navigate(SCENES);
    fireEvent.click(screen.getByRole("button", { name: "Back" }));
    expect(currentRoute()).toEqual({ kind: "home" });
    fireEvent.click(screen.getByRole("button", { name: "Forward" }));
    expect(currentRoute()).toEqual(SCENES);
    expect(selectedId()).toBe(tabs()[0].id);
  });

  it("? opens a keyboard overlay listing every binding", () => {
    register({ id: "pause", keys: ["Space"], scope: "scene", description: "Play or pause" });
    render(() => <KeyboardHelp />);
    fireEvent.keyDown(document.body, { key: "?" });
    const dialog = screen.getByRole("dialog", { name: "Keyboard shortcuts" });
    expect(within(dialog).getByText("New tab")).toBeInTheDocument();
    expect(within(dialog).getByText("Play or pause")).toBeInTheDocument();
    fireEvent.keyDown(document.body, { key: "Escape" });
    expect(screen.queryByRole("dialog", { name: "Keyboard shortcuts" })).not.toBeInTheDocument();
  });
});
