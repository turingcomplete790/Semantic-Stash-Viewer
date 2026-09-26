import { createEffect, createResource, createSignal, For, onCleanup, Show } from "solid-js";
import { commands } from "../bindings";
import type { AppError } from "../bindings";
import { appErrorMessage } from "../messages/failures";
import { playerErrorMessage } from "../messages/player";
import { formatDuration } from "./format";
import { player } from "./state";
import "../components/ConnectionForm.css";
import "../components/ProfileManager.css";
import "./player.css";

/**
 * Spike player screen (US1): pick a scene (recent list or ID), then watch it. The video itself
 * is drawn by mpv on the GPU *under* the webview; while a scene is open this page makes itself
 * transparent so the video shows through.
 */
export default function PlayerScreen(props: { onExit: () => void }) {
  const [recent] = createResource(async () => {
    const res = await commands.listRecentScenes();
    if (res.status === "error") throw res.error;
    return res.data;
  });
  const [sceneId, setSceneId] = createSignal("");
  const [openError, setOpenError] = createSignal<AppError | null>(null);

  // Transparent page while a scene is open (FR-002); restored on close or unmount.
  createEffect(() => {
    document.documentElement.classList.toggle("player-open", player.isOpen());
  });
  onCleanup(() => document.documentElement.classList.remove("player-open"));

  async function play(id: string) {
    const trimmed = id.trim();
    if (!trimmed) return;
    setOpenError(null);
    const res = await commands.playerOpen(trimmed);
    if (res.status === "error") setOpenError(res.error);
  }

  function close() {
    void commands.playerClose();
  }

  return (
    <Show when={player.isOpen()} fallback={picker()}>
      <div class="player-stage">
        <div class="player-topbar">
          <span class="player-title">{player.snapshot().title ?? "Scene"}</span>
          <button type="button" aria-label="Close player" onClick={close}>
            ✕
          </button>
        </div>

        <Show when={player.snapshot().state === "loading"}>
          <div class="player-center">Loading…</div>
        </Show>

        <Show when={player.snapshot().error}>
          {(err) => (
            <div class="player-center">
              <div class="notice error player-error" role="alert">
                <strong>{playerErrorMessage(err()).title}</strong>
                <p>{playerErrorMessage(err()).detail}</p>
                <Show when={playerErrorMessage(err()).hint}>
                  <p class="hint">{playerErrorMessage(err()).hint}</p>
                </Show>
                <button type="button" onClick={close}>
                  Back to scenes
                </button>
              </div>
            </div>
          )}
        </Show>
      </div>
    </Show>
  );

  function picker() {
    return (
      <section class="connect-card player-picker" aria-labelledby="player-title">
        <div class="pm-header">
          <h1 id="player-title">Player</h1>
          <button type="button" onClick={() => props.onExit()}>
            ← Back
          </button>
        </div>

        <form
          class="player-id-form"
          onSubmit={(e) => {
            e.preventDefault();
            void play(sceneId());
          }}
        >
          <div class="field">
            <label for="player-scene-id">Scene ID</label>
            <input
              id="player-scene-id"
              type="text"
              inputmode="numeric"
              value={sceneId()}
              onInput={(e) => setSceneId(e.currentTarget.value)}
            />
          </div>
          <button type="submit" class="primary" disabled={!sceneId().trim()}>
            Play
          </button>
        </form>

        <Show when={openError()}>
          {(err) => (
            <div class="notice error" role="alert">
              <strong>{appErrorMessage(err()).title}</strong>
              <p>{appErrorMessage(err()).detail}</p>
            </div>
          )}
        </Show>

        <h2 class="player-subtitle">Recently added</h2>
        <Show when={!recent.error} fallback={<p class="lede">Couldn't load recent scenes.</p>}>
          <Show when={recent()} fallback={<p class="lede">Loading…</p>}>
            {(items) => (
              <ul class="player-list">
                <For each={items()}>
                  {(scene) => (
                    <li>
                      <button
                        type="button"
                        class="player-row"
                        aria-label={`Play ${scene.title}`}
                        onClick={() => void play(scene.id)}
                      >
                        <span class="player-row-title">{scene.title}</span>
                        <span class="player-row-meta">
                          {[scene.resolution, scene.videoCodec].filter(Boolean).join(" · ")}
                        </span>
                        <span class="player-row-duration">
                          {formatDuration(scene.durationSeconds ?? 0)}
                        </span>
                      </button>
                    </li>
                  )}
                </For>
              </ul>
            )}
          </Show>
        </Show>
      </section>
    );
  }
}
