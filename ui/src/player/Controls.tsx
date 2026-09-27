import { createEffect, createSignal, For, onCleanup, onMount, Show } from "solid-js";
import { commands } from "../bindings";
import { formatDuration } from "./format";
import { SPEEDS } from "./speeds";
import { player, seekBy, seekTo } from "./state";

export { SPEEDS };

/** How long the pointer must be still before controls and cursor hide (FR-010). */
const IDLE_MS = 3000;

/**
 * Playback controls drawn by the viewer on top of the GPU-rendered video (US2, FR-008).
 * Solid backgrounds, no backdrop blur, to keep compositing over the video cheap.
 */
export default function Controls() {
  const s = () => player.snapshot();
  const duration = () => s().durationSeconds ?? 0;

  // While dragging, show the drag position instead of the (lagging) playback position.
  const [dragPosition, setDragPosition] = createSignal<number | null>(null);
  const position = () => dragPosition() ?? player.displayPosition();

  const [hoverTime, setHoverTime] = createSignal<number | null>(null);

  // Auto-hide: only while actually playing (FR-010).
  const [idle, setIdle] = createSignal(false);
  let timer: ReturnType<typeof setTimeout> | undefined;
  const playing = () => s().state === "playing";
  const wake = () => {
    setIdle(false);
    clearTimeout(timer);
    if (playing()) timer = setTimeout(() => setIdle(true), IDLE_MS);
  };
  createEffect(() => {
    if (playing()) wake();
    else {
      clearTimeout(timer);
      setIdle(false);
    }
  });
  onMount(() => {
    document.addEventListener("mousemove", wake);
    document.addEventListener("keydown", wake);
  });
  onCleanup(() => {
    clearTimeout(timer);
    document.removeEventListener("mousemove", wake);
    document.removeEventListener("keydown", wake);
  });

  function onSeekHover(e: MouseEvent & { currentTarget: HTMLInputElement }) {
    const rect = e.currentTarget.getBoundingClientRect();
    if (rect.width <= 0 || duration() <= 0) return;
    const fraction = Math.min(1, Math.max(0, (e.clientX - rect.left) / rect.width));
    setHoverTime(fraction * duration());
  }

  return (
    <div class="player-controls-root" classList={{ idle: idle() }}>
      <Show when={s().state === "ended"}>
        <div class="player-center">
          <button type="button" class="player-replay" onClick={() => void commands.playerReplay()}>
            Replay
          </button>
        </div>
      </Show>

      <div class="player-controls" role="toolbar" aria-label="Playback controls">
        <div class="player-seek">
          <span class="player-time">{formatDuration(position())}</span>
          <div class="player-seek-track">
            <input
              type="range"
              aria-label="Seek"
              min={0}
              max={Math.max(duration(), 0.001)}
              step={0.1}
              value={position()}
              onInput={(e) => {
                const value = Number(e.currentTarget.value);
                setDragPosition(value);
                seekTo(value, false);
              }}
              onChange={(e) => {
                const value = Number(e.currentTarget.value);
                setDragPosition(null);
                // The bar keeps showing `value` until mpv gets there (slow on e.g. WMV).
                seekTo(value, true);
              }}
              onMouseMove={onSeekHover}
              onMouseLeave={() => setHoverTime(null)}
            />
            <Show when={hoverTime()}>
              {(t) => (
                <span
                  class="player-hover-time"
                  style={{ left: `${(t() / Math.max(duration(), 0.001)) * 100}%` }}
                >
                  {formatDuration(t())}
                </span>
              )}
            </Show>
          </div>
          <span class="player-time">{formatDuration(duration())}</span>
        </div>

        <div class="player-buttons">
          <button
            type="button"
            aria-label={s().paused ? "Play" : "Pause"}
            onClick={() => void commands.playerTogglePause()}
          >
            {s().paused ? "▶" : "❚❚"}
          </button>
          <button type="button" aria-label="Back 10 seconds" onClick={() => seekBy(-10)}>
            −10
          </button>
          <button type="button" aria-label="Forward 10 seconds" onClick={() => seekBy(10)}>
            +10
          </button>
          <Show when={s().paused}>
            <button
              type="button"
              aria-label="Previous frame"
              onClick={() => void commands.playerFrameStep("back")}
            >
              ‹
            </button>
            <button
              type="button"
              aria-label="Next frame"
              onClick={() => void commands.playerFrameStep("forward")}
            >
              ›
            </button>
          </Show>

          <span class="player-spacer" />

          <label class="player-speed">
            <span class="visually-hidden">Playback speed</span>
            <select
              aria-label="Playback speed"
              value={String(s().speed ?? 1)}
              onChange={(e) => void commands.playerSetSpeed(Number(e.currentTarget.value))}
            >
              <For each={[...SPEEDS]}>{(speed) => <option value={speed}>{speed}×</option>}</For>
            </select>
          </label>
          <button
            type="button"
            aria-label={s().muted ? "Unmute" : "Mute"}
            onClick={() => void commands.playerSetMuted(!s().muted)}
          >
            {s().muted ? "🔇" : "🔊"}
          </button>
          <input
            class="player-volume"
            type="range"
            aria-label="Volume"
            min={0}
            max={100}
            step={1}
            value={s().volume ?? 100}
            onInput={(e) => void commands.playerSetVolume(Number(e.currentTarget.value))}
          />
          <button
            type="button"
            aria-label={s().fullscreen ? "Exit fullscreen" : "Fullscreen"}
            onClick={() => void commands.playerSetFullscreen(!s().fullscreen)}
          >
            {s().fullscreen ? "⤡" : "⤢"}
          </button>
        </div>
      </div>
    </div>
  );
}
