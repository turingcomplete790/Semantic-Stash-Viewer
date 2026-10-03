import { Show } from "solid-js";
import type { SceneCard as Card } from "../bindings";
import { formatDuration } from "../player/format";

export type CardState = "loading" | "ready" | "unreachable";

/**
 * One card in a page of scenes, or a row in the list (005 FR-001). Text stays on one line each for
 * the title and the details. Every thumbnail on the page loads at once, never lazily: in WebKitGTK
 * each thumbnail that arrives mid-scroll costs frames (005 T024, research R3).
 */
export default function SceneCard(props: {
  index: number;
  card: Card | undefined;
  state: CardState;
  focusable: boolean;
  onOpen: (card: Card, newTab: boolean) => void;
  onFocusIndex: (index: number) => void;
}) {
  const details = () => {
    const c = props.card;
    if (!c) return "";
    return [
      c.durationSeconds != null ? formatDuration(c.durationSeconds) : null,
      c.resolution,
      c.studio,
      c.date,
    ]
      .filter(Boolean)
      .join(" · ");
  };

  return (
    <Show
      when={props.card}
      fallback={
        <div
          class="scene-card"
          classList={{
            placeholder: props.state !== "unreachable",
            unreachable: props.state === "unreachable",
          }}
          aria-hidden="true"
        >
          <div class="scene-card-thumb" />
          <div class="scene-card-title">{" "}</div>
          <div class="scene-card-details">{" "}</div>
        </div>
      }
    >
      {(card) => (
        <button
          type="button"
          class="scene-card"
          data-index={props.index}
          tabIndex={props.focusable ? 0 : -1}
          aria-label={card().title}
          onClick={(e) => props.onOpen(card(), e.ctrlKey || e.metaKey)}
          onAuxClick={(e) => {
            if (e.button === 1) {
              e.preventDefault();
              props.onOpen(card(), true);
            }
          }}
          onFocus={() => props.onFocusIndex(props.index)}
        >
          <div class="scene-card-thumb">
            <Show when={card().thumb}>
              {(thumb) => <img src={thumb()} alt="" decoding="async" draggable={false} />}
            </Show>
          </div>
          <div class="scene-card-title" title={card().title}>
            {card().title}
          </div>
          <div class="scene-card-details">{details()}</div>
        </button>
      )}
    </Show>
  );
}
