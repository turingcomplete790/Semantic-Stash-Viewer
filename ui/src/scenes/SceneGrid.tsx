import { createEffect, createSignal, For, Index, Match, on, Switch } from "solid-js";
import type { Accessor } from "solid-js";
import type { SceneCard as Card } from "../bindings";
import { nextIndex } from "./keyboard";
import type { PageDirection } from "./keyboard";
import SceneCard from "./SceneCard";
import "./scenes.css";

export type GridMode = "grid" | "list";
export type PageLoadState = "loading" | "ready" | "unreachable";
/** Focus a card once the page is shown: an index, or the last card (after moving back a page). */
export type FocusRequest = { which: number | "last"; seq: number };

/**
 * One page of scenes as a plain CSS grid (or list): no virtualization, because a page renders
 * and scrolls within budget at every size (005 research R3, measured in WebKitGTK). Roving focus
 * moves across the page; past its ends the parent changes page (`onEdge`).
 */
export default function SceneGrid(props: {
  cards: Card[];
  state: PageLoadState;
  /** Placeholder cards to show while loading (the page size, or what's left on the last page). */
  placeholders: number;
  mode: GridMode;
  onOpen: (card: Card, newTab: boolean) => void;
  onEdge: (direction: PageDirection) => void;
  onPageKey: (direction: PageDirection) => void;
  /** Columns per row; read from the rendered grid when not given (tests pass one). */
  columns?: () => number;
  /** A card to focus once the page is ready (keyboard page changes); a new `seq` repeats it. */
  focus?: Accessor<FocusRequest | undefined>;
}) {
  let el!: HTMLDivElement;
  const [focused, setFocused] = createSignal(0);
  const lastIndex = () => Math.max(0, props.cards.length - 1);
  const focusIndex = () => {
    const f = focused();
    return Math.min(f, lastIndex());
  };

  /** Columns in the rendered grid: how many cards share the first card's row. */
  function columns(): number {
    if (props.columns) return props.columns();
    if (props.mode === "list") return 1;
    const items = el.querySelectorAll<HTMLElement>(".scene-card");
    if (items.length === 0) return 1;
    const top = items[0].offsetTop;
    let n = 0;
    for (const item of items) {
      if (item.offsetTop !== top) break;
      n += 1;
    }
    return Math.max(1, n);
  }

  function focusCard(index: number) {
    setFocused(index);
    queueMicrotask(() => el.querySelector<HTMLElement>(`[data-index="${index}"]`)?.focus());
  }

  createEffect(
    on([() => props.focus?.(), () => props.state, () => props.cards], ([request, state]) => {
      if (!request || state !== "ready" || props.cards.length === 0) return;
      focusCard(request.which === "last" ? lastIndex() : Math.min(request.which, lastIndex()));
    }),
  );

  function onKeyDown(e: KeyboardEvent) {
    const target = (e.target as HTMLElement).closest<HTMLElement>("[data-index]");
    if (!target) return;
    const action = nextIndex(e.key, {
      index: Number(target.dataset.index),
      count: props.cards.length,
      cols: columns(),
      ctrl: e.ctrlKey || e.metaKey,
    });
    if (!action) return;
    e.preventDefault();
    if ("open" in action) {
      const card = props.cards[action.open];
      if (card) props.onOpen(card, action.newTab);
    } else if ("edge" in action) {
      props.onEdge(action.edge);
    } else if ("page" in action) {
      props.onPageKey(action.page);
    } else {
      focusCard(action.move);
    }
  }

  return (
    <div
      ref={el}
      class="scene-grid"
      classList={{ list: props.mode === "list" }}
      onKeyDown={onKeyDown}
    >
      <Switch>
        <Match when={props.state === "ready"}>
          <For each={props.cards}>
            {(card, i) => (
              <SceneCard
                index={i()}
                card={card}
                state="ready"
                focusable={i() === focusIndex()}
                onOpen={props.onOpen}
                onFocusIndex={setFocused}
              />
            )}
          </For>
        </Match>
        <Match when={props.state === "unreachable"}>
          <p class="scene-grid-message" role="status">
            Can't reach the server
          </p>
        </Match>
        <Match when={props.state === "loading"}>
          <Index each={Array.from({ length: props.placeholders })}>
            {(_, i) => (
              <SceneCard
                index={i}
                card={undefined}
                state="loading"
                focusable={false}
                onOpen={props.onOpen}
                onFocusIndex={setFocused}
              />
            )}
          </Index>
        </Match>
      </Switch>
    </div>
  );
}
