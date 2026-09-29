import { cleanup, fireEvent, render, screen } from "@solidjs/testing-library";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const mocks = vi.hoisted(() => ({
  commands: {
    shellLoadTabs: vi.fn(),
    shellSaveTabs: vi.fn(() => Promise.resolve({ status: "ok", data: null })),
    playerClose: vi.fn(() => Promise.resolve({ status: "ok", data: null })),
  },
  events: { playerState: { listen: vi.fn(() => Promise.resolve(() => {})) } },
}));
vi.mock("../../bindings", () => ({ commands: mocks.commands, events: mocks.events }));

import TabPanes, { MAX_MOUNTED_TABS } from "../../shell/TabPanes";
import { back, navigate, resetTabs, select, tabs } from "../../shell/tabs";
import type { Route } from "../../shell/routes";
import { useViewState } from "../../shell/viewState";

const mounts: string[] = [];

function Notes(props: { label: string }) {
  mounts.push(props.label);
  const [text, setText] = useViewState("text", "");
  return (
    <input
      aria-label={`Notes ${props.label}`}
      value={text()}
      onInput={(e) => setText(e.currentTarget.value)}
    />
  );
}

function label(route: Route) {
  return route.kind === "scene" ? `scene-${route.sceneId}` : route.kind;
}

beforeEach(() => {
  vi.clearAllMocks();
  mounts.length = 0;
  resetTabs();
});
afterEach(cleanup);

describe("tab panes", () => {
  it("keeps a background tab's view as it was", () => {
    render(() => <TabPanes render={(route) => <Notes label={label(route)} />} />);
    fireEvent.input(screen.getByLabelText("Notes home"), { target: { value: "draft" } });
    navigate({ kind: "scenes" }, { newTab: true });
    expect(screen.getByLabelText("Notes home").closest("[hidden]")).not.toBeNull();
    select(0);
    expect(screen.getByLabelText("Notes home")).toHaveValue("draft");
    expect(mounts.filter((m) => m === "home")).toHaveLength(1); // not remounted
  });

  it("restores each pane's scroll position when shown again", async () => {
    render(() => <TabPanes render={(route) => <Notes label={label(route)} />} />);
    const pane = screen.getByLabelText("Notes home").closest(".tab-pane") as HTMLElement;
    pane.scrollTop = 420;
    fireEvent.scroll(pane);
    navigate({ kind: "scenes" }, { newTab: true });
    pane.scrollTop = 0; // hidden panes may lose their scroll position
    select(0);
    // Restored after the pane is shown (browsers ignore scrollTop on hidden elements).
    await Promise.resolve();
    expect(pane.scrollTop).toBe(420);
  });

  it("restores scroll on Back once the page's content has loaded", async () => {
    // A stand-in pane: scrollTop is clamped to the content height, like a real browser, and the
    // content only grows once its "data" arrives.
    const observers: Array<() => void> = [];
    vi.stubGlobal(
      "ResizeObserver",
      class {
        constructor(private cb: () => void) {
          observers.push(() => this.cb());
        }
        observe() {}
        disconnect() {}
        unobserve() {}
      },
    );
    let maxScroll = 2000;
    render(() => <TabPanes render={(route) => <Notes label={label(route)} />} />);
    const pane = screen.getByLabelText("Notes home").closest(".tab-pane") as HTMLElement;
    let top = 0;
    Object.defineProperty(pane, "scrollTop", {
      get: () => top,
      set: (v: number) => {
        top = Math.max(0, Math.min(v, maxScroll));
        pane.dispatchEvent(new Event("scroll"));
      },
    });
    pane.scrollTop = 900;
    navigate({ kind: "scenes" }); // same tab, next page
    maxScroll = 0; // new page: short
    pane.scrollTop = 0;
    back();
    maxScroll = 100; // Back: the old page starts short ("Loading…")
    await Promise.resolve();
    expect(pane.scrollTop).toBe(100); // clamped for now…
    maxScroll = 2000; // …then its content arrives
    observers.forEach((fire) => fire());
    expect(pane.scrollTop).toBe(900);
    vi.unstubAllGlobals();
  });

  it(`mounts at most ${MAX_MOUNTED_TABS} tabs and restores state when a tab comes back`, () => {
    render(() => <TabPanes render={(route) => <Notes label={label(route)} />} />);
    fireEvent.input(screen.getByLabelText("Notes home"), { target: { value: "kept" } });
    for (let i = 0; i < MAX_MOUNTED_TABS; i += 1) {
      navigate({ kind: "scene", sceneId: String(i), title: "" }, { newTab: true });
    }
    expect(tabs()).toHaveLength(MAX_MOUNTED_TABS + 1);
    expect(document.querySelectorAll(".tab-pane")).toHaveLength(MAX_MOUNTED_TABS);
    // Home was least recently used, so it was unmounted.
    expect(screen.queryByLabelText("Notes home")).not.toBeInTheDocument();
    select(0);
    expect(screen.getByLabelText("Notes home")).toHaveValue("kept");
    expect(mounts.filter((m) => m === "home")).toHaveLength(2);
  });
});
