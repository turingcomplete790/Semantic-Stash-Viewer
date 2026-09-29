import { cleanup, fireEvent, render, screen } from "@solidjs/testing-library";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { PlayerSnapshot } from "../../bindings";

const mocks = vi.hoisted(() => {
  let handler: ((e: { payload: unknown }) => void) | undefined;
  const ok = () => Promise.resolve({ status: "ok", data: null });
  return {
    emit: (payload: unknown) => handler?.({ payload }),
    commands: {
      playerSnapshot: vi.fn(),
      playerTogglePause: vi.fn(ok),
      playerSeek: vi.fn(ok),
      playerSeekRelative: vi.fn(ok),
      playerSetSpeed: vi.fn(ok),
      playerSetVolume: vi.fn(ok),
      playerSetMuted: vi.fn(ok),
      playerFrameStep: vi.fn(ok),
      playerReplay: vi.fn(ok),
      playerSetFullscreen: vi.fn(ok),
      playerClose: vi.fn(ok),
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

import Controls, { SPEEDS } from "../../player/Controls";
import { initPlayer } from "../../player/state";

const playing: PlayerSnapshot = {
  sceneId: "1",
  title: "Scene 1",
  state: "playing",
  positionSeconds: 30,
  durationSeconds: 600,
  paused: false,
  speed: 1,
  volume: 80,
  muted: false,
  fullscreen: false,
  hwdec: "vaapi",
  tracks: [],
  error: null,
};

beforeEach(async () => {
  vi.clearAllMocks();
  mocks.commands.playerSnapshot.mockResolvedValue({ status: "ok", data: playing });
  await initPlayer();
  mocks.emit(playing);
});
afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

describe("Controls", () => {
  it("wires each button to its command", () => {
    render(() => <Controls />);
    fireEvent.click(screen.getByRole("button", { name: "Pause" }));
    expect(mocks.commands.playerTogglePause).toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "Back 10 seconds" }));
    expect(mocks.commands.playerSeekRelative).toHaveBeenCalledWith(-10);
    fireEvent.click(screen.getByRole("button", { name: "Forward 10 seconds" }));
    expect(mocks.commands.playerSeekRelative).toHaveBeenCalledWith(10);
    fireEvent.click(screen.getByRole("button", { name: "Mute" }));
    expect(mocks.commands.playerSetMuted).toHaveBeenCalledWith(true);
    fireEvent.click(screen.getByRole("button", { name: "Fullscreen" }));
    expect(mocks.commands.playerSetFullscreen).toHaveBeenCalledWith(true);
  });

  it("shows current time and duration", () => {
    render(() => <Controls />);
    expect(screen.getByText("0:30")).toBeInTheDocument();
    expect(screen.getByText("10:00")).toBeInTheDocument();
  });

  it("seeks fast while dragging and exactly on release", () => {
    render(() => <Controls />);
    const bar = screen.getByRole("slider", { name: "Seek" });
    fireEvent.input(bar, { target: { value: "120" } });
    expect(mocks.commands.playerSeek).toHaveBeenLastCalledWith(120, false);
    fireEvent.change(bar, { target: { value: "150" } });
    expect(mocks.commands.playerSeek).toHaveBeenLastCalledWith(150, true);
  });

  it("keeps showing the seek target until mpv reports it has arrived", () => {
    render(() => <Controls />);
    const bar = screen.getByRole("slider", { name: "Seek" }) as HTMLInputElement;
    fireEvent.change(bar, { target: { value: "300" } }); // click at 5:00
    // mpv hasn't got there yet: still reports 0:30.
    mocks.emit({ ...playing, positionSeconds: 30 });
    expect(bar.value).toBe("300");
    expect(screen.getByText("5:00")).toBeInTheDocument();
    // Arrived: the bar follows mpv again.
    mocks.emit({ ...playing, positionSeconds: 300.4 });
    mocks.emit({ ...playing, positionSeconds: 305 });
    expect(bar.value).toBe("305");
  });

  it("±10 s buttons move the bar immediately", () => {
    render(() => <Controls />);
    fireEvent.click(screen.getByRole("button", { name: "Forward 10 seconds" }));
    expect(screen.getByText("0:40")).toBeInTheDocument();
  });

  it("gives up on a seek target after 5 s", () => {
    vi.useFakeTimers();
    render(() => <Controls />);
    const bar = screen.getByRole("slider", { name: "Seek" }) as HTMLInputElement;
    fireEvent.change(bar, { target: { value: "300" } });
    mocks.emit({ ...playing, positionSeconds: 31 });
    expect(bar.value).toBe("300");
    vi.advanceTimersByTime(5100);
    expect(bar.value).toBe("31");
  });

  it("shows the time under the pointer when hovering the seek bar", () => {
    render(() => <Controls />);
    const bar = screen.getByRole("slider", { name: "Seek" });
    bar.getBoundingClientRect = () =>
      ({ left: 0, width: 200, top: 0, height: 10, right: 200, bottom: 10 }) as DOMRect;
    fireEvent.mouseMove(bar, { clientX: 50 }); // 25% of 600 s
    expect(screen.getByText("2:30")).toBeInTheDocument();
  });

  it("moves the hover time with a transform, not a layout property", () => {
    render(() => <Controls />);
    const bar = screen.getByRole("slider", { name: "Seek" });
    bar.getBoundingClientRect = () =>
      ({ left: 0, width: 200, top: 0, height: 10, right: 200, bottom: 10 }) as DOMRect;
    fireEvent.mouseMove(bar, { clientX: 50 });
    const anchor = screen.getByText("2:30").parentElement as HTMLElement;
    expect(anchor.style.transform).toBe("translateX(25%)");
    expect(anchor.style.left).toBe("");
    expect(screen.getByText("2:30").style.left).toBe("");
  });

  it("offers the speed list and sets the chosen speed", () => {
    render(() => <Controls />);
    const menu = screen.getByRole("combobox", { name: "Playback speed" });
    const values = Array.from((menu as HTMLSelectElement).options).map((o) => Number(o.value));
    expect(values).toEqual([0.25, 0.5, 0.75, 1, 1.25, 1.5, 2, 3, 4]);
    expect(SPEEDS).toEqual(values);
    fireEvent.change(menu, { target: { value: "1.5" } });
    expect(mocks.commands.playerSetSpeed).toHaveBeenCalledWith(1.5);
  });

  it("sets the volume", () => {
    render(() => <Controls />);
    fireEvent.input(screen.getByRole("slider", { name: "Volume" }), { target: { value: "35" } });
    expect(mocks.commands.playerSetVolume).toHaveBeenCalledWith(35);
  });

  it("shows frame-step buttons only while paused", () => {
    render(() => <Controls />);
    expect(screen.queryByRole("button", { name: "Next frame" })).not.toBeInTheDocument();
    mocks.emit({ ...playing, paused: true, state: "paused" });
    fireEvent.click(screen.getByRole("button", { name: "Next frame" }));
    expect(mocks.commands.playerFrameStep).toHaveBeenCalledWith("forward");
    fireEvent.click(screen.getByRole("button", { name: "Previous frame" }));
    expect(mocks.commands.playerFrameStep).toHaveBeenCalledWith("back");
  });

  it("shows Replay at the end", () => {
    render(() => <Controls />);
    mocks.emit({ ...playing, state: "ended", paused: true, positionSeconds: 600 });
    fireEvent.click(screen.getByRole("button", { name: "Replay" }));
    expect(mocks.commands.playerReplay).toHaveBeenCalled();
  });

  it("hides controls and cursor after 3 s idle while playing, and shows them on mouse move", () => {
    vi.useFakeTimers();
    const { container } = render(() => <Controls />);
    const root = container.querySelector(".player-controls-root") as HTMLElement;
    expect(root).not.toHaveClass("idle");
    vi.advanceTimersByTime(3100);
    expect(root).toHaveClass("idle");
    fireEvent.mouseMove(document);
    expect(root).not.toHaveClass("idle");
  });

  it("keeps controls visible while paused", () => {
    vi.useFakeTimers();
    mocks.emit({ ...playing, paused: true, state: "paused" });
    const { container } = render(() => <Controls />);
    vi.advanceTimersByTime(5000);
    expect(container.querySelector(".player-controls-root")).not.toHaveClass("idle");
  });
});
