import { cleanup, fireEvent, render, screen, within } from "@solidjs/testing-library";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ConnectionSnapshot, ProfileSummary } from "../../bindings";

const mocks = vi.hoisted(() => {
  const handlers: Record<string, (e: { payload: unknown }) => void> = {};
  const listen = (name: string) =>
    vi.fn((h: (e: { payload: unknown }) => void) => {
      handlers[name] = h;
      return Promise.resolve(() => {});
    });
  return {
    emit: (name: string, payload: unknown) => handlers[name]?.({ payload }),
    commands: {
      getConnectionSnapshot: vi.fn(),
      listProfiles: vi.fn(),
      connect: vi.fn(() => Promise.resolve({ status: "ok", data: null })),
    },
    events: {
      connectionState: { listen: listen("connectionState") },
      profilesChanged: { listen: listen("profilesChanged") },
    },
  };
});
vi.mock("../../bindings", () => ({ commands: mocks.commands, events: mocks.events }));

import NavBar from "../../shell/NavBar";
import { installKeymap, resetKeymap } from "../../shell/keymap";
import type { Route } from "../../shell/routes";
import { initConnection, refreshProfiles } from "../../state/connection";

const home: ProfileSummary = {
  id: "home",
  displayName: "Home server",
  baseUrl: "http://localhost:9999",
  strictTls: false,
  apiKey: null,
  lastUsedAt: null,
};
const other: ProfileSummary = { ...home, id: "other", displayName: "Test box" };
const connected: ConnectionSnapshot = {
  profileId: "home",
  state: { kind: "connected" },
  security: "unencrypted",
  finalUrl: "http://localhost:9999",
  server: null,
  lastContactAt: null,
};
const idle: ConnectionSnapshot = { ...connected, profileId: null, state: { kind: "idle" } };

// jsdom has no layout: the tests control widths and resize callbacks.
const resizeCallbacks: Array<() => void> = [];
class FakeResizeObserver {
  constructor(private cb: () => void) {
    resizeCallbacks.push(() => this.cb());
  }
  observe() {}
  disconnect() {}
  unobserve() {}
}
let sectionsWidth = 1000;

function handlers() {
  return {
    onHome: vi.fn(),
    onNavigate: vi.fn(),
    onOpenSettings: vi.fn(),
    onUpdateKey: vi.fn(),
    onManageServers: vi.fn(),
  };
}

let uninstall: () => void;

