import {
  batch,
  createEffect,
  createSignal,
  Match,
  onCleanup,
  Show,
  Switch,
  untrack,
} from "solid-js";
import type { SceneQuery } from "../bindings";
import { DEFAULT_PAGE_SIZE, PAGE_SIZES } from "../scenes/PageControls";
import PageControls from "../scenes/PageControls";
import { createScenePage } from "../scenes/pages";
import SceneGrid from "../scenes/SceneGrid";
import type { FocusRequest, GridMode } from "../scenes/SceneGrid";
import type { PageDirection } from "../scenes/keyboard";
import SortMenu from "../scenes/SortMenu";
import { registerRawHandler } from "../shell/keymap";
import { navigate, setViewState } from "../shell/tabs";
import { useTab, useViewState } from "../shell/viewState";
import "../scenes/scenes.css";

/** Save the scroll position this long after scrolling stops. */
const SCROLL_SAVE_DELAY_MS = 200;

const DEFAULT_QUERY: SceneQuery = { search: "", sort: "date", direction: "desc", seed: null };

/**
 * Scenes: the whole library, a page at a time (005 US1; constitution IV). Each history entry keeps
 * its query, page, page size, display mode, and scroll position (research R6), and the shell
 * returns to that entry when the user comes back (R13), so leaving and returning never resets it.
 */
