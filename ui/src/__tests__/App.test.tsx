import { cleanup, render, screen } from "@solidjs/testing-library";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ConnectionSnapshot, ProfileSummary } from "../bindings";

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
      connect: vi.fn(),
      shellLoadTabs: vi.fn(() => Promise.resolve(null)),
      shellSaveTabs: vi.fn(() => Promise.resolve({ status: "ok", data: null })),
      playerSetVideoVisible: vi.fn(() => Promise.resolve(null)),
      playerSnapshot: vi.fn(() =>
        Promise.resolve({ status: "error", error: { kind: "internal" } }),
      ),
    },
    events: {
      connectionState: { listen: listen("connectionState") },
      profilesChanged: { listen: listen("profilesChanged") },
      playerState: { listen: listen("playerState") },
    },
  };
});
vi.mock("../bindings", () => ({ commands: mocks.commands, events: mocks.events }));

import App from "../App";

const idle: ConnectionSnapshot = {
  profileId: null,
  state: { kind: "idle" },
  security: null,
  finalUrl: null,
  server: null,
  lastContactAt: null,
};
const home: ProfileSummary = {
  id: "home",
  displayName: "Home",
  baseUrl: "http://localhost:9999",
  strictTls: false,
  apiKey: null,
  lastUsedAt: null,
};

beforeEach(() => {
  vi.clearAllMocks();
  mocks.commands.getConnectionSnapshot.mockResolvedValue(idle);
});
afterEach(cleanup);

describe("App routing", () => {
  it("shows the connection form when no servers are saved", async () => {
    mocks.commands.listProfiles.mockResolvedValue({ status: "ok", data: [] });
    render(() => <App />);
    expect(await screen.findByRole("heading", { name: "Connect to Stash" })).toBeInTheDocument();
  });

  it("shows the profile picker when nothing is active but servers are saved", async () => {
    mocks.commands.listProfiles.mockResolvedValue({ status: "ok", data: [home] });
    render(() => <App />);
    expect(
      await screen.findByRole("heading", { name: "Choose a Stash server" }),
    ).toBeInTheDocument();
  });

  it("returns to the picker when the active profile is deleted (US4 AS3)", async () => {
    mocks.commands.listProfiles.mockResolvedValue({ status: "ok", data: [home] });
    mocks.commands.getConnectionSnapshot.mockResolvedValue({
      ...idle,
      profileId: "home",
      state: { kind: "connecting", attemptUrl: "http://localhost:9999" },
    });
    render(() => <App />);
    expect(await screen.findByText("Connecting to Home…")).toBeInTheDocument();

    // Deleting another profile doesn't matter; deleting the active one disconnects (Idle).
    const other: ProfileSummary = { ...home, id: "other", displayName: "Other" };
    mocks.emit("profilesChanged", [other]);
    mocks.emit("connectionState", idle);
    expect(
      await screen.findByRole("heading", { name: "Choose a Stash server" }),
    ).toBeInTheDocument();
    expect(screen.getByText("Other")).toBeInTheDocument();
  });
});
