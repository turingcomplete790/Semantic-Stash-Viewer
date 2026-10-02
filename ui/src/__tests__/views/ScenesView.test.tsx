import { cleanup, fireEvent, render, screen, waitFor } from "@solidjs/testing-library";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { SceneCard, SceneQuery } from "../../bindings";

const mocks = vi.hoisted(() => {
  const viewData = new Set<(e: { payload: unknown }) => void>();
  let connectionHandler: ((e: { payload: unknown }) => void) | undefined;
  const listen = () => vi.fn(() => Promise.resolve(() => {}));
  return {
    changed: (key: string) => viewData.forEach((h) => h({ payload: { profileId: "p1", key } })),
    connection: (payload: unknown) => connectionHandler?.({ payload }),
    commands: { scenesPage: vi.fn(), sceneSorts: vi.fn(), getConnectionSnapshot: vi.fn() },
    events: {
      viewDataChanged: {
        listen: vi.fn((h: (e: { payload: unknown }) => void) => {
          viewData.add(h);
          return Promise.resolve(() => viewData.delete(h));
        }),
      },
      connectionState: {
        listen: vi.fn((h: (e: { payload: unknown }) => void) => {
          connectionHandler = h;
          return Promise.resolve(() => {});
        }),
      },
      profilesChanged: { listen: listen() },
    },
  };
});
vi.mock("../../bindings", () => ({ commands: mocks.commands, events: mocks.events }));

import { initConnection } from "../../state/connection";
import { dispatch, resetKeymap } from "../../shell/keymap";
import { resetTabs, selectedId, setViewState, tabs, viewStateOf } from "../../shell/tabs";
import { TabContext } from "../../shell/viewState";
import ScenesView from "../../views/ScenesView";

let total = 36_350;
let failing = false;
const warmed: string[] = [];

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

function answer(_query: SceneQuery, page: number, pageSize: number) {
  if (failing) return Promise.resolve({ status: "error", error: { kind: "notConnected" } });
  const last = Math.max(1, Math.ceil(total / pageSize));
  const p = Math.min(page, last);
  const start = (p - 1) * pageSize;
  const items = Array.from({ length: Math.max(0, Math.min(pageSize, total - start)) }, (_, k) =>
    card(start + k),
  );
  return Promise.resolve({
    status: "ok",
    data: { data: { count: total, page: p, pageSize, items }, fromCache: false, fetchedAt: "" },
  });
}

const calls = () =>
  mocks.commands.scenesPage.mock.calls.map((c) => ({
    query: c[0] as SceneQuery,
    page: c[1] as number,
    size: c[2] as number,
  }));
const scroller = () => document.querySelector(".scenes-scroller") as HTMLDivElement;
const state = () => viewStateOf(selectedId());

function view() {
  const tabId = selectedId();
  return render(() => (
    <TabContext.Provider value={{ tabId, isActive: () => true }}>
      <ScenesView />
    </TabContext.Provider>
  ));
}

beforeEach(() => {
  vi.clearAllMocks();
  total = 36_350;
  failing = false;
  warmed.length = 0;
  resetTabs();
  vi.stubGlobal(
    "Image",
    class {
      decoding = "";
      set src(v: string) {
        warmed.push(v);
      }
    },
  );
  mocks.commands.scenesPage.mockImplementation(answer);
  mocks.commands.sceneSorts.mockResolvedValue([
    { value: "date", label: "Date" },
    { value: "duration", label: "Duration" },
    { value: "random", label: "Random" },
    { value: "title", label: "Title" },
  ]);
});
afterEach(() => {
  cleanup();
  resetKeymap();
  vi.unstubAllGlobals();
});

/** A key pressed with nothing focused, as the app's keymap sees it. */
function press(key: string) {
  (document.activeElement as HTMLElement | null)?.blur?.();
  dispatch(new KeyboardEvent("keydown", { key, bubbles: true, cancelable: true }));
}