export default function ScenesView() {
  const [savedQuery] = useViewState<SceneQuery>("query", DEFAULT_QUERY);
  const [savedPage] = useViewState<number>("page", 1);
  const [savedSize] = useViewState<number>("pageSize", DEFAULT_PAGE_SIZE);
  const [savedMode] = useViewState<GridMode>("mode", "grid");
  const [savedScroll, saveScroll] = useViewState<number>("scroll", 0);

  // Read once on mount; afterwards local signals drive the view and write back on change.
  const [query, setQuery] = createSignal<SceneQuery>({ ...DEFAULT_QUERY, ...savedQuery() });
  const [page, setPage] = createSignal(Math.max(1, savedPage()));
  const [pageSize, setPageSize] = createSignal(
    (PAGE_SIZES as readonly number[]).includes(savedSize()) ? savedSize() : DEFAULT_PAGE_SIZE,
  );
  const [mode, setMode] = createSignal<GridMode>(savedMode());
  const [focus, setFocus] = createSignal<FocusRequest | undefined>();
  let pendingScroll: number | null = savedScroll() || null;
  let scroller!: HTMLDivElement;

  const { tabId } = useTab();

  /**
   * Record the view's state in this tab's history entry. Only from event handlers and effects,
   * never while rendering: a write to the tab store during rendering makes the tab panes rebuild
   * themselves inside their own rebuild (on a restart, recursively until the stack overflows).
   */
  function persist() {
    if (!tabId) return;
    setViewState(tabId, {
      query: query(),
      page: page(),
      pageSize: pageSize(),
      mode: mode(),
    });
  }

  const scenes = createScenePage(query, page, pageSize);
  const pages = () => Math.max(1, Math.ceil((scenes.count() ?? 0) / pageSize()));

  // Restore the scroll position once the restored page has rendered.
  createEffect(() => {
    if (scenes.state() !== "ready" || pendingScroll === null) return;
    const top = pendingScroll;
    pendingScroll = null;
    queueMicrotask(() => {
      scroller.scrollTop = top;
    });
  });

  // A page past the end came back as the last page: show that number. Only for the answer to the
  // page currently asked for (older answers may still be on screen while a new page loads).
  createEffect(() => {
    if (scenes.state() !== "ready" || scenes.requested() !== page()) return;
    if (scenes.shown() !== page()) {
      setPage(scenes.shown());
      persist();
    }
  });

  // Save the scroll position once scrolling pauses: it's only needed when the user leaves and
  // comes back, and saving replaces the tab's state, which isn't free (005 harness: per-frame
  // saves cost most of the frame budget while scrolling).
  let scrollTimer: ReturnType<typeof setTimeout> | undefined;
  function onScroll() {
    clearTimeout(scrollTimer);
    scrollTimer = setTimeout(() => saveScroll(scroller.scrollTop), SCROLL_SAVE_DELAY_MS);
  }
  onCleanup(() => clearTimeout(scrollTimer));

  function toTop() {
    clearTimeout(scrollTimer);
    pendingScroll = null;
    scroller.scrollTop = 0;
    saveScroll(0);
  }

  function goTo(next: number, focusWhich?: number | "last") {
    const clamped = Math.min(Math.max(1, next), pages());
    if (clamped === page()) return;
    setPage(clamped);
    persist();
    toTop();
    if (focusWhich !== undefined) requestFocus(focusWhich);
  }

  function step(direction: PageDirection) {
    if (direction === "next") goTo(page() + 1, 0);
    else goTo(page() - 1, "last");
  }

  function changeQuery(next: SceneQuery) {
    // One update, so only the new query's page 1 is requested.
    batch(() => {
      setQuery(next);
      setPage(1);
    });
    persist();
    toTop();
  }

  /** The first scene in view, as a position in the whole result set. */
  function firstVisibleIndex(): number {
    const top = scroller.scrollTop;
    const cards = scroller.querySelectorAll<HTMLElement>(".scene-card[data-index]");
    let local = 0;
    for (const card of cards) {
      if (card.offsetTop + card.offsetHeight > top) {
        local = Number(card.dataset.index);
        break;
      }
    }
    return (scenes.shown() - 1) * pageSize() + local;
  }

  function changePageSize(size: number) {
    const target = Math.floor(firstVisibleIndex() / size) + 1;
    batch(() => {
      setPageSize(size);
      setPage(target);
    });
    persist();
    toTop();
  }

  function requestFocus(which: number | "last") {
    setFocus({ which, seq: (focus()?.seq ?? 0) + 1 });
  }

  // The page keys work whenever this Scenes tab is showing, not only while a card has focus (the
  // grid handles them itself when one does). Nothing fires while typing in a field: the keymap
  // skips its handlers then.
  const tab = useTab();
  onCleanup(
    registerRawHandler("shell", (e) =>
      untrack(() => {
        if (!tab.isActive() || e.defaultPrevented) return false;
        switch (e.key) {
          case "]":
            step("next");
            break;
          case "[":
            step("previous");
            break;
          case "Home":
            requestFocus(0);
            break;
          case "End":
            requestFocus("last");
            break;
          case "ArrowUp":
          case "ArrowDown":
          case "ArrowLeft":
          case "ArrowRight":
            requestFocus(0);
            break;
          default:
            return false;
        }
        e.preventDefault();
        return true;
      }),
    ),
  );

  function changeMode(next: GridMode) {
    setMode(next);
    persist();
  }

  const controls = () => (
    <PageControls
      page={page()}
      pageSize={pageSize()}
      count={scenes.count() ?? 0}
      onPage={(p) => goTo(p)}
      onPageSize={changePageSize}
    />
  );

  return (
    <div class="scenes-view">
      <div class="scenes-toolbar">
        <SortMenu query={query()} onChange={changeQuery} />
        <div class="scenes-mode" role="group" aria-label="Display">
          <button type="button" aria-pressed={mode() === "grid"} onClick={() => changeMode("grid")}>
            Grid
          </button>
          <button type="button" aria-pressed={mode() === "list"} onClick={() => changeMode("list")}>
            List
          </button>
        </div>
        <span class="spacer" />
        <Show when={(scenes.count() ?? 0) > 0}>{controls()}</Show>
      </div>
      <div ref={scroller} class="scenes-scroller" onScroll={onScroll}>
        <Switch>
          <Match when={scenes.state() === "ready" && scenes.count() === 0}>
            <p class="scenes-empty">No scenes in this library yet</p>
          </Match>
          <Match when={true}>
            <SceneGrid
              cards={scenes.cards()}
              state={scenes.state()}
              placeholders={pageSize()}
              mode={mode()}
              focus={focus}
              onOpen={(card, newTab) =>
                navigate({ kind: "scene", sceneId: card.id, title: card.title }, { newTab })
              }
              onEdge={step}
              onPageKey={step}
            />
            <Show when={(scenes.count() ?? 0) > 0}>
              <div class="scenes-bottom">{controls()}</div>
            </Show>
          </Match>
        </Switch>
      </div>
    </div>
  );
}
