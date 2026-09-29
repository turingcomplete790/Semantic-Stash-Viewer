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
      listTestScenes: vi.fn(),
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

import { initPlayer } from "../../player/state";
import { currentRoute, resetTabs } from "../../shell/tabs";
import ScenesView from "../../views/ScenesView";

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
  resetTabs();
  mocks.commands.playerSnapshot.mockResolvedValue({ status: "ok", data: idle });
  mocks.commands.listRecentScenes.mockResolvedValue({ status: "ok", data: scenes(20) });
  mocks.commands.listTestScenes.mockResolvedValue({
    status: "ok",
    data: [
      {
        label: "4K HEVC",
        scenes: [
          {
            id: "401",
            title: "Big file",
            durationSeconds: 600,
            resolution: "3840×2160",
            videoCodec: "hevc",
            container: "mp4",
          },
        ],
      },
      {
        label: "WMV above 720p",
        scenes: [
          {
            id: "501",
            title: "Old file",
            durationSeconds: 60,
            resolution: "1920×1080",
            videoCodec: "wmv3",
            container: "wmv",
          },
        ],
      },
    ],
  });
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

describe("ScenesView", () => {
  it("lists up to 20 recent scenes with title and duration", async () => {
    render(() => <ScenesView />);
    expect(await screen.findByText("Scene 1")).toBeInTheDocument();
    expect(screen.getAllByRole("button", { name: /^Play Scene/ })).toHaveLength(20);
    // 3725 s → 1:02:05
    expect(screen.getByText("1:02:05")).toBeInTheDocument();
  });

  it("shows the test set grouped by format and opens a scene from it", async () => {
    render(() => <ScenesView />);
    const wmv = await screen.findByRole("region", { name: "WMV above 720p" });
    expect(wmv).toHaveTextContent("1920×1080 · wmv3 · wmv");
    expect(screen.getByRole("region", { name: "4K HEVC" })).toHaveTextContent("3840×2160");
    fireEvent.click(screen.getByRole("button", { name: "Play Old file" }));
    expect(currentRoute()).toEqual({ kind: "scene", sceneId: "501", title: "Old file" });
  });

  it("Shuffle reloads the test set", async () => {
    render(() => <ScenesView />);
    await screen.findByRole("region", { name: "4K HEVC" });
    expect(mocks.commands.listTestScenes).toHaveBeenCalledTimes(1);
    fireEvent.click(screen.getByRole("button", { name: "Shuffle" }));
    await waitFor(() => expect(mocks.commands.listTestScenes).toHaveBeenCalledTimes(2));
  });

  it("opens a scene from the list", async () => {
    render(() => <ScenesView />);
    fireEvent.click(await screen.findByRole("button", { name: "Play Scene 3" }));
    expect(currentRoute()).toEqual({ kind: "scene", sceneId: "3", title: "Scene 3" });
  });

  it("opens a scene by ID with Enter or the Play button", async () => {
    render(() => <ScenesView />);
    const field = screen.getByLabelText("Scene ID");
    fireEvent.input(field, { target: { value: " 42 " } });
    fireEvent.submit(field.closest("form") as HTMLFormElement);
    expect(currentRoute()).toEqual({ kind: "scene", sceneId: "42", title: "" });
  });
});
