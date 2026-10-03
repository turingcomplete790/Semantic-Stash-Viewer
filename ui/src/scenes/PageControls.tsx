import { createSignal, For } from "solid-js";

/** Stash's page sizes, plus our default of 50 (constitution IV; spec clarification). */
export const PAGE_SIZES = [20, 40, 50, 60, 120, 250, 500, 1000] as const;
export const DEFAULT_PAGE_SIZE = 50;

const numbers = new Intl.NumberFormat("en-US");

/**
 * Where the page is, and how to move (005 FR-004): "Page 25 of 727 · 1,201–1,250 of 36,350",
 * first/previous/next/last, go to a page, and the page size.
 */
export default function PageControls(props: {
  page: number;
  pageSize: number;
  count: number;
  onPage: (page: number) => void;
  onPageSize: (size: number) => void;
}) {
  const [target, setTarget] = createSignal("");
  const pages = () => Math.max(1, Math.ceil(props.count / props.pageSize));
  const first = () => (props.page - 1) * props.pageSize + 1;
  const last = () => Math.min(props.count, props.page * props.pageSize);

  return (
    <div class="page-controls">
      <button
        type="button"
        aria-label="First page"
        title="First page"
        disabled={props.page <= 1}
        onClick={() => props.onPage(1)}
      >
        «
      </button>
      <button
        type="button"
        aria-label="Previous page"
        title="Previous page ([)"
        disabled={props.page <= 1}
        onClick={() => props.onPage(props.page - 1)}
      >
        ‹
      </button>
      <span class="page-position" data-testid="page-position">
        Page {numbers.format(props.page)} of {numbers.format(pages())} · {numbers.format(first())}–
        {numbers.format(last())} of {numbers.format(props.count)}
      </span>
      <button
        type="button"
        aria-label="Next page"
        title="Next page (])"
        disabled={props.page >= pages()}
        onClick={() => props.onPage(props.page + 1)}
      >
        ›
      </button>
      <button
        type="button"
        aria-label="Last page"
        title="Last page"
        disabled={props.page >= pages()}
        onClick={() => props.onPage(pages())}
      >
        »
      </button>
      <form
        onSubmit={(e) => {
          e.preventDefault();
          const n = Number.parseInt(target(), 10);
          if (Number.isFinite(n) && n >= 1) props.onPage(Math.min(n, pages()));
          setTarget("");
        }}
      >
        <input
          type="number"
          min="1"
          max={pages()}
          aria-label="Go to page"
          placeholder="Page…"
          value={target()}
          onInput={(e) => setTarget(e.currentTarget.value)}
        />
      </form>
      <select
        aria-label="Per page"
        value={String(props.pageSize)}
        onChange={(e) => props.onPageSize(Number(e.currentTarget.value))}
      >
        <For each={PAGE_SIZES}>{(size) => <option value={String(size)}>{size}</option>}</For>
      </select>
    </div>
  );
}
