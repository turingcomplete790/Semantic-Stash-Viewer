import { createSignal, For } from "solid-js";
import { render } from "solid-js/web";

/**
 * Debug-only scroll test for the performance harness (003 research R8): a windowed list of
 * 10,000 rows in a full-window overlay, scrolled programmatically. It is mounted only while the
 * UI bench runs; users never see it (FR-018).
 */

const ROWS = 10_000;
const ROW_HEIGHT = 32;
const OVERSCAN = 10;
/** Pixels per frame: fast enough to bring new rows in on every frame. */
const STEP = 48;

function ScrollList(props: { ref: (el: HTMLDivElement) => void }) {
  const [top, setTop] = createSignal(0);
  const first = () => Math.max(0, Math.floor(top() / ROW_HEIGHT) - OVERSCAN);
  const count = () => Math.ceil(window.innerHeight / ROW_HEIGHT) + 2 * OVERSCAN;
  const rows = () => {
    const start = first();
    return Array.from({ length: Math.min(count(), ROWS - start) }, (_, i) => start + i);
  };

  return (
    <div
      ref={props.ref}
      onScroll={(e) => setTop(e.currentTarget.scrollTop)}
      style={{
        position: "fixed",
        inset: "0",
        "z-index": "10000",
        "overflow-y": "auto",
        background: "var(--bg, #111)",
        color: "var(--text, #eee)",
      }}
    >
      <div style={{ position: "relative", height: `${ROWS * ROW_HEIGHT}px` }}>
        <For each={rows()}>
          {(i) => (
            <div
              style={{
                position: "absolute",
                left: "0",
                right: "0",
                top: `${i * ROW_HEIGHT}px`,
                height: `${ROW_HEIGHT}px`,
                display: "flex",
                gap: "12px",
                padding: "0 16px",
                "align-items": "center",
                "border-bottom": "1px solid rgba(128, 128, 128, 0.2)",
              }}
            >
              <span style={{ flex: "1" }}>Scene {i + 1}</span>
              <span>1920×1080 · h264 · mp4</span>
              <span>{`${Math.floor(i / 60) % 60}:${String(i % 60).padStart(2, "0")}`}</span>
            </div>
          )}
        </For>
      </div>
    </div>
  );
}

/** Scroll for `durationMs` and return every animation frame's interval in ms. */
export async function runScrollBench(durationMs = 5000): Promise<number[]> {
  const host = document.createElement("div");
  document.body.appendChild(host);
  let list: HTMLDivElement | undefined;
  const dispose = render(() => <ScrollList ref={(el) => (list = el)} />, host);
  try {
    // Settle before timing.
    await new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(r)));
    const el = list;
    if (!el) return [];
    const intervals: number[] = [];
    await new Promise<void>((resolve) => {
      let last = performance.now();
      const end = last + durationMs;
      const step = (now: number) => {
        intervals.push(now - last);
        last = now;
        const max = el.scrollHeight - el.clientHeight;
        el.scrollTop = el.scrollTop + STEP >= max ? 0 : el.scrollTop + STEP;
        if (now < end) requestAnimationFrame(step);
        else resolve();
      };
      requestAnimationFrame(step);
    });
    // The first interval includes the wait for the first frame.
    return intervals.slice(1);
  } finally {
    dispose();
    host.remove();
  }
}
