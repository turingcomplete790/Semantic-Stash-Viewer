import { cleanup, fireEvent, render, screen } from "@solidjs/testing-library";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ConnectionSnapshot, ProfileSummary } from "../../bindings";

const mocks = vi.hoisted(() => {
  const handlers: Record<string, (e: { payload: unknown }) => void> = {};
  const listen = (name: string) =>
    vi.fn((h: (e: { payload: unknown }) => void) => {
      handlers[name] = h;
      return Promise.resolve(() => {});
    });
  const ok = (data: unknown) => Promise.resolve({ status: "ok", data });
  return {
    emit: (name: string, payload: unknown) => handlers[name]?.({ payload }),
    commands: {
      getConnectionSnapshot: vi.fn(),
      listProfiles: vi.fn(),
      connect: vi.fn(),
      playerSnapshot: vi.fn(() =>
        Promise.resolve({ status: "error", error: { kind: "internal" } }),
      ),
      listRecentScenes: vi.fn(() => ok({ data: [], fromCache: false, fetchedAt: "" })),
      listTestScenes: vi.fn(() => ok({ data: [], fromCache: false, fetchedAt: "" })),
      scenesPage: vi.fn((_q: unknown, page: number, pageSize: number) =>
        ok({
          data: {
            count: 300,
            page,
            pageSize,
            items: Array.from({ length: pageSize }, (_, k) => {
              const n = (page - 1) * pageSize + k + 1;
              return {
                id: String(n),
                title: `Scene ${n}`,
                date: null,
                durationSeconds: null,
                resolution: null,
                studio: null,
                thumb: null,
                hasPreview: false,
              };
            }),
          },
          fromCache: false,
          fetchedAt: "",
        }),
      ),
      playerOpen: vi.fn(() =>
        Promise.resolve({ status: "error", error: { kind: "notConnected" } }),
      ),
      sceneSorts: vi.fn(() => Promise.resolve([{ value: "date", label: "Date" }])),
      playerSetViewport: vi.fn(() => ok(null)),
      debugOpenScene: vi.fn(() => Promise.resolve(null)),
      shellLoadTabs: vi.fn((): Promise<unknown> => Promise.resolve(null)),
      shellSaveTabs: vi.fn(() => ok(null)),
      playerSetVideoVisible: vi.fn(() => Promise.resolve(null)),
      notificationsList: vi.fn(() => Promise.resolve([])),
      debugBenchEnabled: vi.fn(() => Promise.resolve(false)),
      debugMeasureEnabled: vi.fn(() => Promise.resolve(false)),
      debugMarkInteractive: vi.fn(() => Promise.resolve(null)),
      cachedServerInfo: vi.fn(() => Promise.resolve(null)),
    },
    events: {
      connectionState: { listen: listen("connectionState") },
      profilesChanged: { listen: listen("profilesChanged") },
      playerState: { listen: listen("playerState") },
      notificationsChanged: { listen: listen("notificationsChanged") },
      viewDataChanged: { listen: listen("viewDataChanged") },
      appClosing: { listen: listen("appClosing") },
    },
  };
});
vi.mock("../../bindings", () => ({ commands: mocks.commands, events: mocks.events }));

import App from "../../App";
import type { Route } from "../../shell/routes";
import { navigate, resetTabs } from "../../shell/tabs";

const home: ProfileSummary = {
  id: "home",
  displayName: "Home server",
  baseUrl: "http://localhost:9999",
  strictTls: false,
  apiKey: null,
  lastUsedAt: null,
};
const connected: ConnectionSnapshot = {
  profileId: "home",
  state: { kind: "connected" },
  security: "unencrypted",
  finalUrl: "http://localhost:9999",
  server: {
    version: "v0.31.1",
    versionStatus: "supported",
    appSchema: 85,
    identity: "0000000000000000",
    counts: { scenes: 10, images: 2, galleries: 1, performers: 3 },
  },
  lastContactAt: null,
};

beforeEach(() => {
  vi.clearAllMocks();
  resetTabs();
  mocks.commands.listProfiles.mockResolvedValue({ status: "ok", data: [home] });
  mocks.commands.getConnectionSnapshot.mockResolvedValue(connected);
});
afterEach(cleanup);

