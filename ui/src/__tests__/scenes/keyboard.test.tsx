import { cleanup } from "@solidjs/testing-library";
import { afterEach, describe, expect, it } from "vitest";
import { nextIndex, registerSceneGridKeys } from "../../scenes/keyboard";
import { listedBindings } from "../../shell/KeyboardHelp";
import { resetKeymap } from "../../shell/keymap";

/** A page of 50 cards in 5 columns. */
const at = (key: string, index: number, ctrl = false) =>
  nextIndex(key, { index, count: 50, cols: 5, ctrl });

afterEach(() => {
  cleanup();
  resetKeymap();
});

describe("nextIndex (paged)", () => {
  it("moves by one and by a row within the page", () => {
    expect(at("ArrowRight", 5)).toEqual({ move: 6 });
    expect(at("ArrowLeft", 5)).toEqual({ move: 4 });
    expect(at("ArrowDown", 5)).toEqual({ move: 10 });
    expect(at("ArrowUp", 5)).toEqual({ move: 0 });
  });

  it("goes to the next or previous page past the page's ends", () => {
    expect(at("ArrowRight", 49)).toEqual({ edge: "next" });
    expect(at("ArrowLeft", 0)).toEqual({ edge: "previous" });
    expect(at("ArrowDown", 47)).toEqual({ edge: "next" });
    expect(at("ArrowUp", 3)).toEqual({ edge: "previous" });
  });

  it("changes page with [ and ], and Home/End stay on the page", () => {
    expect(at("]", 7)).toEqual({ page: "next" });
    expect(at("[", 7)).toEqual({ page: "previous" });
    expect(at("Home", 30)).toEqual({ move: 0 });
    expect(at("End", 3)).toEqual({ move: 49 });
  });

  it("opens with Enter, in a new tab with Ctrl+Enter, and ignores other keys", () => {
    expect(at("Enter", 3)).toEqual({ open: 3, newTab: false });
    expect(at("Enter", 3, true)).toEqual({ open: 3, newTab: true });
    expect(at("a", 3)).toBeNull();
  });
});

it("lists the grid's keys under their own group", () => {
  registerSceneGridKeys();
  const group = listedBindings().find((g) => g.scope === "Scene lists");
  expect(group?.items.map((b) => b.description)).toEqual(
    expect.arrayContaining([
      "Move between scenes",
      "Next page",
      "Previous page",
      "Open the scene",
      "Open in a new tab",
    ]),
  );
});
