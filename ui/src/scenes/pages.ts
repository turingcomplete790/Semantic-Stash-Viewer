import { batch, createEffect, createSignal, on, onCleanup, untrack } from "solid-js";
import type { Accessor } from "solid-js";
import { commands, events } from "../bindings";
import type { SceneCard, SceneQuery } from "../bindings";
import { connection } from "../state/connection";

/**
 * The page of scenes a view is showing (005 research R4, R7): one request per page, from 003's
 * cache when present. Once the page has arrived, its neighbours are requested too (so the core
 * caches them) and the next page's thumbnails are warmed, so moving there is instant. The page is
 * re-read when a refresh changes it, and retried when the connection comes back.
 */

export type PageState = "loading" | "ready" | "unreachable";

/** Thumbnails warmed per neighbouring page. */
const WARM_LIMIT = 120;

export type ScenePage = {
  state: Accessor<PageState>;
  cards: Accessor<SceneCard[]>;
  /** Total matching scenes; null until a page has arrived. */
  count: Accessor<number | null>;
  /** The page actually shown (the last page when the requested one was past the end). */
  shown: Accessor<number>;
  /** The page that was asked for when `shown` arrived. */
  requested: Accessor<number>;
};

/** Load an image in the background so it's in the webview's memory cache when shown. */
export function warmImage(src: string): void {
  const img = new Image();
  img.decoding = "async";
  img.src = src;
}

export function createScenePage(
  query: Accessor<SceneQuery>,
  page: Accessor<number>,
  pageSize: Accessor<number>,
): ScenePage {
  const [state, setState] = createSignal<PageState>("loading");
  const [cards, setCards] = createSignal<SceneCard[]>([]);
  const [count, setCount] = createSignal<number | null>(null);
  const [shown, setShown] = createSignal(1);
  const [requested, setRequested] = createSignal(1);
  let generation = 0;

  async function fetchPage(p: number) {
    const res = await commands.scenesPage(untrack(query), p, untrack(pageSize));
    return res.status === "ok" ? res.data.data : null;
  }

  async function load(keepCards: boolean) {
    const gen = ++generation;
    const p = untrack(page);
    if (!keepCards) setState("loading");
    let data;
    try {
      data = await fetchPage(p);
    } catch {
      data = null;
    }
    if (gen !== generation) return;
    if (!data) {
      // Still connecting (a cold start with restored tabs): keep the page loading; it's retried
      // as soon as the session connects. Only a session that's up, or has given up, makes the
      // page unreachable.
      const kind = untrack(() => connection.snapshot().state.kind);
      if (!keepCards)
        setState(kind === "connecting" || kind === "idle" ? "loading" : "unreachable");
      return;
    }
    batch(() => {
      setCount(data.count);
      setShown(data.page);
      setRequested(p);
      setCards(data.items);
      setState("ready");
    });
    prefetchNeighbours(gen, data.page, data.count);
  }

  function prefetchNeighbours(gen: number, p: number, total: number) {
    const last = Math.max(1, Math.ceil(total / untrack(pageSize)));
    const neighbours = [p + 1, p - 1].filter((n) => n >= 1 && n <= last);
    for (const n of neighbours) {
      void fetchPage(n)
        .then((data) => {
          // Dropped if the user has moved on; warm the next page's thumbnails only.
          if (!data || gen !== generation || n !== p + 1) return;
          for (const card of data.items.slice(0, WARM_LIMIT)) {
            if (card.thumb) warmImage(card.thumb);
          }
        })
        .catch(() => {});
    }
  }

  // A new query, page, or size: load it.
  createEffect(
    on([() => JSON.stringify(query()), page, pageSize], () => {
      void load(false);
    }),
  );

  // A refresh changed this page (or the cache was cleared): re-read it in place.
  let unlisten: (() => void) | undefined;
  let disposed = false;
  void events.viewDataChanged
    .listen((e) =>
      untrack(() => {
        const key = e.payload.key;
        const match = /^scenes:q:[0-9a-f]+:s:(\d+):p:(\d+)$/.exec(key);
        const here =
          match !== null && Number(match[1]) === pageSize() && Number(match[2]) === shown();
        if (key === "*" || here) void load(true);
      }),
    )
    .then((stop) => {
      if (disposed) stop();
      else unlisten = stop;
    });
  onCleanup(() => {
    disposed = true;
    generation += 1;
    unlisten?.();
  });

  // Connected (after a cold start, or back online): load a page that hasn't loaded yet. Once
  // per connection, never in a loop.
  createEffect(
    on(
      () => connection.snapshot().state.kind,
      (kind) => {
        if (kind === "connected" && untrack(state) !== "ready") void load(false);
      },
      { defer: true },
    ),
  );

  return { state, cards, count, shown, requested };
}