describe("Shell", () => {
  it("renders the current route's view in the content area", async () => {
    render(() => <App />);
    const content = await screen.findByRole("main");
    expect(await screen.findByText("Connected to Home server")).toBeInTheDocument();
    navigate({ kind: "scenes" });
    expect(await screen.findByLabelText("Sort by")).toBeInTheDocument();
    expect(content).toContainElement(screen.getByLabelText("Sort by"));
  });

  it("explains an unknown view and offers a way out", async () => {
    render(() => <App />);
    await screen.findByText("Connected to Home server");
    navigate({ kind: "galleries" } as unknown as Route);
    expect(await screen.findByText("This view isn't available")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Close" }));
    expect(await screen.findByText("Connected to Home server")).toBeInTheDocument();
  });

  it("shows the server picker in the content area while disconnected", async () => {
    mocks.commands.getConnectionSnapshot.mockResolvedValue({
      ...connected,
      profileId: null,
      state: { kind: "idle" },
      server: null,
    });
    render(() => <App />);
    const content = await screen.findByRole("main");
    const heading = await screen.findByRole("heading", { name: "Choose a Stash server" });
    expect(content).toContainElement(heading);
  });

  // The demo's resets (005 SC-009): coming back to Scenes keeps page and mode.
  async function scenesOnPage3InList() {
    render(() => <App />);
    await screen.findByText("Connected to Home server");
    navigate({ kind: "scenes" });
    await screen.findByRole("button", { name: "Scene 1" });
    fireEvent.click(screen.getAllByRole("button", { name: "Next page" })[0]);
    await screen.findByRole("button", { name: "Scene 51" });
    fireEvent.click(screen.getAllByRole("button", { name: "Next page" })[0]);
    await screen.findByRole("button", { name: "Scene 101" });
    fireEvent.click(screen.getByRole("button", { name: "List" }));
  }

  it("returns from a scene to the same page and mode", async () => {
    await scenesOnPage3InList();
    fireEvent.click(screen.getByRole("button", { name: "Scene 101" }));
    fireEvent.click(await screen.findByRole("button", { name: "Back to scenes" }));
    expect(await screen.findByRole("button", { name: "Scene 101" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "List" })).toHaveAttribute("aria-pressed", "true");
  });

  it("returns through the navigation bar to the same page and mode", async () => {
    await scenesOnPage3InList();
    navigate({ kind: "home" });
    await screen.findByText("Connected to Home server");
    navigate({ kind: "scenes" });
    expect(await screen.findByRole("button", { name: "Scene 101" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "List" })).toHaveAttribute("aria-pressed", "true");
  });

  // A restart with saved tabs that include Scenes (the "can't interact after restart" bug): the
  // Scenes view must not write view state while the restored tabs are being built, which made the
  // tab panes rebuild themselves recursively until the stack overflowed and every pane vanished.
  it("restores saved tabs that include Scenes, and they all work", async () => {
    const errors: unknown[] = [];
    const onError = (e: ErrorEvent) => errors.push(e.error ?? e.message);
    const onRejection = (e: PromiseRejectionEvent) => errors.push(e.reason);
    window.addEventListener("error", onError);
    window.addEventListener("unhandledrejection", onRejection);
    mocks.commands.shellLoadTabs.mockResolvedValue({
      tabs: [
        {
          id: "t-scenes",
          history: [
            { route: { kind: "home" }, viewState: null },
            {
              route: { kind: "scenes" },
              viewState: {
                query: { search: "", sort: "title", direction: "desc", seed: null },
                page: 2,
                mode: "list",
              },
            },
          ],
          index: 1,
        },
        { id: "t-home", history: [{ route: { kind: "home" }, viewState: null }], index: 0 },
        {
          id: "t-scenes-2",
          history: [{ route: { kind: "scenes" }, viewState: { page: 3 } }],
          index: 0,
        },
      ],
      selectedTabId: "t-scenes",
    });
    try {
      render(() => <App />);
      expect(await screen.findByRole("button", { name: "Scene 51" })).toBeInTheDocument();
      expect(screen.getByRole("button", { name: "List" })).toHaveAttribute("aria-pressed", "true");
      expect(screen.getAllByRole("tab")).toHaveLength(3);
      fireEvent.click(screen.getAllByRole("tab")[2]);
      expect(await screen.findByRole("button", { name: "Scene 101" })).toBeInTheDocument();
      expect(errors).toEqual([]);
    } finally {
      window.removeEventListener("error", onError);
      window.removeEventListener("unhandledrejection", onRejection);
    }
  });
});
