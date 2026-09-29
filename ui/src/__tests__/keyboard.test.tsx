// Keyboard accessibility (T058, Principle VI): focus on open, Tab stays inside dialogs,
// Escape backs out one level, focus returns to the trigger.

import { cleanup, fireEvent, render, screen, waitFor, within } from "@solidjs/testing-library";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ProfileSummary } from "../bindings";

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
      updateProfile: vi.fn(),
      deleteProfile: vi.fn(),
      reorderProfiles: vi.fn(),
      connect: vi.fn(),
    },
    events: {
      connectionState: { listen: listen("connectionState") },
      profilesChanged: { listen: listen("profilesChanged") },
    },
  };
});
vi.mock("../bindings", () => ({ commands: mocks.commands, events: mocks.events }));

import ConnectionForm from "../components/ConnectionForm";
import ConnectionIndicator from "../components/ConnectionIndicator";
import KeyPrompt from "../components/KeyPrompt";
import ServersPage from "../settings/ServersPage";
import { initConnection } from "../state/connection";

const home: ProfileSummary = {
  id: "home",
  displayName: "Home",
  baseUrl: "http://localhost:9999",
  strictTls: false,
  apiKey: "k",
  lastUsedAt: null,
};

beforeEach(async () => {
  vi.clearAllMocks();
  mocks.commands.getConnectionSnapshot.mockResolvedValue({
    profileId: "home",
    state: { kind: "connected" },
    security: "unencrypted",
    finalUrl: "http://localhost:9999",
    server: null,
    lastContactAt: null,
  });
  await initConnection();
  mocks.emit("profilesChanged", [home]);
});
afterEach(cleanup);

function tab(shift = false) {
  fireEvent.keyDown(document, { key: "Tab", shiftKey: shift });
}

describe("keyboard", () => {
  it("focuses the address field when the connection form opens", () => {
    render(() => <ConnectionForm onSaved={() => {}} />);
    expect(screen.getByLabelText("Server address")).toHaveFocus();
  });

  it("keeps Tab inside the key prompt", () => {
    render(() => <KeyPrompt profile={home} onDone={() => {}} onCancel={() => {}} />);
    const cancel = screen.getByRole("button", { name: "Cancel" });
    cancel.focus();
    tab(); // past the last control → wraps to the first
    expect(screen.getByLabelText("API key")).toHaveFocus();
    tab(true); // back past the first → wraps to the last
    expect(cancel).toHaveFocus();
  });

  it("Escape in the delete confirmation cancels it without deleting", () => {
    render(() => <ServersPage onAdd={() => {}} />);
    const row = screen.getByRole("listitem", { name: "Home" });
    fireEvent.click(within(row).getByRole("button", { name: "Delete" }));
    const confirm = screen.getByRole("alertdialog");

    fireEvent.keyDown(confirm, { key: "Escape" });
    expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
    expect(mocks.commands.deleteProfile).not.toHaveBeenCalled();
    expect(screen.getByRole("heading", { name: "Servers" })).toBeInTheDocument();
  });

  it("focuses Cancel (the safe choice) in the delete confirmation", async () => {
    render(() => <ServersPage onAdd={() => {}} />);
    const row = screen.getByRole("listitem", { name: "Home" });
    fireEvent.click(within(row).getByRole("button", { name: "Delete" }));
    const confirm = screen.getByRole("alertdialog");
    await waitFor(() =>
      expect(within(confirm).getByRole("button", { name: "Cancel" })).toHaveFocus(),
    );
  });

  it("returns focus to the security button when details close with Escape", async () => {
    render(() => <ConnectionIndicator profileName="Home" onUpdateKey={() => {}} />);
    const trigger = await screen.findByRole("button", { name: /Unencrypted/ });
    fireEvent.click(trigger);
    expect(screen.getByRole("dialog", { name: "Connection details" })).toBeInTheDocument();

    fireEvent.keyDown(document, { key: "Escape" });
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
    expect(trigger).toHaveFocus();
  });
});
