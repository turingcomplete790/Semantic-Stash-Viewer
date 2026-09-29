import { cleanup, fireEvent, render, screen, waitFor, within } from "@solidjs/testing-library";
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
      openLogFolder: vi.fn(),
      appInfo: vi.fn(() => Promise.resolve({ version: "0.1.0" })),
    },
    events: {
      connectionState: { listen: listen("connectionState") },
      profilesChanged: { listen: listen("profilesChanged") },
    },
  };
});
vi.mock("../../bindings", () => ({ commands: mocks.commands, events: mocks.events }));

import SettingsView from "../../settings/SettingsView";
import { register, resetKeymap } from "../../shell/keymap";
import type { SettingsPage } from "../../shell/routes";
import { initConnection, refreshProfiles } from "../../state/connection";

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
    counts: { scenes: 1, images: 1, galleries: 1, performers: 1 },
  },
  lastContactAt: null,
};

beforeEach(async () => {
  vi.clearAllMocks();
  mocks.commands.getConnectionSnapshot.mockResolvedValue(connected);
  mocks.commands.listProfiles.mockResolvedValue({ status: "ok", data: [home] });
  await initConnection();
  await refreshProfiles();
});
afterEach(() => {
  cleanup();
  resetKeymap();
});

function view(page: SettingsPage, onPage = vi.fn()) {
  render(() => <SettingsView page={page} onPage={onPage} onAddServer={() => {}} />);
  return onPage;
}

describe("Settings", () => {
  it("lists the pages and changes page on click", () => {
    const onPage = view("servers");
    const pages = screen.getByRole("navigation", { name: "Settings pages" });
    expect(
      within(pages)
        .getAllByRole("button")
        .map((b) => b.textContent),
    ).toEqual(["Servers", "Keyboard", "Troubleshooting", "About"]);
    expect(within(pages).getByRole("button", { name: "Servers" })).toHaveAttribute(
      "aria-current",
      "page",
    );
    fireEvent.click(within(pages).getByRole("button", { name: "About" }));
    expect(onPage).toHaveBeenCalledWith("about");
  });

  it("Servers shows the saved servers", () => {
    view("servers");
    expect(screen.getByRole("heading", { name: "Servers" })).toBeInTheDocument();
    expect(screen.getByRole("listitem", { name: "Home server" })).toBeInTheDocument();
  });

  it("Keyboard lists every registered shortcut", () => {
    register({ id: "x", keys: ["Ctrl+T"], scope: "shell", description: "New tab", run: () => {} });
    register({ id: "y", keys: ["Space"], scope: "scene", description: "Play or pause" });
    view("keyboard");
    expect(screen.getByText("New tab")).toBeInTheDocument();
    expect(screen.getByText("Play or pause")).toBeInTheDocument();
  });

  it("Troubleshooting opens the log folder and explains failures", async () => {
    mocks.commands.openLogFolder.mockResolvedValueOnce({ status: "ok", data: null });
    view("troubleshooting");
    fireEvent.click(screen.getByRole("button", { name: "Open log folder" }));
    expect(mocks.commands.openLogFolder).toHaveBeenCalled();
    mocks.commands.openLogFolder.mockResolvedValueOnce({
      status: "error",
      error: { kind: "openFailed", detail: "xdg-open not found" },
    });
    fireEvent.click(screen.getByRole("button", { name: "Open log folder" }));
    expect(await screen.findByText("Couldn't open the folder")).toBeInTheDocument();
    expect(screen.getByText("xdg-open not found")).toBeInTheDocument();
  });

  it("About shows the viewer version and the connected server", async () => {
    view("about");
    await waitFor(() => expect(screen.getByText("0.1.0")).toBeInTheDocument());
    expect(screen.getByText("http://localhost:9999")).toBeInTheDocument();
    expect(screen.getByText("v0.31.1")).toBeInTheDocument();
  });
});
