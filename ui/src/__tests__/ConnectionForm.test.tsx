import { cleanup, fireEvent, render, screen, waitFor } from "@solidjs/testing-library";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { AppError, ProfileSummary, TestResult } from "../bindings";

type Res<T> = { status: "ok"; data: T } | { status: "error"; error: AppError };

const commands = vi.hoisted(() => ({
  testConnection: vi.fn(),
  cancelRequest: vi.fn(),
  createProfile: vi.fn(),
}));
vi.mock("../bindings", () => ({ commands }));

import ConnectionForm from "../components/ConnectionForm";

const okResult: TestResult = {
  normalizedUrl: "http://localhost:9999",
  security: "unencrypted",
  server: {
    version: "v0.31.1",
    versionStatus: "supported",
    appSchema: 85,
    identity: "0000000000000000",
    counts: { scenes: 27552, images: 25632, galleries: 739, performers: 575 },
  },
};

const saved: ProfileSummary = {
  id: "p1",
  displayName: "localhost:9999",
  baseUrl: "http://localhost:9999",
  strictTls: false,
  apiKey: "my-key",
  lastUsedAt: null,
};

function ok<T>(data: T): Promise<Res<T>> {
  return Promise.resolve({ status: "ok", data });
}
function fail(error: AppError): Promise<Res<never>> {
  return Promise.resolve({ status: "error", error });
}

function fill(address: string, key = "") {
  fireEvent.input(screen.getByLabelText("Server address"), { target: { value: address } });
  fireEvent.input(screen.getByLabelText("API key (optional)"), { target: { value: key } });
}

beforeEach(() => {
  vi.resetAllMocks();
  commands.cancelRequest.mockReturnValue(ok(null));
});
afterEach(cleanup);

describe("ConnectionForm", () => {
  it("enters the API key in a plain text field and submits the draft", async () => {
    commands.testConnection.mockReturnValue(ok(okResult));
    render(() => <ConnectionForm onSaved={() => {}} />);

    const keyField = screen.getByLabelText("API key (optional)");
    expect(keyField).toHaveAttribute("type", "text");

    fill("localhost:9999", "my-key");
    fireEvent.click(screen.getByRole("button", { name: "Connect" }));

    await waitFor(() => expect(commands.testConnection).toHaveBeenCalledTimes(1));
    const [draft, requestId] = commands.testConnection.mock.calls[0];
    expect(draft).toEqual({
      address: "localhost:9999",
      apiKey: "my-key",
      displayName: null,
      strictTls: false,
    });
    expect(typeof requestId).toBe("string");
  });

  it("has strict certificate checking off by default and sends it when turned on", async () => {
    commands.testConnection.mockReturnValue(ok(okResult));
    render(() => <ConnectionForm onSaved={() => {}} />);

    const strict = screen.getByLabelText("Verify certificate (strict)");
    expect(strict).not.toBeChecked();
    fireEvent.click(strict);
    fill("https://stash.example.com");
    fireEvent.click(screen.getByRole("button", { name: "Connect" }));

    await waitFor(() => expect(commands.testConnection).toHaveBeenCalledTimes(1));
    expect(commands.testConnection.mock.calls[0][0].strictTls).toBe(true);
  });

  it("shows the connection security in the result", async () => {
    commands.testConnection.mockReturnValue(ok(okResult));
    render(() => <ConnectionForm onSaved={() => {}} />);
    fill("localhost:9999");
    fireEvent.click(screen.getByRole("button", { name: "Connect" }));
    expect(await screen.findByText("Unencrypted")).toBeInTheDocument();
  });

  it("shows Connecting immediately with a working Cancel", async () => {
    let resolve!: (r: Res<TestResult>) => void;
    commands.testConnection.mockReturnValue(new Promise((r) => (resolve = r)));
    render(() => <ConnectionForm onSaved={() => {}} />);

    fill("localhost:9999");
    fireEvent.click(screen.getByRole("button", { name: "Connect" }));

    expect(await screen.findByText(/Connecting/)).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Cancel" }));
    const requestId = commands.testConnection.mock.calls[0][1];
    expect(commands.cancelRequest).toHaveBeenCalledWith(requestId);

    resolve({ status: "error", error: { kind: "cancelled" } });
    await waitFor(() => expect(screen.queryByText(/Connecting/)).not.toBeInTheDocument());
  });

  it("shows the failure message and keeps the typed address and key", async () => {
    commands.testConnection.mockReturnValue(
      fail({ kind: "connect", failure: { kind: "apiKeyRejected" } }),
    );
    render(() => <ConnectionForm onSaved={() => {}} />);

    fill("stash.local:9999", "wrong-key");
    fireEvent.click(screen.getByRole("button", { name: "Connect" }));

    expect(await screen.findByText("The API key was rejected")).toBeInTheDocument();
    expect(screen.getByLabelText("Server address")).toHaveValue("stash.local:9999");
    expect(screen.getByLabelText("API key (optional)")).toHaveValue("wrong-key");
  });

  it("offers Connect without a key for apiKeyInvalidButNotRequired", async () => {
    commands.testConnection
      .mockReturnValueOnce(
        fail({ kind: "connect", failure: { kind: "apiKeyInvalidButNotRequired" } }),
      )
      .mockReturnValueOnce(ok(okResult));
    render(() => <ConnectionForm onSaved={() => {}} />);

    fill("localhost:9999", "bogus");
    fireEvent.click(screen.getByRole("button", { name: "Connect" }));
    fireEvent.click(await screen.findByRole("button", { name: "Connect without a key" }));

    await waitFor(() => expect(commands.testConnection).toHaveBeenCalledTimes(2));
    expect(commands.testConnection.mock.calls[1][0].apiKey).toBeNull();
    expect(screen.getByLabelText("API key (optional)")).toHaveValue("");
  });

  it("shows the version and the four counts on success, then saves", async () => {
    commands.testConnection.mockReturnValue(ok(okResult));
    commands.createProfile.mockReturnValue(ok(saved));
    const onSaved = vi.fn();
    render(() => <ConnectionForm onSaved={onSaved} />);

    fill("localhost:9999", "my-key");
    fireEvent.click(screen.getByRole("button", { name: "Connect" }));

    expect(await screen.findByText("v0.31.1")).toBeInTheDocument();
    for (const n of ["27,552", "25,632", "739", "575"]) {
      expect(screen.getByText(n)).toBeInTheDocument();
    }
    expect(screen.getByText("http://localhost:9999")).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: "Save and continue" }));
    await waitFor(() => expect(onSaved).toHaveBeenCalledWith(saved, okResult));
    expect(commands.createProfile.mock.calls[0][0].apiKey).toBe("my-key");
  });
});
