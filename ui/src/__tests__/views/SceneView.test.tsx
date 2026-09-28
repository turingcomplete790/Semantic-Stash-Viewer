import { cleanup, fireEvent, render, screen, waitFor } from "@solidjs/testing-library";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { PlayerSnapshot } from "../../bindings";

const mocks = vi.hoisted(() => {
  let handler: ((e: { payload: unknown }) => void) | undefined;
  const ok = (data: unknown) => Promise.resolve({ status: "ok", data });
  return {
    emit: (payload: unknown) => handler?.({ payload }),
    commands: {
      playerSnapshot: vi.fn(),
      playerOpen: vi.fn(() => ok(null)),
      playerClose: vi.fn(() => ok(null)),
      playerSetViewport: vi.fn(() => ok(null)),
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
import SceneView from "../../views/SceneView";

const playing: PlayerSnapshot = {
  sceneId: "7",
  title: "Scene 7",
  state: "playing",
  positionSeconds: 1,
  durationSeconds: 60,
  paused: false,
  speed: 1,
  volume: 100,
  muted: false,
  fullscreen: false,
  hwdec: null,
  tracks: [],
  error: null,
};

// jsdom has no ResizeObserver: capture callbacks so the test can trigger resizes.
const observers: Array<() => void> = [];
class FakeResizeObserver {
  constructor(private cb: () => void) {
    observers.push(() => this.cb());
  }
  observe() {}
  disconnect() {}
  unobserve() {}
}

let rect = { left: 0, top: 84, width: 1280, height: 600 };

beforeEach(async () => {
  vi.clearAllMocks();
  observers.length = 0;
  rect = { left: 0, top: 84, width: 1280, height: 600 };
  vi.stubGlobal("ResizeObserver", FakeResizeObserver);
  vi.spyOn(HTMLElement.prototype, "getBoundingClientRect").mockImplementation(
    () =>
      ({
        ...rect,
        x: rect.left,
        y: rect.top,
        right: rect.left + rect.width,
        bottom: rect.top + rect.height,
      }) as DOMRect,
  );
  mocks.commands.playerSnapshot.mockResolvedValue({ status: "ok", data: playing });
  await initPlayer();
  mocks.emit(playing);
});
afterEach(() => {
  cleanup();
  document.documentElement.classList.remove("player-open", "player-fullscreen");
  vi.unstubAllGlobals();
  vi.restoreAllMocks();
});

describe("SceneView", () => {
  it("makes the page transparent while its scene plays, and leaves when closed", async () => {
    const onDone = vi.fn();
    render(() => <SceneView sceneId="7" title="Scene 7" onDone={onDone} />);
    await waitFor(() =>
      expect(document.documentElement.classList.contains("player-open")).toBe(true),
    );
    fireEvent.click(screen.getByRole("button", { name: "Close player" }));
    expect(mocks.commands.playerClose).toHaveBeenCalled();
    mocks.emit({ ...playing, sceneId: null, state: "idle" });
    await waitFor(() => expect(onDone).toHaveBeenCalled());
    expect(document.documentElement.classList.contains("player-open")).toBe(false);
  });

  it("shows a plain-language playback error with a way back", async () => {
    render(() => <SceneView sceneId="7" title="Scene 7" onDone={() => {}} />);
    mocks.emit({ ...playing, state: "error", error: { kind: "streamUnreachable" } });
    expect(await screen.findByText("Couldn't open this scene's video")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Back to scenes" }));
    expect(mocks.commands.playerClose).toHaveBeenCalled();
  });

  it("shows an error when opening fails before playback starts", async () => {
    mocks.emit({ ...playing, sceneId: null, state: "idle" });
    mocks.commands.playerOpen.mockResolvedValue({
      status: "error",
      error: { kind: "sceneNotFound", id: "999" },
    } as never);
    const onDone = vi.fn();
    render(() => <SceneView sceneId="999" title="" onDone={onDone} />);
    expect(await screen.findByText("That scene doesn't exist")).toBeInTheDocument();
    fireEvent.click(screen.getByRole("button", { name: "Back to scenes" }));
    expect(onDone).toHaveBeenCalled();
  });
});

describe("SceneView viewport", () => {
  it("reports its video area on mount and on resize", async () => {
    render(() => <SceneView sceneId="7" title="Scene 7" onDone={() => {}} />);
    await waitFor(() =>
      expect(mocks.commands.playerSetViewport).toHaveBeenLastCalledWith({
        x: 0,
        y: 84,
        width: 1280,
        height: 600,
      }),
    );
    rect = { left: 0, top: 84, width: 900, height: 500 };
    observers.forEach((fire) => fire());
    await waitFor(() =>
      expect(mocks.commands.playerSetViewport).toHaveBeenLastCalledWith({
        x: 0,
        y: 84,
        width: 900,
        height: 500,
      }),
    );
  });

  it("uses the whole window in fullscreen", async () => {
    render(() => <SceneView sceneId="7" title="Scene 7" onDone={() => {}} />);
    mocks.emit({ ...playing, fullscreen: true });
    await waitFor(() => expect(mocks.commands.playerSetViewport).toHaveBeenLastCalledWith(null));
  });

  it("opens its scene when another one (or none) is loaded", async () => {
    mocks.emit({ ...playing, sceneId: null, state: "idle" });
    render(() => <SceneView sceneId="7" title="Scene 7" onDone={() => {}} />);
    await waitFor(() => expect(mocks.commands.playerOpen).toHaveBeenCalledWith("7"));
  });
});
