import { cleanup, fireEvent, render, screen, waitFor } from "@solidjs/testing-library";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ConnectionSnapshot } from "../bindings";

const mocks = vi.hoisted(() => {
  let handler: ((e: { payload: unknown }) => void) | undefined;
  return {
    emit: (payload: unknown) => handler?.({ payload }),
    commands: { getConnectionSnapshot: vi.fn() },
    events: {
      connectionState: {
        listen: vi.fn((h: (e: { payload: unknown }) => void) => {
          handler = h;
          return Promise.resolve(() => {});
        }),
      },
    },
  };
});
vi.mock("../bindings", () => ({ commands: mocks.commands, events: mocks.events }));

import ConnectionIndicator from "../components/ConnectionIndicator";
import { connection, initConnection } from "../state/connection";

const base: ConnectionSnapshot = {
  profileId: "p1",
  state: { kind: "idle" },
  security: null,
  finalUrl: "http://localhost:9999",
  server: null,
  lastContactAt: null,
};

function snap(state: ConnectionSnapshot["state"]): ConnectionSnapshot {
  return { ...base, state };
}

beforeEach(async () => {
  mocks.commands.getConnectionSnapshot.mockResolvedValue({ ...base, profileId: null });
  await initConnection();
});
afterEach(cleanup);

describe("connection store", () => {
  it("hydrates from getConnectionSnapshot and follows connection-state events", async () => {
    expect(connection.snapshot().profileId).toBeNull();
    mocks.emit(snap({ kind: "connected" }));
    expect(connection.snapshot().state.kind).toBe("connected");
  });
});

describe("ConnectionIndicator", () => {
  it("renders connecting", async () => {
    render(() => <ConnectionIndicator profileName="Home" onUpdateKey={() => {}} />);
    mocks.emit(snap({ kind: "connecting", attemptUrl: "http://localhost:9999" }));
    expect(await screen.findByText(/Connecting/)).toBeInTheDocument();
  });

  it("renders connected with the profile name", async () => {
    render(() => <ConnectionIndicator profileName="Home" onUpdateKey={() => {}} />);
    mocks.emit(snap({ kind: "connected" }));
    expect(await screen.findByText("Connected")).toBeInTheDocument();
    expect(screen.getByText("Home")).toBeInTheDocument();
  });

  it("renders offline with a retry countdown", async () => {
    render(() => <ConnectionIndicator profileName="Home" onUpdateKey={() => {}} />);
    const nextRetryAt = new Date(Date.now() + 5_000).toISOString();
    mocks.emit(snap({ kind: "offline", attempt: 3, nextRetryAt }));
    expect(await screen.findByText("Offline")).toBeInTheDocument();
    expect(screen.getByText(/retrying in [4-6]s/)).toBeInTheDocument();
  });

  it("renders authFailed with an Update key action", async () => {
    const onUpdateKey = vi.fn();
    render(() => <ConnectionIndicator profileName="Home" onUpdateKey={onUpdateKey} />);
    mocks.emit(snap({ kind: "authFailed", failure: { kind: "apiKeyRejected" } }));
    fireEvent.click(await screen.findByRole("button", { name: "Update key" }));
    expect(onUpdateKey).toHaveBeenCalledTimes(1);
  });

  it("renders failed", async () => {
    render(() => <ConnectionIndicator profileName="Home" onUpdateKey={() => {}} />);
    mocks.emit(snap({ kind: "failed", failure: { kind: "timeout" } }));
    await waitFor(() => expect(screen.getByText("Connection failed")).toBeInTheDocument());
  });

  it.each([
    ["unencrypted", "Unencrypted"],
    ["encryptedUnverified", "Encrypted, not verified"],
    ["encryptedVerified", "Encrypted, verified"],
  ] as const)("shows the %s security state", async (security, label) => {
    render(() => <ConnectionIndicator profileName="Home" onUpdateKey={() => {}} />);
    mocks.emit({ ...snap({ kind: "connected" }), security });
    expect(await screen.findByText(label)).toBeInTheDocument();
  });

  it("opens details with the address, security state, and strict setting", async () => {
    render(() => <ConnectionIndicator profileName="Home" onUpdateKey={() => {}} />);
    mocks.emit({
      ...snap({ kind: "connected" }),
      security: "encryptedUnverified",
      finalUrl: "https://stash.example.com",
    });
    fireEvent.click(await screen.findByRole("button", { name: /Encrypted, not verified/ }));
    const details = screen.getByRole("dialog", { name: "Connection details" });
    expect(details).toHaveTextContent("https://stash.example.com");
    expect(details).toHaveTextContent("Encrypted, not verified");
    expect(details).toHaveTextContent("Strict certificate checking");
    expect(details).toHaveTextContent("Off");

    fireEvent.keyDown(document, { key: "Escape" });
    await waitFor(() => expect(screen.queryByRole("dialog")).not.toBeInTheDocument());
  });

  it("is a live status region", () => {
    render(() => <ConnectionIndicator profileName="Home" onUpdateKey={() => {}} />);
    expect(screen.getByRole("status")).toBeInTheDocument();
  });
});