describe("ScenesView (paged)", () => {
  it("shows page 1 of 50 with controls above and below the grid", async () => {
    view();
    expect(await screen.findByRole("button", { name: "Scene 1" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "Scene 50" })).toBeInTheDocument();
    expect(screen.queryByRole("button", { name: "Scene 51" })).not.toBeInTheDocument();
    expect(calls()[0]).toMatchObject({ page: 1, size: 50 });
    expect(calls()[0].query).toMatchObject({ sort: "date", direction: "desc" });
    expect(screen.getAllByRole("button", { name: "Next page" })).toHaveLength(2);
    expect(screen.getAllByTestId("page-position")[0]).toHaveTextContent(
      "Page 1 of 727 · 1–50 of 36,350",
    );
  });

  it("loads the next page and warms its thumbnails ahead", async () => {
    view();
    await screen.findByRole("button", { name: "Scene 1" });
    await waitFor(() => expect(calls().some((c) => c.page === 2)).toBe(true));
    expect(calls().some((c) => c.page === 0)).toBe(false);
    await waitFor(() => expect(warmed).toContain("ssv-thumb://localhost/scene/51?v=1"));
  });

  it("changes page with the buttons and with [ and ], scrolled to the top", async () => {
    view();
    await screen.findByRole("button", { name: "Scene 1" });
    scroller().scrollTop = 300;
    fireEvent.click(screen.getAllByRole("button", { name: "Next page" })[0]);
    expect(await screen.findByRole("button", { name: "Scene 51" })).toBeInTheDocument();
    expect(scroller().scrollTop).toBe(0);
    fireEvent.keyDown(screen.getByRole("button", { name: "Scene 51" }), { key: "]" });
    expect(await screen.findByRole("button", { name: "Scene 101" })).toBeInTheDocument();
    fireEvent.keyDown(screen.getByRole("button", { name: "Scene 101" }), { key: "[" });
    expect(await screen.findByRole("button", { name: "Scene 51" })).toBeInTheDocument();
  });

  it("saves page, mode, size, and scroll, and a view on that entry restores them all", async () => {
    view();
    await screen.findByRole("button", { name: "Scene 1" });
    const go = screen.getAllByLabelText("Go to page")[0];
    fireEvent.input(go, { target: { value: "37" } });
    fireEvent.submit(go.closest("form") as HTMLFormElement);
    await screen.findByRole("button", { name: "Scene 1801" });
    fireEvent.click(screen.getByRole("button", { name: "List" }));
    scroller().scrollTop = 420;
    fireEvent.scroll(scroller());
    await waitFor(() =>
      expect(state()).toMatchObject({ page: 37, pageSize: 50, mode: "list", scroll: 420 }),
    );
    cleanup();
    view();
    expect(await screen.findByRole("button", { name: "Scene 1801" })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: "List" })).toHaveAttribute("aria-pressed", "true");
    await waitFor(() => expect(scroller().scrollTop).toBe(420));
  });

  it("keeps your place when the page size changes", async () => {
    setViewState(selectedId(), { page: 37 });
    view();
    await screen.findByRole("button", { name: "Scene 1801" });
    fireEvent.change(screen.getAllByLabelText("Per page")[0], { target: { value: "120" } });
    // The first scene shown (#1,801) is on page 16 at 120 per page.
    expect(await screen.findByRole("button", { name: "Scene 1801" })).toBeInTheDocument();
    expect(state()).toMatchObject({ page: 16, pageSize: 120 });
    expect(calls().at(-1)?.size).toBe(120);
  });

  it("starts a new sort on page 1, and Random keeps its seed until Reshuffle", async () => {
    setViewState(selectedId(), { page: 5 });
    view();
    await screen.findByRole("button", { name: "Scene 201" });
    fireEvent.change(await screen.findByLabelText("Sort by"), { target: { value: "random" } });
    await waitFor(() => expect(state()).toMatchObject({ page: 1 }));
    const seed = (state().query as SceneQuery).seed;
    expect(typeof seed).toBe("number");
    fireEvent.click(screen.getAllByRole("button", { name: "Next page" })[0]);
    await waitFor(() =>
      expect(calls().some((c) => c.page === 2 && c.query.seed === seed)).toBe(true),
    );
    fireEvent.click(screen.getByRole("button", { name: "Reshuffle" }));
    await waitFor(() => expect((state().query as SceneQuery).seed).not.toBe(seed));
  });

  it("re-reads the page in place when a refresh changes it", async () => {
    setViewState(selectedId(), { page: 37, scroll: 200 });
    view();
    await screen.findByRole("button", { name: "Scene 1801" });
    const before = calls().filter((c) => c.page === 37).length;
    mocks.changed("scenes:q:0123456789abcdef:s:50:p:37");
    await waitFor(() => expect(calls().filter((c) => c.page === 37).length).toBe(before + 1));
    expect(scroller().scrollTop).toBe(200);
  });

  it("says when the library is empty", async () => {
    total = 0;
    view();
    expect(await screen.findByText("No scenes in this library yet")).toBeInTheDocument();
  });

  it("says when the server can't be reached, keeping the controls", async () => {
    mocks.commands.getConnectionSnapshot.mockResolvedValue({
      profileId: "p1",
      state: { kind: "offline", attempt: 2, nextRetryAt: "2026-10-02T12:00:00Z" },
      security: null,
      finalUrl: null,
      server: null,
      lastContactAt: null,
    });
    await initConnection();
    failing = true;
    view();
    expect(await screen.findByText("Can't reach the server")).toBeInTheDocument();
  });

  it("opens a scene in a new tab with Ctrl+click", async () => {
    view();
    const count = tabs().length;
    fireEvent.click(await screen.findByRole("button", { name: "Scene 4" }), { ctrlKey: true });
    expect(tabs().length).toBe(count + 1);
  });

  it("changes page with [ and ] even when no card has focus", async () => {
    view();
    await screen.findByRole("button", { name: "Scene 1" });
    press("]");
    expect(await screen.findByRole("button", { name: "Scene 51" })).toBeInTheDocument();
    press("[");
    expect(await screen.findByRole("button", { name: "Scene 1" })).toBeInTheDocument();
  });

  it("Home, End, and the arrows reach the cards when no card has focus", async () => {
    view();
    await screen.findByRole("button", { name: "Scene 1" });
    press("End");
    await waitFor(() =>
      expect(document.activeElement).toBe(screen.getByRole("button", { name: "Scene 50" })),
    );
    press("Home");
    await waitFor(() =>
      expect(document.activeElement).toBe(screen.getByRole("button", { name: "Scene 1" })),
    );
    press("ArrowDown");
    await waitFor(() =>
      expect(document.activeElement).toBe(screen.getByRole("button", { name: "Scene 1" })),
    );
  });

  it("leaves the keys alone while typing in a field", async () => {
    view();
    await screen.findByRole("button", { name: "Scene 1" });
    const go = screen.getAllByLabelText("Go to page")[0];
    go.focus();
    // As the app's document listener sees it: the key comes from the focused field.
    const event = new KeyboardEvent("keydown", { key: "]", bubbles: true, cancelable: true });
    go.dispatchEvent(event);
    dispatch(event);
    await new Promise((r) => setTimeout(r, 50));
    expect(screen.getByRole("button", { name: "Scene 1" })).toBeInTheDocument();
  });

  it("on a cold start, waits for the connection instead of giving up (restored tabs)", async () => {
    const snapshot = (kind: string) => ({
      profileId: "p1",
      state: { kind },
      security: null,
      finalUrl: null,
      server: null,
      lastContactAt: null,
    });
    mocks.commands.getConnectionSnapshot.mockResolvedValue(snapshot("connecting"));
    await initConnection();
    failing = true; // nothing cached, and the session isn't connected yet
    view();
    await waitFor(() => expect(calls().length).toBeGreaterThan(0));
    // Still connecting: a loading page, not "Can't reach the server".
    expect(screen.queryByText("Can't reach the server")).not.toBeInTheDocument();
    expect(document.querySelectorAll(".scene-card.placeholder").length).toBeGreaterThan(0);
    failing = false;
    mocks.connection(snapshot("connected"));
    expect(await screen.findByRole("button", { name: "Scene 1" })).toBeInTheDocument();
  });
});
