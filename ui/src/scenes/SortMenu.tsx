import { createResource, For, Show } from "solid-js";
import { commands } from "../bindings";
import type { SceneQuery, SceneSort } from "../bindings";

/** A new random seed (32-bit; Stash accepts any u64). */
export function newSeed(): number {
  const values = new Uint32Array(1);
  crypto.getRandomValues(values);
  return values[0];
}

/**
 * Every web UI sort, a direction toggle, and Reshuffle for Random (005 FR-003). A random order
 * keeps its seed until Reshuffle, so it stays the same while scrolling and across back/forward.
 */
export default function SortMenu(props: {
  query: SceneQuery;
  onChange: (query: SceneQuery) => void;
}) {
  const [sorts] = createResource(() => commands.sceneSorts());
  const descending = () => props.query.direction === "desc";

  function pick(sort: SceneSort) {
    props.onChange({ ...props.query, sort, seed: sort === "random" ? newSeed() : null });
  }

  return (
    <>
      <label class="visually-hidden" for="scenes-sort">
        Sort by
      </label>
      <select
        id="scenes-sort"
        value={props.query.sort}
        onChange={(e) => pick(e.currentTarget.value as SceneSort)}
      >
        <For each={sorts() ?? []}>{(s) => <option value={s.value}>{s.label}</option>}</For>
      </select>
      <button
        type="button"
        aria-label={
          descending() ? "Descending (change to ascending)" : "Ascending (change to descending)"
        }
        title={descending() ? "Descending" : "Ascending"}
        onClick={() => props.onChange({ ...props.query, direction: descending() ? "asc" : "desc" })}
      >
        {descending() ? "↓" : "↑"}
      </button>
      <Show when={props.query.sort === "random"}>
        <button type="button" onClick={() => props.onChange({ ...props.query, seed: newSeed() })}>
          Reshuffle
        </button>
      </Show>
    </>
  );
}
