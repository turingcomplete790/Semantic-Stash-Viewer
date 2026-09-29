import { afterEach, describe, expect, it, vi } from "vitest";
import {
  bindings,
  chordName,
  dispatch,
  register,
  registerRawHandler,
  resetKeymap,
  setSceneActive,
} from "../../shell/keymap";

function key(k: string, init: KeyboardEventInit = {}, target?: HTMLElement) {
  const e = new KeyboardEvent("keydown", { key: k, cancelable: true, ...init });
  if (target) Object.defineProperty(e, "target", { value: target });
  return e;
}

afterEach(() => resetKeymap());

describe("keymap", () => {
  it("runs chords, even while typing", () => {
    const run = vi.fn();
    register({ id: "new-tab", keys: ["Ctrl+T"], scope: "shell", description: "New tab", run });
    const input = document.createElement("input");
    const e = key("t", { ctrlKey: true }, input);
    expect(dispatch(e)).toBe(true);
    expect(run).toHaveBeenCalledOnce();
    expect(e.defaultPrevented).toBe(true);
  });

  it("names chords with modifiers in a fixed order", () => {
    expect(chordName(key("T", { ctrlKey: true, shiftKey: true }))).toBe("Ctrl+Shift+T");
    expect(chordName(key("ArrowLeft", { altKey: true }))).toBe("Alt+ArrowLeft");
    expect(chordName(key("Tab", { ctrlKey: true }))).toBe("Ctrl+Tab");
  });

  it("runs two-key sequences within a second, but not while typing", () => {
    const run = vi.fn();
    register({ id: "scenes", keys: ["g s"], scope: "shell", description: "Scenes", run });
    dispatch(key("g"));
    dispatch(key("s"));
    expect(run).toHaveBeenCalledOnce();

    const input = document.createElement("input");
    dispatch(key("g", {}, input));
    dispatch(key("s", {}, input));
    expect(run).toHaveBeenCalledOnce();
  });

  it("forgets a sequence prefix after the timeout", () => {
    vi.useFakeTimers();
    const run = vi.fn();
    register({ id: "scenes", keys: ["g s"], scope: "shell", description: "Scenes", run });
    dispatch(key("g"));
    vi.advanceTimersByTime(1500);
    dispatch(key("s"));
    expect(run).not.toHaveBeenCalled();
    vi.useRealTimers();
  });

  it("runs scene bindings and raw handlers only while a scene is active", () => {
    const raw = vi.fn(() => true);
    registerRawHandler("scene", raw);
    expect(dispatch(key(" "))).toBe(false);
    setSceneActive(() => true);
    expect(dispatch(key(" "))).toBe(true);
    expect(raw).toHaveBeenCalledOnce();
  });

  it("lists every registered binding, including display-only ones", () => {
    register({
      id: "help",
      keys: ["?"],
      scope: "shell",
      description: "Keyboard help",
      run: () => {},
    });
    register({ id: "pause", keys: ["Space"], scope: "scene", description: "Play / pause" });
    expect(bindings().map((b) => b.id)).toEqual(["help", "pause"]);
  });
});
