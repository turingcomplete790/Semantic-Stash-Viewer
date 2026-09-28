import { createEffect, createSignal, on, onCleanup, onMount, Show } from "solid-js";
import { commands } from "../bindings";
import type { AppError } from "../bindings";
import { appErrorMessage } from "../messages/failures";
import { playerErrorMessage } from "../messages/player";
import Controls from "../player/Controls";
import { handlePlayerKey } from "../player/keyboard";
import { player } from "../player/state";
import { registerRawHandler, setSceneActive } from "../shell/keymap";
import "../player/player.css";

/**
 * One scene, played in the shell's content area (004 FR-029). mpv draws the video on the GPU
 * *under* the webview; this view makes its area transparent and tells the core where that area
 * is, so the video sits below the navigation bar instead of behind it (research R6).
 */
export default function SceneView(props: {
  sceneId: string;
  title: string;
  /** Leave the scene (after closing the player, or when playback stops). */
  onDone: () => void;
}) {
  let stage: HTMLDivElement | undefined;
  const [openError, setOpenError] = createSignal<AppError | null>(null);

  const snap = () => player.snapshot();
  const isThisScene = () => snap().sceneId === props.sceneId && player.isOpen();
  const showControls = () => isThisScene() && ["playing", "paused", "ended"].includes(snap().state);

  // Open the scene unless it's already the one loaded.
  onMount(() => {
    if (isThisScene() && snap().state !== "error") return;
    void commands.playerOpen(props.sceneId).then((res) => {
      if (res.status === "error") setOpenError(res.error);
    });
  });

  // Leaving playback (✕, Escape, disconnect) leaves the view.
  createEffect(
    on(isThisScene, (now, before) => {
      if (before && !now) props.onDone();
    }),
  );

  // Transparent page while this scene is on screen (002 FR-002); shell chrome hides in fullscreen.
  createEffect(() => {
    document.documentElement.classList.toggle("player-open", isThisScene());
    document.documentElement.classList.toggle(
      "player-fullscreen",
      isThisScene() && snap().fullscreen,
    );
  });
  onCleanup(() => {
    document.documentElement.classList.remove("player-open", "player-fullscreen");
  });

  // Tell the core where to draw: this view's area, or the whole window in fullscreen.
  const report = () => {
    if (snap().fullscreen) {
      void commands.playerSetViewport(null);
      return;
    }
    if (!stage) return;
    const r = stage.getBoundingClientRect();
    void commands.playerSetViewport({
      x: Math.round(r.left),
      y: Math.round(r.top),
      width: Math.round(r.width),
      height: Math.round(r.height),
    });
  };
  onMount(() => {
    const observer = new ResizeObserver(report);
    if (stage) observer.observe(stage);
    window.addEventListener("resize", report);
    onCleanup(() => {
      observer.disconnect();
      window.removeEventListener("resize", report);
    });
  });
  createEffect(on(() => snap().fullscreen, report));

  // The player's keyboard map, while this view is shown (FR-009, research R7).
  onMount(() => {
    setSceneActive(() => true);
    const remove = registerRawHandler("scene", (e) => handlePlayerKey(e, player.snapshot()));
    onCleanup(() => {
      remove();
      setSceneActive(() => false);
    });
  });

  function close() {
    void commands.playerClose();
  }

  return (
    <div ref={stage} class="player-stage">
      <div class="player-topbar">
        <span class="player-title">{snap().title ?? (props.title || "Scene")}</span>
        <button type="button" aria-label="Close player" onClick={close}>
          ✕
        </button>
      </div>

      <Show when={showControls()}>
        <Controls />
      </Show>

      <Show when={!openError() && (!isThisScene() || snap().state === "loading")}>
        <div class="player-center">Loading…</div>
      </Show>

      <Show when={openError()}>
        {(err) => (
          <div class="player-center">
            <div class="notice error player-error" role="alert">
              <strong>{appErrorMessage(err()).title}</strong>
              <p>{appErrorMessage(err()).detail}</p>
              <button type="button" onClick={() => props.onDone()}>
                Back to scenes
              </button>
            </div>
          </div>
        )}
      </Show>

      <Show when={isThisScene() && snap().error}>
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
  );
}
