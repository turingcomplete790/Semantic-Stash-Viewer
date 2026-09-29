import { cleanup, fireEvent, render, screen, waitFor } from "@solidjs/testing-library";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { PlayerSnapshot } from "../../bindings";

const mocks = vi.hoisted(() => {
  let handler: ((e: { payload: unknown }) => void) | undefined;
  const ok = () => Promise.resolve({ status: "ok", data: null });
  return {
    emit: (payload: unknown) => handler?.({ payload }),
    commands: {
      playerSnapshot: vi.fn(),
      playerOpen: vi.fn(ok),
      playerClose: vi.fn(ok),
      playerTogglePause: vi.fn(ok),
      playerSetViewport: vi.fn(ok),
      playerSetVideoVisible: vi.fn(ok),
      shellLoadTabs: vi.fn(),
      shellSaveTabs: vi.fn(ok),
      sceneScreenshotUrl: vi.fn(() =>
        Promise.resolve({ status: "ok", data: "data:image/jpeg;base64,/9j/" }),
      ),
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
import NowPlayingBar from "../../shell/NowPlayingBar";
import { close, navigate, resetTabs, select, tabs } from "../../shell/tabs";
import SceneView from "../../views/SceneView";

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
const playing: PlayerSnapshot = { ...idle, sceneId: "7", title: "Beach day", state: "playing" };

beforeEach(async () => {
  vi.clearAllMocks();
  resetTabs();
  mocks.commands.playerSnapshot.mockResolvedValue({ status: "ok", data: idle });
  await initPlayer();
  mocks.emit(idle);
  // Tab 0: the scene. Tab 1: Scenes.
  navigate({ kind: "scene", sceneId: "7", title: "Beach day" });
});
afterEach(cleanup);

describe("NowPlayingBar", () => {
  it("appears when the playing scene's tab isn't selected", async () => {
    render(() => <NowPlayingBar />);
    mocks.emit(playing);
    expect(screen.queryByRole("region", { name: "Now playing" })).not.toBeInTheDocument();
    navigate({ kind: "scenes" }, { newTab: true });
    const bar = await screen.findByRole("region", { name: "Now playing" });
    expect(bar).toHaveTextContent("Beach day");
    fireEvent.click(screen.getByRole("button", { name: "Pause" }));
    expect(mocks.commands.playerTogglePause).toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Back to scene" }));
    expect(screen.queryByRole("region", { name: "Now playing" })).not.toBeInTheDocument();
  });

  it("stays hidden when idle or in fullscreen", async () => {
    render(() => <NowPlayingBar />);
    navigate({ kind: "scenes" }, { newTab: true });
    expect(screen.queryByRole("region", { name: "Now playing" })).not.toBeInTheDocument();
    mocks.emit({ ...playing, fullscreen: true });
    expect(screen.queryByRole("region", { name: "Now playing" })).not.toBeInTheDocument();
  });

  it("hides the video while another tab is shown, and shows it again on return", async () => {
    render(() => <NowPlayingBar />);
    mocks.emit(playing);
    navigate({ kind: "scenes" }, { newTab: true });
    await waitFor(() =>
      expect(mocks.commands.playerSetVideoVisible).toHaveBeenLastCalledWith(false),
    );
    select(0);
    await waitFor(() =>
      expect(mocks.commands.playerSetVideoVisible).toHaveBeenLastCalledWith(true),
    );
  });

  it("closing the playing scene's tab stops playback", () => {
    mocks.emit(playing);
    navigate({ kind: "scenes" }, { newTab: true });
    close(tabs()[0].id);
    expect(mocks.commands.playerClose).toHaveBeenCalled();
  });
});

describe("restored scene tabs", () => {
  it("show the title and a Play button, and never play on their own", async () => {
    render(() => <SceneView sceneId="7" title="Beach day" restored onDone={() => {}} />);
    expect(screen.getByText("Beach day")).toBeInTheDocument();
    expect(mocks.commands.playerOpen).not.toHaveBeenCalled();
    // A still of the scene sits under the Play button.
    await waitFor(() =>
      expect(document.querySelector("img.player-poster")).toHaveAttribute(
        "src",
        "data:image/jpeg;base64,/9j/",
      ),
    );
    expect(mocks.commands.sceneScreenshotUrl).toHaveBeenCalledWith("7");
    fireEvent.click(screen.getByRole("button", { name: "Play" }));
    expect(mocks.commands.playerOpen).toHaveBeenCalledWith("7");
  });

  it("don't interrupt another scene that's playing", () => {
    mocks.emit({ ...playing, sceneId: "99", title: "Other" });
    render(() => <SceneView sceneId="7" title="Beach day" restored onDone={() => {}} />);
    expect(mocks.commands.playerOpen).not.toHaveBeenCalled();
    expect(screen.getByRole("button", { name: "Play" })).toBeInTheDocument();
  });
});
