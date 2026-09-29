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
      listRecentScenes: vi.fn(() => ok([])),
      listTestScenes: vi.fn(() => ok([])),
      playerSetViewport: vi.fn(() => ok(null)),
      debugOpenScene: vi.fn(() => Promise.resolve(null)),
      shellLoadTabs: vi.fn(() => Promise.resolve(null)),
      shellSaveTabs: vi.fn(() => ok(null)),
      playerSetVideoVisible: vi.fn(() => Promise.resolve(null)),
      notificationsList: vi.fn(() => Promise.resolve([])),
      debugBenchEnabled: vi.fn(() => Promise.resolve(false)),
    },
    events: {
      connectionState: { listen: listen("connectionState") },
      profilesChanged: { listen: listen("profilesChanged") },
      playerState: { listen: listen("playerState") },
      notificationsChanged: { listen: listen("notificationsChanged") },
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
    expect(await screen.findByRole("heading", { name: "Recently added" })).toBeInTheDocument();
    expect(content).toContainElement(screen.getByRole("heading", { name: "Recently added" }));
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
});
