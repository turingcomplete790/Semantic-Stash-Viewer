import { cleanup, fireEvent, render } from "@solidjs/testing-library";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { SceneCard } from "../../bindings";
import SceneGrid from "../../scenes/SceneGrid";
import type { GridMode, PageLoadState } from "../../scenes/SceneGrid";

function card(i: number): SceneCard {
  return {
    id: String(i + 1),
    title: `Scene ${i + 1}`,
    date: "2024-05-17",
    durationSeconds: 3725,
    resolution: "1920×1080",
    studio: "Studio One",
    thumb: `ssv-thumb://localhost/scene/${i + 1}?v=1`,
    hasPreview: false,
  };
}

function setup(o: { count?: number; state?: PageLoadState; mode?: GridMode } = {}) {
  const onOpen = vi.fn();
  const onEdge = vi.fn();
  const onPageKey = vi.fn();
  const cards = Array.from({ length: o.count ?? 50 }, (_, i) => card(i));
  const r = render(() => (
    <SceneGrid
      cards={o.state === "ready" || o.state === undefined ? cards : []}
      state={o.state ?? "ready"}
      placeholders={o.count ?? 50}
      mode={o.mode ?? "grid"}
      columns={() => 5}
      onOpen={onOpen}
      onEdge={onEdge}
      onPageKey={onPageKey}
    />
  ));
  const button = (name: string) => r.getByRole("button", { name });
  return { ...r, onOpen, onEdge, onPageKey, button };
}

afterEach(cleanup);

describe("SceneGrid", () => {
  it("shows exactly the page's cards, as a grid or a list", () => {
    const g = setup();
    expect(g.container.querySelectorAll("button.scene-card")).toHaveLength(50);
    expect(g.container.querySelector(".scene-grid")).not.toHaveClass("list");
    cleanup();
    const l = setup({ mode: "list" });
    expect(l.container.querySelector(".scene-grid")).toHaveClass("list");
  });

  it("loads the whole page's thumbnails at once, not lazily (005 T024)", () => {
    const g = setup();
    const img = g.button("Scene 50").querySelector("img") as HTMLImageElement;
    expect(img.getAttribute("src")).toBe("ssv-thumb://localhost/scene/50?v=1");
    // Lazy loading in WebKitGTK costs frames for every thumbnail that arrives mid-scroll.
    expect(img.getAttribute("loading")).toBeNull();
    expect(img.getAttribute("decoding")).toBe("async");
  });

  it("shows placeholders while loading, and says so when the server can't be reached", () => {
    const g = setup({ state: "loading" });
    expect(g.container.querySelectorAll(".scene-card.placeholder")).toHaveLength(50);
    cleanup();
    const u = setup({ state: "unreachable" });
    expect(u.getByText("Can't reach the server")).toBeInTheDocument();
  });

  it("gives the full title as a tooltip", () => {
    const g = setup();
    expect(g.button("Scene 1").querySelector(".scene-card-title")).toHaveAttribute(
      "title",
      "Scene 1",
    );
  });

  it("has one focusable card and moves focus with the arrows", async () => {
    const g = setup();
    expect(g.button("Scene 1").tabIndex).toBe(0);
    expect(g.button("Scene 2").tabIndex).toBe(-1);
    g.button("Scene 1").focus();
    fireEvent.keyDown(g.button("Scene 1"), { key: "ArrowRight" });
    await Promise.resolve();
    expect(document.activeElement).toBe(g.button("Scene 2"));
    fireEvent.keyDown(g.button("Scene 2"), { key: "ArrowDown" });
    await Promise.resolve();
    expect(document.activeElement).toBe(g.button("Scene 7"));
    fireEvent.keyDown(g.button("Scene 7"), { key: "End" });
    await Promise.resolve();
    expect(document.activeElement).toBe(g.button("Scene 50"));
    fireEvent.keyDown(g.button("Scene 50"), { key: "Home" });
    await Promise.resolve();
    expect(document.activeElement).toBe(g.button("Scene 1"));
  });

  it("asks for the next or previous page past the page's ends", () => {
    const g = setup();
    fireEvent.keyDown(g.button("Scene 50"), { key: "ArrowRight" });
    expect(g.onEdge).toHaveBeenLastCalledWith("next");
    fireEvent.keyDown(g.button("Scene 1"), { key: "ArrowLeft" });
    expect(g.onEdge).toHaveBeenLastCalledWith("previous");
    fireEvent.keyDown(g.button("Scene 48"), { key: "ArrowDown" });
    expect(g.onEdge).toHaveBeenLastCalledWith("next");
  });

  it("passes [ and ] on as page keys", () => {
    const g = setup();
    fireEvent.keyDown(g.button("Scene 3"), { key: "]" });
    expect(g.onPageKey).toHaveBeenLastCalledWith("next");
    fireEvent.keyDown(g.button("Scene 3"), { key: "[" });
    expect(g.onPageKey).toHaveBeenLastCalledWith("previous");
  });

  it("opens with Enter, Ctrl+Enter, clicks, and the middle button", () => {
    const g = setup();
    fireEvent.keyDown(g.button("Scene 3"), { key: "Enter" });
    expect(g.onOpen).toHaveBeenLastCalledWith(card(2), false);
    fireEvent.keyDown(g.button("Scene 3"), { key: "Enter", ctrlKey: true });
    expect(g.onOpen).toHaveBeenLastCalledWith(card(2), true);
    fireEvent.click(g.button("Scene 4"));
    expect(g.onOpen).toHaveBeenLastCalledWith(card(3), false);
    fireEvent.click(g.button("Scene 4"), { ctrlKey: true });
    expect(g.onOpen).toHaveBeenLastCalledWith(card(3), true);
    fireEvent(g.button("Scene 5"), new MouseEvent("auxclick", { bubbles: true, button: 1 }));
    expect(g.onOpen).toHaveBeenLastCalledWith(card(4), true);
  });
});
