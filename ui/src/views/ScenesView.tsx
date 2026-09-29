import { createResource, createSignal, For, Show } from "solid-js";
import { commands } from "../bindings";
import type { SceneGroup, SceneListItem } from "../bindings";
import { formatDuration } from "../player/format";
import { navigate } from "../shell/tabs";
import { onViewDataChanged } from "../state/viewData";
import "../components/ConnectionForm.css";
import "../components/ProfileManager.css";
import "../player/player.css";

/**
 * Scenes: the spike's scene picker (recently added, open by ID, and the test set). Phase 1
 * replaces it with the real scene grid. Choosing a scene opens it in the Scene view.
 */
export default function ScenesView() {
  // Shown from the cache when there is one; a refresh that changes it updates in place (003).
  const [recent, { refetch: reloadRecent }] = createResource(async () => {
    const res = await commands.listRecentScenes();
    if (res.status === "error") throw res.error;
    return res.data.data;
  });
  onViewDataChanged("scenes:recent", () => void reloadRecent());
  // Spike test set: random 4K / WMV / VP9 / AV1 / MPEG-4 / FLV scenes (002 research R7). It
  // stays the same until Shuffle.
  const [testSets, { refetch: reloadTestSets }] = createResource<SceneGroup[], string>(
    async (_, info) => {
      const res = await commands.listTestScenes(info.refetching === "shuffle");
      if (res.status === "error") throw res.error;
      return res.data.data;
    },
  );
  onViewDataChanged("scenes:test-set", () => void reloadTestSets());
  const [sceneId, setSceneId] = createSignal("");

  function open(id: string, title = "", newTab = false) {
    const trimmed = id.trim();
    if (trimmed) navigate({ kind: "scene", sceneId: trimmed, title }, { newTab });
  }

  return (
    <div class="view-page">
      <section class="connect-card player-picker" aria-labelledby="scenes-title">
        <h1 id="scenes-title">Scenes</h1>

        <form
          class="player-id-form"
          onSubmit={(e) => {
            e.preventDefault();
            open(sceneId());
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

        <h2 class="player-subtitle">Recently added</h2>
        <Show when={!recent.error} fallback={<p class="lede">Couldn't load recent scenes.</p>}>
          <Show when={recent.latest} fallback={<p class="lede">Loading…</p>}>
            {(items) => <SceneList scenes={items()} onPlay={open} />}
          </Show>
        </Show>

        <div class="player-subtitle-row">
          <h2 class="player-subtitle">Test scenes</h2>
          <button type="button" onClick={() => void reloadTestSets("shuffle")}>
            Shuffle
          </button>
        </div>
        <p class="lede">
          Random picks of hard-to-play formats: 4K, WMV, VP9, AV1, and older codecs.
        </p>
        <Show when={!testSets.error} fallback={<p class="lede">Couldn't load test scenes.</p>}>
          <Show when={testSets.latest} fallback={<p class="lede">Loading…</p>}>
            {(groups) => (
              <For each={groups()}>
                {(group) => (
                  <section class="player-group" aria-label={group.label}>
                    <h3 class="player-group-title">{group.label}</h3>
                    <SceneList scenes={group.scenes} onPlay={open} />
                  </section>
                )}
              </For>
            )}
          </Show>
        </Show>
      </section>
    </div>
  );
}

function SceneList(props: {
  scenes: SceneListItem[];
  onPlay: (id: string, title: string, newTab: boolean) => void;
}) {
  return (
    <ul class="player-list">
      <For each={props.scenes}>
        {(scene) => (
          <li>
            <button
              type="button"
              class="player-row"
              aria-label={`Play ${scene.title}`}
              onClick={(e) => props.onPlay(scene.id, scene.title, e.ctrlKey || e.metaKey)}
              onAuxClick={(e) => {
                if (e.button === 1) props.onPlay(scene.id, scene.title, true);
              }}
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
