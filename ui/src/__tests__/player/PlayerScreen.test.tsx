import { cleanup, fireEvent, render, screen, waitFor } from "@solidjs/testing-library";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { PlayerSnapshot, SceneListItem } from "../../bindings";

const mocks = vi.hoisted(() => {
  let handler: ((e: { payload: unknown }) => void) | undefined;
  return {
    emit: (payload: unknown) => handler?.({ payload }),
    commands: {
      playerSnapshot: vi.fn(),
      listRecentScenes: vi.fn(),
      playerOpen: vi.fn(),
      playerClose: vi.fn(),
    },
    events: {
      playerState: {
        listen: vi.fn((h: (e: { payload: unknown }) => void) => {
          handler = h;
          return Promise.resolve(() => {});
        }),
      },
    },
  };
});
vi.mock("../../bindings", () => ({ commands: mocks.commands, events: mocks.events }));

import PlayerScreen from "../../player/PlayerScreen";
import { initPlayer } from "../../player/state";

const idle: PlayerSnapshot = {
  sceneId: null,
  title: null,
  state: "idle",
  positionSeconds: 0,
  durationSeconds: null,
  paused: false,
  speed: 1,
  volume: 100,
  muted: false,
  fullscreen: false,
  hwdec: null,
  tracks: [],
  error: null,
};

function scenes(n: number): SceneListItem[] {
  return Array.from({ length: n }, (_, i) => ({
    id: String(i + 1),
    title: `Scene ${i + 1}`,
    durationSeconds: 3725 + i,
    resolution: "1920×1080",
    videoCodec: "h264",
    container: "mp4",
  }));
}

beforeEach(async () => {
  vi.clearAllMocks();
  mocks.commands.playerSnapshot.mockResolvedValue({ status: "ok", data: idle });
  mocks.commands.listRecentScenes.mockResolvedValue({ status: "ok", data: scenes(20) });
  mocks.commands.playerOpen.mockResolvedValue({
    status: "ok",
    data: { ...idle, state: "loading" },
  });
  mocks.commands.playerClose.mockResolvedValue({ status: "ok", data: null });
  await initPlayer();
  mocks.emit(idle);
});
afterEach(() => {
  cleanup();
  document.documentElement.classList.remove("player-open");
});

describe("PlayerScreen", () => {
  it("lists up to 20 recent scenes with title and duration", async () => {
    render(() => <PlayerScreen onExit={() => {}} />);
    expect(await screen.findByText("Scene 1")).toBeInTheDocument();
    expect(screen.getAllByRole("button", { name: /^Play Scene/ })).toHaveLength(20);
    // 3725 s → 1:02:05
    expect(screen.getByText("1:02:05")).toBeInTheDocument();
  });

  it("plays a scene from the list", async () => {
    render(() => <PlayerScreen onExit={() => {}} />);
    fireEvent.click(await screen.findByRole("button", { name: "Play Scene 3" }));
    expect(mocks.commands.playerOpen).toHaveBeenCalledWith("3");
  });

  it("plays a scene by ID with Enter or the Play button", async () => {
    render(() => <PlayerScreen onExit={() => {}} />);
    const field = screen.getByLabelText("Scene ID");
    fireEvent.input(field, { target: { value: " 42 " } });
    fireEvent.submit(field.closest("form") as HTMLFormElement);
    await waitFor(() => expect(mocks.commands.playerOpen).toHaveBeenCalledWith("42"));
  });

  it("makes the page transparent while a scene is open, and restores it on close", async () => {
    render(() => <PlayerScreen onExit={() => {}} />);
    mocks.emit({ ...idle, sceneId: "1", title: "Scene 1", state: "playing" });
    await waitFor(() =>
      expect(document.documentElement.classList.contains("player-open")).toBe(true),
    );
    fireEvent.click(screen.getByRole("button", { name: "Close player" }));
    expect(mocks.commands.playerClose).toHaveBeenCalled();
    mocks.emit(idle);
    await waitFor(() =>
      expect(document.documentElement.classList.contains("player-open")).toBe(false),
    );
  });

  it("shows a plain-language error with a way back to the list", async () => {
    render(() => <PlayerScreen onExit={() => {}} />);
    mocks.emit({
      ...idle,
      sceneId: "9",
      title: "Broken",
      state: "error",
      error: { kind: "streamUnreachable" },
    });
    expect(await screen.findByText("Couldn't open this scene's video")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Back to scenes" }));
    expect(mocks.commands.playerClose).toHaveBeenCalled();
  });

  it("shows an error when opening fails before playback starts", async () => {
    mocks.commands.playerOpen.mockResolvedValue({
      status: "error",
      error: { kind: "sceneNotFound", id: "999" },
    });
    render(() => <PlayerScreen onExit={() => {}} />);
    fireEvent.input(screen.getByLabelText("Scene ID"), { target: { value: "999" } });
    fireEvent.click(screen.getByRole("button", { name: "Play" }));
    expect(await screen.findByText("That scene doesn't exist")).toBeInTheDocument();
  });
});
