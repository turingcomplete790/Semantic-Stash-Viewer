import { cleanup, fireEvent, render, screen, waitFor, within } from "@solidjs/testing-library";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ProfileSummary } from "../../bindings";

const mocks = vi.hoisted(() => {
  let profilesHandler: ((e: { payload: unknown }) => void) | undefined;
  return {
    setProfiles: (payload: unknown) => profilesHandler?.({ payload }),
    commands: {
      getConnectionSnapshot: vi.fn(),
      connect: vi.fn(),
      updateProfile: vi.fn(),
      deleteProfile: vi.fn(),
      reorderProfiles: vi.fn(),
    },
    events: {
      connectionState: { listen: vi.fn(() => Promise.resolve(() => {})) },
      profilesChanged: {
        listen: vi.fn((h: (e: { payload: unknown }) => void) => {
          profilesHandler = h;
          return Promise.resolve(() => {});
        }),
      },
    },
  };
});
vi.mock("../../bindings", () => ({ commands: mocks.commands, events: mocks.events }));

import ServersPage from "../../settings/ServersPage";
import { initConnection } from "../../state/connection";

const home: ProfileSummary = {
  id: "home",
  displayName: "Home",
  baseUrl: "http://localhost:9999",
  strictTls: false,
  apiKey: "home-key",
  lastUsedAt: null,
};
const travel: ProfileSummary = {
  id: "travel",
  displayName: "Travel",
  baseUrl: "https://travel.example.com",
  strictTls: true,
  apiKey: null,
  lastUsedAt: null,
};

beforeEach(async () => {
  vi.resetAllMocks();
  mocks.events.connectionState.listen.mockResolvedValue(() => {});
  mocks.commands.getConnectionSnapshot.mockResolvedValue({
    profileId: "home",
    state: { kind: "connected" },
    security: "unencrypted",
    finalUrl: "http://localhost:9999",
    server: null,
    lastContactAt: null,
  });
  mocks.commands.connect.mockResolvedValue({ status: "ok", data: {} });
  mocks.commands.deleteProfile.mockResolvedValue({ status: "ok", data: null });
  mocks.commands.reorderProfiles.mockResolvedValue({ status: "ok", data: null });
  await initConnection();
  mocks.setProfiles([home, travel]);
});
afterEach(cleanup);

function row(name: string) {
  return screen.getByRole("listitem", { name });
}

describe("Settings → Servers", () => {
  it("lists profiles with address and strict setting, marking the active one", () => {
    render(() => <ServersPage onAdd={() => {}} />);
    expect(within(row("Home")).getByText("http://localhost:9999")).toBeInTheDocument();
    expect(within(row("Home")).getByText("Active")).toBeInTheDocument();
    expect(within(row("Travel")).getByText(/Strict/)).toBeInTheDocument();
  });

  it("switching calls connect with the chosen profile id", () => {
    render(() => <ServersPage onAdd={() => {}} />);
    fireEvent.click(within(row("Travel")).getByRole("button", { name: "Switch" }));
    expect(mocks.commands.connect).toHaveBeenCalledWith("travel", expect.any(String));
  });

  it("asks for confirmation before deleting", async () => {
    render(() => <ServersPage onAdd={() => {}} />);
    fireEvent.click(within(row("Travel")).getByRole("button", { name: "Delete" }));
    expect(mocks.commands.deleteProfile).not.toHaveBeenCalled();

    const confirm = screen.getByRole("alertdialog");
    expect(confirm).toHaveTextContent("Delete Travel?");
    fireEvent.click(within(confirm).getByRole("button", { name: "Delete" }));
    await waitFor(() => expect(mocks.commands.deleteProfile).toHaveBeenCalledWith("travel"));
  });

  it("cancelling the confirmation deletes nothing", () => {
    render(() => <ServersPage onAdd={() => {}} />);
    fireEvent.click(within(row("Travel")).getByRole("button", { name: "Delete" }));
    fireEvent.click(
      within(screen.getByRole("alertdialog")).getByRole("button", { name: "Cancel" }),
    );
    expect(mocks.commands.deleteProfile).not.toHaveBeenCalled();
  });

  it("shows the API key as plain text in edit mode and saves the edited profile", async () => {
    mocks.commands.updateProfile.mockResolvedValue({ status: "ok", data: home });
    render(() => <ServersPage onAdd={() => {}} />);
    fireEvent.click(within(row("Home")).getByRole("button", { name: "Edit" }));

    const key = screen.getByLabelText("API key");
    expect(key).toHaveValue("home-key");
    expect(key).toHaveAttribute("type", "text");

    fireEvent.input(screen.getByLabelText("Display name"), { target: { value: "Home server" } });
    fireEvent.click(screen.getByRole("button", { name: "Save" }));
    await waitFor(() =>
      expect(mocks.commands.updateProfile).toHaveBeenCalledWith(
        "home",
        {
          displayName: "Home server",
          address: "http://localhost:9999",
          apiKey: "home-key",
          strictTls: false,
        },
        false,
      ),
    );
  });

  it("offers Save anyway when the check fails", async () => {
    mocks.commands.updateProfile
      .mockResolvedValueOnce({
        status: "error",
        error: { kind: "connect", failure: { kind: "unreachable", tried: ["http://x:1"] } },
      })
      .mockResolvedValueOnce({ status: "ok", data: home });
    render(() => <ServersPage onAdd={() => {}} />);
    fireEvent.click(within(row("Home")).getByRole("button", { name: "Edit" }));
    fireEvent.click(screen.getByRole("button", { name: "Save" }));

    fireEvent.click(await screen.findByRole("button", { name: "Save anyway" }));
    await waitFor(() => expect(mocks.commands.updateProfile).toHaveBeenCalledTimes(2));
    expect(mocks.commands.updateProfile.mock.calls[1][2]).toBe(true);
  });

  it("offers Open existing on a duplicate", async () => {
    mocks.commands.updateProfile.mockResolvedValue({
      status: "error",
      error: {
        kind: "connect",
        failure: { kind: "duplicateProfile", existingId: "travel" },
      },
    });
    render(() => <ServersPage onAdd={() => {}} />);
    fireEvent.click(within(row("Home")).getByRole("button", { name: "Edit" }));
    fireEvent.click(screen.getByRole("button", { name: "Save" }));

    fireEvent.click(await screen.findByRole("button", { name: "Open existing" }));
    expect(mocks.commands.connect).toHaveBeenCalledWith("travel", expect.any(String));
  });

  it("moves a profile down and saves the new order", async () => {
    render(() => <ServersPage onAdd={() => {}} />);
    fireEvent.click(within(row("Home")).getByRole("button", { name: "Move Home down" }));
    await waitFor(() =>
      expect(mocks.commands.reorderProfiles).toHaveBeenCalledWith(["travel", "home"]),
    );
  });

  it("Add server calls onAdd", () => {
    const onAdd = vi.fn();
    render(() => <ServersPage onAdd={onAdd} />);
    fireEvent.click(screen.getByRole("button", { name: "Add server" }));
    expect(onAdd).toHaveBeenCalled();
  });
});
