import { cleanup, fireEvent, render, screen, waitFor } from "@solidjs/testing-library";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ProfileSummary } from "../bindings";

const commands = vi.hoisted(() => ({ updateProfile: vi.fn() }));
vi.mock("../bindings", () => ({ commands }));

import KeyPrompt from "../components/KeyPrompt";

const profile: ProfileSummary = {
  id: "p1",
  displayName: "Home",
  baseUrl: "https://stash.example.com",
  strictTls: true,
  apiKey: "old-key",
  lastUsedAt: null,
};

beforeEach(() => vi.resetAllMocks());
afterEach(cleanup);

describe("KeyPrompt", () => {
  it("prefills the current key as plain text and saves the whole profile with the new key", async () => {
    const updated = { ...profile, apiKey: "new-key" };
    commands.updateProfile.mockResolvedValue({ status: "ok", data: updated });
    const onDone = vi.fn();
    render(() => <KeyPrompt profile={profile} onDone={onDone} onCancel={() => {}} />);

    const field = screen.getByLabelText("API key");
    expect(field).toHaveValue("old-key");
    expect(field).toHaveAttribute("type", "text");

    fireEvent.input(field, { target: { value: " new-key " } });
    fireEvent.click(screen.getByRole("button", { name: "Save and reconnect" }));

    await waitFor(() => expect(onDone).toHaveBeenCalledWith(updated));
    expect(commands.updateProfile).toHaveBeenCalledWith(
      "p1",
      {
        address: "https://stash.example.com",
        apiKey: "new-key",
        displayName: "Home",
        strictTls: true,
      },
      false,
    );
  });

  it("keeps the dialog open and shows why when the new key is rejected too", async () => {
    commands.updateProfile.mockResolvedValue({
      status: "error",
      error: { kind: "connect", failure: { kind: "apiKeyRejected" } },
    });
    const onDone = vi.fn();
    render(() => <KeyPrompt profile={profile} onDone={onDone} onCancel={() => {}} />);

    fireEvent.click(screen.getByRole("button", { name: "Save and reconnect" }));
    expect(await screen.findByText("The API key was rejected")).toBeInTheDocument();
    expect(onDone).not.toHaveBeenCalled();
    expect(screen.getByRole("dialog")).toBeInTheDocument();
  });

  it("closes on Escape", () => {
    const onCancel = vi.fn();
    render(() => <KeyPrompt profile={profile} onDone={() => {}} onCancel={onCancel} />);
    fireEvent.keyDown(screen.getByRole("dialog"), { key: "Escape" });
    expect(onCancel).toHaveBeenCalled();
  });
});
