import { cleanup, render, screen } from "@solidjs/testing-library";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ConnectionSnapshot, ProfileSummary, ServerInfo } from "../../bindings";

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
      cachedServerInfo: vi.fn(),
      debugMarkInteractive: vi.fn(() => Promise.resolve(null)),
    },
    events: {
      connectionState: { listen: listen("connectionState") },
      profilesChanged: { listen: listen("profilesChanged") },
    },
  };
});
vi.mock("../../bindings", () => ({ commands: mocks.commands, events: mocks.events }));

import { initConnection, refreshProfiles } from "../../state/connection";
import HomeView from "../../views/HomeView";

const home: ProfileSummary = {
  id: "home",
  displayName: "Home server",
  baseUrl: "http://localhost:9999",
  strictTls: false,
  apiKey: null,
  lastUsedAt: null,
};
const lastKnown: ServerInfo = {
  version: "v0.31.1",
  versionStatus: "supported",
  appSchema: 85,
  identity: "0000000000000000",
  counts: { scenes: 4321, images: 12, galleries: 3, performers: 99 },
};

function snapshot(kind: "connecting" | "offline"): ConnectionSnapshot {
  return {
    profileId: "home",
    state: { kind },
    security: null,
    finalUrl: null,
    server: null,
    lastContactAt: null,
  } as ConnectionSnapshot;
}

async function start(snap: ConnectionSnapshot) {
  mocks.commands.getConnectionSnapshot.mockResolvedValue(snap);
  await initConnection();
  await refreshProfiles();
  render(() => <HomeView onAddAnother={() => {}} onRetry={() => {}} onUpdateKey={() => {}} />);
}

beforeEach(() => {
  vi.clearAllMocks();
  mocks.commands.listProfiles.mockResolvedValue({ status: "ok", data: [home] });
  mocks.commands.cachedServerInfo.mockResolvedValue({
    server: lastKnown,
    fetchedAt: "2026-09-29T12:00:00Z",
  });
});
afterEach(cleanup);

describe("Home", () => {
  it("shows the last known summary while connecting", async () => {
    await start(snapshot("connecting"));
    expect(screen.getByRole("heading", { name: "Connecting to Home server…" })).toBeInTheDocument();
    expect(await screen.findByText("4,321")).toBeInTheDocument();
    expect(screen.getByText("v0.31.1")).toBeInTheDocument();
    // The cache is never mentioned (003 FR-003).
    expect(screen.queryByText(/cache/i)).not.toBeInTheDocument();
  });

  it("shows the last known summary while offline", async () => {
    await start(snapshot("offline"));
    expect(screen.getByRole("heading", { name: "Can't reach Home server" })).toBeInTheDocument();
    expect(await screen.findByText("4,321")).toBeInTheDocument();
  });

  it("shows no summary when nothing is known yet", async () => {
    mocks.commands.cachedServerInfo.mockResolvedValue(null);
    await start(snapshot("connecting"));
    await Promise.resolve();
    expect(mocks.commands.cachedServerInfo).toHaveBeenCalled();
    expect(screen.queryByText("Scenes")).not.toBeInTheDocument();
  });
});
