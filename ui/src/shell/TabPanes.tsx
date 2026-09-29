import { createEffect, createMemo, createSignal, For, on, onCleanup, onMount } from "solid-js";
import type { JSX } from "solid-js";
import type { Route } from "./routes";
import {
  isPlayingTab,
  routeOf,
  selectedId,
  setViewState,
  tabById,
  tabs,
  viewStateOf,
} from "./tabs";
import { TabContext } from "./viewState";

/** Most tabs kept mounted; older background tabs are rebuilt from their view state (FR-016). */
export const MAX_MOUNTED_TABS = 8;
/** How long to keep re-applying a restored scroll position while the page's content loads. */
const RESTORE_WINDOW_MS = 3000;

/**
 * Renders tabs with keep-alive (004 research R2): the selected tab is visible, up to 7 recently
 * used tabs stay mounted but `hidden` and `inert` (not painted, not focusable), and older tabs
 * are unmounted. The playing scene's tab is never unmounted. Each pane is its own scroll
 * container and restores its scroll position per history entry.
 */
export default function TabPanes(props: { render: (route: Route, tabId: string) => JSX.Element }) {
  const [recent, setRecent] = createSignal<string[]>([selectedId()]);

  // Most recently used first; drop closed tabs; keep at most MAX_MOUNTED_TABS (+ the playing one).
  createEffect(() => {
    const id = selectedId();
    const alive = new Set(tabs().map((t) => t.id));
    setRecent((list) => {
      const ordered = [id, ...list.filter((x) => x !== id && alive.has(x))];
      const kept = ordered.slice(0, MAX_MOUNTED_TABS);
      const playing = tabs().find((t) => isPlayingTab(t));
      if (playing && !kept.includes(playing.id)) kept[kept.length - 1] = playing.id;
      return kept;
    });
  });

  const mounted = () =>
    tabs()
      .filter((t) => recent().includes(t.id))
      .map((t) => t.id);

  return <For each={mounted()}>{(id) => <Pane id={id} render={props.render} />}</For>;
}

function Pane(props: { id: string; render: (route: Route, tabId: string) => JSX.Element }) {
  let el: HTMLDivElement | undefined;
  const active = () => selectedId() === props.id;
  const tab = () => tabById(props.id);
  // Only a real navigation changes the route object; view-state updates must not re-render the
  // view (they replace the tab object on every keystroke or scroll).
  const route = createMemo(() => {
    const t = tab();
    return t ? routeOf(t) : ({ kind: "home" } as Route);
  });
  const entryKey = () => `${props.id}:${tab()?.index ?? 0}`;

  let content: HTMLDivElement | undefined;
  // A restored position waiting for the page to be tall enough. Pages often render short first
  // ("Loading…"), which clamps scrollTop; re-apply as the content grows, and don't save the
  // clamped values meanwhile. Stops when reached, when the user scrolls, or after 3 s.
  let pending: number | null = null;
  let pendingTimer: ReturnType<typeof setTimeout> | undefined;
  const applyPending = () => {
    if (!el || pending === null) return;
    const target = pending;
    el.scrollTop = target;
    if (el.scrollTop >= target - 1) pending = null;
  };
  const cancelPending = () => {
    pending = null;
    clearTimeout(pendingTimer);
  };

  const saveScroll = () => {
    if (!el) return;
    setViewState(props.id, { scroll: el.scrollTop });
  };
  const restoreScroll = () => {
    if (!el) return;
    const top = viewStateOf(props.id).scroll;
    pending = typeof top === "number" ? top : 0;
    clearTimeout(pendingTimer);
    pendingTimer = setTimeout(cancelPending, RESTORE_WINDOW_MS);
    applyPending();
  };

  onMount(() => {
    queueMicrotask(restoreScroll);
    const observer = new ResizeObserver(applyPending);
    if (content) observer.observe(content);
    onCleanup(() => {
      observer.disconnect();
      clearTimeout(pendingTimer);
    });
  });

  // Restore when the tab is shown again or moves to another history entry.
  createEffect(
    on([active, entryKey], ([isActive]) => {
      if (isActive) queueMicrotask(restoreScroll);
    }),
  );

  return (
    <div
      ref={el}
      class="tab-pane"
      hidden={!active()}
      inert={!active()}
      onScroll={() => {
        if (active() && pending === null) saveScroll();
      }}
      onWheel={cancelPending}
      onPointerDown={cancelPending}
      onKeyDown={cancelPending}
    >
      <div ref={content} class="tab-pane-content">
        <TabContext.Provider value={{ tabId: props.id, isActive: active }}>
          {props.render(route(), props.id)}
        </TabContext.Provider>
      </div>
    </div>
  );
}