beforeEach(async () => {
  vi.clearAllMocks();
  resizeCallbacks.length = 0;
  sectionsWidth = 1000;
  vi.stubGlobal("ResizeObserver", FakeResizeObserver);
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(function (
    this: HTMLElement,
  ) {
    const width = this.classList.contains("nav-sections")
      ? sectionsWidth
      : this.classList.contains("nav-section")
        ? 90
        : 40;
    return { width, height: 32, x: 0, y: 0, left: 0, top: 0, right: width, bottom: 32 } as DOMRect;
  });
  mocks.commands.listProfiles.mockResolvedValue({ status: "ok", data: [home, other] });
  mocks.commands.getConnectionSnapshot.mockResolvedValue(connected);
  await initConnection();
  await refreshProfiles();
  uninstall = installKeymap();
});
afterEach(() => {
  cleanup();
  uninstall();
  resetKeymap();
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

function key(k: string) {
  fireEvent.keyDown(document.body, { key: k });
}

describe("NavBar", () => {
  it("shows the viewer name, the sections in order, then the bell, Settings, and the server", () => {
    render(() => <NavBar current={{ kind: "home" }} {...handlers()} />);
    const nav = screen.getByRole("navigation", { name: "Main" });
    const labels = within(nav)
      .getAllByRole("button")
      .map((b) => b.getAttribute("aria-label") ?? b.textContent?.trim());
    expect(labels.slice(0, 3)).toEqual(["Semantic Stash Viewer", "Home", "Scenes"]);
    expect(labels).toContain("Notifications");
    expect(labels).toContain("Settings");
    expect(within(nav).getByRole("button", { name: /Server: Home server/ })).toBeInTheDocument();
    // Only sections that exist; no placeholders for future ones.
    expect(within(nav).queryByRole("button", { name: "Galleries" })).not.toBeInTheDocument();
  });

  it("highlights the current section and navigates on click", () => {
    const h = handlers();
    render(() => <NavBar current={{ kind: "scene", sceneId: "1", title: "x" }} {...h} />);
    expect(screen.getByRole("button", { name: "Scenes" })).toHaveAttribute("aria-current", "page");
    expect(screen.getByRole("button", { name: "Home" })).not.toHaveAttribute("aria-current");
    fireEvent.click(screen.getByRole("button", { name: "Home" }));
    expect(h.onNavigate).toHaveBeenCalledWith({ kind: "home" }, { newTab: false });
    fireEvent.click(screen.getByRole("button", { name: "Semantic Stash Viewer" }));
    expect(h.onHome).toHaveBeenCalled();
  });

  it("shows only Settings and the bell while disconnected", async () => {
    render(() => <NavBar current={null} {...handlers()} />);
    mocks.emit("connectionState", idle);
    const nav = screen.getByRole("navigation", { name: "Main" });
    expect(within(nav).queryByRole("button", { name: "Home" })).not.toBeInTheDocument();
    expect(within(nav).queryByRole("button", { name: "Scenes" })).not.toBeInTheDocument();
    expect(within(nav).getByRole("button", { name: "Settings" })).toBeInTheDocument();
    expect(within(nav).getByRole("button", { name: "Notifications" })).toBeInTheDocument();
  });

  it("supports g-shortcuts for sections, Settings, and notifications", () => {
    const h = handlers();
    render(() => <NavBar current={{ kind: "home" }} {...h} />);
    key("g");
    key("s");
    expect(h.onNavigate).toHaveBeenCalledWith({ kind: "scenes" } satisfies Route, {
      newTab: false,
    });
    key("g");
    key("h");
    expect(h.onNavigate).toHaveBeenCalledWith({ kind: "home" }, { newTab: false });
    key("g");
    key("z");
    expect(h.onOpenSettings).toHaveBeenCalled();
    key("g");
    key("n");
    expect(screen.getByRole("dialog", { name: "Notifications" })).toBeInTheDocument();
  });

  it("ignores g-shortcuts while typing", () => {
    const h = handlers();
    render(() => (
      <>
        <input aria-label="Search" />
        <NavBar current={{ kind: "home" }} {...h} />
      </>
    ));
    const input = screen.getByLabelText("Search");
    fireEvent.keyDown(input, { key: "g" });
    fireEvent.keyDown(input, { key: "s" });
    expect(h.onNavigate).not.toHaveBeenCalled();
  });

  it("moves sections that don't fit into a More menu", async () => {
    render(() => <NavBar current={{ kind: "home" }} {...handlers()} />);
    expect(screen.queryByRole("button", { name: "More sections" })).not.toBeInTheDocument();
    sectionsWidth = 150; // room for one 90 px section plus the More button
    resizeCallbacks.forEach((fire) => fire());
    const more = await screen.findByRole("button", { name: "More sections" });
    expect(screen.getByRole("button", { name: "Home" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Scenes" })).not.toBeInTheDocument();
    fireEvent.click(more);
    expect(screen.getByRole("menuitem", { name: "Scenes" })).toBeInTheDocument();
  });

  it("offers switch server, update API key, and manage servers from the server entry", () => {
    const h = handlers();
    render(() => <NavBar current={{ kind: "home" }} {...h} />);
    fireEvent.click(screen.getByRole("button", { name: /Server: Home server/ }));
    fireEvent.click(screen.getByRole("menuitem", { name: "Switch to Test box" }));
    expect(mocks.commands.connect).toHaveBeenCalledWith("other", expect.any(String));
    fireEvent.click(screen.getByRole("button", { name: /Server: Home server/ }));
    fireEvent.click(screen.getByRole("menuitem", { name: "Update API key" }));
    expect(h.onUpdateKey).toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: /Server: Home server/ }));
    fireEvent.click(screen.getByRole("menuitem", { name: "Manage servers" }));
    expect(h.onManageServers).toHaveBeenCalled();
  });
});
