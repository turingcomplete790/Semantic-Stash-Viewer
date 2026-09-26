import { beforeEach, describe, expect, it, vi } from "vitest";
import type { PlayerSnapshot } from "../../bindings";

const commands = vi.hoisted(() => {
  const ok = () => Promise.resolve({ status: "ok", data: null });
  return {
    playerTogglePause: vi.fn(ok),
    playerSeekRelative: vi.fn(ok),
    playerSetVolume: vi.fn(ok),
    playerSetSpeed: vi.fn(ok),
    playerFrameStep: vi.fn(ok),
    playerSetFullscreen: vi.fn(ok),
    playerSetMuted: vi.fn(ok),
    playerClose: vi.fn(ok),
  };
});
vi.mock("../../bindings", () => ({ commands }));

import { handlePlayerKey } from "../../player/keyboard";

const base: PlayerSnapshot = {
  sceneId: "1",
  title: "Scene",
  state: "playing",
  positionSeconds: 30,
  durationSeconds: 600,
  paused: false,
  speed: 1,
  volume: 50,
  muted: false,
  fullscreen: false,
  hwdec: null,
  tracks: [],
  error: null,
};

function press(key: string, snapshot: Partial<PlayerSnapshot> = {}, target?: HTMLElement) {
  const event = new KeyboardEvent("keydown", { key, cancelable: true });
  if (target) Object.defineProperty(event, "target", { value: target });
  const handled = handlePlayerKey(event, { ...base, ...snapshot });
  return { handled, prevented: event.defaultPrevented };
}

beforeEach(() => vi.clearAllMocks());

describe("player keyboard map (FR-009)", () => {
  it("space toggles pause and prevents page scrolling", () => {
    const r = press(" ");
    expect(commands.playerTogglePause).toHaveBeenCalled();
    expect(r).toEqual({ handled: true, prevented: true });
  });

  it("left/right seek ±10 s", () => {
    press("ArrowLeft");
    expect(commands.playerSeekRelative).toHaveBeenLastCalledWith(-10);
    press("ArrowRight");
    expect(commands.playerSeekRelative).toHaveBeenLastCalledWith(10);
  });

  it("up/down change volume by 5, clamped to 0–100", () => {
    press("ArrowUp", { volume: 50 });
    expect(commands.playerSetVolume).toHaveBeenLastCalledWith(55);
    press("ArrowDown", { volume: 50 });
    expect(commands.playerSetVolume).toHaveBeenLastCalledWith(45);
    press("ArrowUp", { volume: 98 });
    expect(commands.playerSetVolume).toHaveBeenLastCalledWith(100);
    press("ArrowDown", { volume: 3 });
    expect(commands.playerSetVolume).toHaveBeenLastCalledWith(0);
  });

  it("[ and ] step through the speed list; \\ resets to 1×", () => {
    press("]", { speed: 1 });
    expect(commands.playerSetSpeed).toHaveBeenLastCalledWith(1.25);
    press("[", { speed: 1 });
    expect(commands.playerSetSpeed).toHaveBeenLastCalledWith(0.75);
    press("]", { speed: 2 });
    expect(commands.playerSetSpeed).toHaveBeenLastCalledWith(3);
    press("]", { speed: 4 });
    expect(commands.playerSetSpeed).toHaveBeenLastCalledWith(4);
    press("[", { speed: 0.25 });
    expect(commands.playerSetSpeed).toHaveBeenLastCalledWith(0.25);
    press("\\", { speed: 3 });
    expect(commands.playerSetSpeed).toHaveBeenLastCalledWith(1);
  });

  it(", and . step frames only while paused", () => {
    press(".", { paused: false });
    press(",", { paused: false });
    expect(commands.playerFrameStep).not.toHaveBeenCalled();
    press(".", { paused: true });
    expect(commands.playerFrameStep).toHaveBeenLastCalledWith("forward");
    press(",", { paused: true });
    expect(commands.playerFrameStep).toHaveBeenLastCalledWith("back");
  });

  it("f toggles fullscreen and m toggles mute", () => {
    press("f", { fullscreen: false });
    expect(commands.playerSetFullscreen).toHaveBeenLastCalledWith(true);
    press("f", { fullscreen: true });
    expect(commands.playerSetFullscreen).toHaveBeenLastCalledWith(false);
    press("m", { muted: false });
    expect(commands.playerSetMuted).toHaveBeenLastCalledWith(true);
  });

  it("Escape leaves fullscreen first, otherwise closes the player", () => {
    press("Escape", { fullscreen: true });
    expect(commands.playerSetFullscreen).toHaveBeenLastCalledWith(false);
    expect(commands.playerClose).not.toHaveBeenCalled();
    press("Escape", { fullscreen: false });
    expect(commands.playerClose).toHaveBeenCalled();
  });

  it("ignores keys while typing in an input", () => {
    const input = document.createElement("input");
    const r = press(" ", {}, input);
    expect(r.handled).toBe(false);
    expect(commands.playerTogglePause).not.toHaveBeenCalled();
  });

  it("still handles keys when a slider (e.g. the seek bar) has focus", () => {
    const slider = document.createElement("input");
    slider.type = "range";
    const r = press("ArrowRight", {}, slider);
    expect(r).toEqual({ handled: true, prevented: true });
    expect(commands.playerSeekRelative).toHaveBeenLastCalledWith(10);
  });

  it("ignores keys when no scene is open", () => {
    const r = press(" ", { state: "idle" });
    expect(r.handled).toBe(false);
  });

  it("leaves unrelated keys alone", () => {
    expect(press("q").handled).toBe(false);
  });
});
