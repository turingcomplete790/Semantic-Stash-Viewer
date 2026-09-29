import { createEffect, on, Show } from "solid-js";
import { commands } from "../bindings";
import { player } from "../player/state";
import { PauseIcon, PlayIcon } from "./icons";
import { isPlayingTab, select, selectedId, tabs } from "./tabs";

/**
 * "Now playing" (004 FR-015; data-model "Now playing"): while a scene plays and its tab isn't
 * selected, a bar in the shell shows its title, play/pause, and a way back. It also hides the
 * video surface while another tab is shown, and shows it again on return (research R6).
 */
export default function NowPlayingBar() {
  const playingTab = () => tabs().find((t) => isPlayingTab(t));
  const sceneShown = () => playingTab()?.id === selectedId();
  const visible = () => {
    const snap = player.snapshot();
    return !!playingTab() && !sceneShown() && !snap.fullscreen;
  };

  // The video surface is only visible while the playing scene's tab is selected.
  createEffect(
    on(
      () => !player.isOpen() || sceneShown(),
      (show) => void commands.playerSetVideoVisible(show),
    ),
  );

  return (
    <Show when={visible()}>
      <section class="now-playing" aria-label="Now playing">
        <button
          type="button"
          class="nav-icon-button"
          aria-label={player.snapshot().paused ? "Play" : "Pause"}
          onClick={() => void commands.playerTogglePause()}
        >
          <Show when={player.snapshot().paused} fallback={<PauseIcon />}>
            <PlayIcon />
          </Show>
        </button>
        <span class="now-playing-title">{player.snapshot().title ?? "Scene"}</span>
        <button
          type="button"
          class="now-playing-back"
          onClick={() => {
            const tab = playingTab();
            if (tab) select(tab.id);
          }}
        >
          Back to scene
        </button>
      </section>
    </Show>
  );
}
