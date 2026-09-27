import {
  createEffect,
  createResource,
  createSignal,
  For,
  onCleanup,
  onMount,
  Show,
} from "solid-js";
import { commands } from "../bindings";
import type { AppError, SceneListItem } from "../bindings";
import { appErrorMessage } from "../messages/failures";
import { playerErrorMessage } from "../messages/player";
import Controls from "./Controls";
import { formatDuration } from "./format";
import { handlePlayerKey } from "./keyboard";
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
  // Spike test set: random 4K / WMV / VP9 / AV1 / MPEG-4 / FLV scenes (research R7).
  const [testSets, { refetch: shuffleTestSets }] = createResource(async () => {
    const res = await commands.listTestScenes();
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

  // Keyboard map while a scene is open (FR-009).
  const onKey = (e: KeyboardEvent) => {
    handlePlayerKey(e, player.snapshot());
  };
  onMount(() => document.addEventListener("keydown", onKey));
  onCleanup(() => document.removeEventListener("keydown", onKey));

  const showControls = () => ["playing", "paused", "ended"].includes(player.snapshot().state);

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

        <Show when={showControls()}>
          <Controls />
        </Show>

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
            {(items) => <SceneList scenes={items()} onPlay={(id) => void play(id)} />}
          </Show>
        </Show>

        <div class="player-subtitle-row">
          <h2 class="player-subtitle">Test scenes</h2>
          <button type="button" onClick={() => void shuffleTestSets()}>
            Shuffle
          </button>
        </div>
        <p class="lede">
          Random picks of hard-to-play formats: 4K, WMV, VP9, AV1, and older codecs.
        </p>
        <Show when={!testSets.error} fallback={<p class="lede">Couldn't load test scenes.</p>}>
          <Show when={testSets()} fallback={<p class="lede">Loading…</p>}>
            {(groups) => (
              <For each={groups()}>
                {(group) => (
                  <section class="player-group" aria-label={group.label}>
                    <h3 class="player-group-title">{group.label}</h3>
                    <SceneList scenes={group.scenes} onPlay={(id) => void play(id)} />
                  </section>
                )}
              </For>
            )}
          </Show>
        </Show>
      </section>
    );
  }
}

function SceneList(props: { scenes: SceneListItem[]; onPlay: (id: string) => void }) {
  return (
    <ul class="player-list">
      <For each={props.scenes}>
        {(scene) => (
          <li>
            <button
              type="button"
              class="player-row"
              aria-label={`Play ${scene.title}`}
              onClick={() => props.onPlay(scene.id)}
            >
              <span class="player-row-title">{scene.title}</span>
              <span class="player-row-meta">
                {[scene.resolution, scene.videoCodec, scene.container].filter(Boolean).join(" · ")}
              </span>
              <span class="player-row-duration">{formatDuration(scene.durationSeconds ?? 0)}</span>
            </button>
          </li>
        )}
      </For>
    </ul>
  );
}
