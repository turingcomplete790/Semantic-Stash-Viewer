import { createEffect, createResource, createSignal, on, onCleanup, onMount, Show } from "solid-js";
import { commands } from "../bindings";
import type { AppError } from "../bindings";
import { appErrorMessage } from "../messages/failures";
import { playerErrorMessage } from "../messages/player";
import Controls from "../player/Controls";
import { handlePlayerKey } from "../player/keyboard";
import { player } from "../player/state";
import { registerRawHandler, setSceneActive } from "../shell/keymap";
import { useTab } from "../shell/viewState";
import "../player/player.css";

/**
 * One scene, played in the shell's content area (004 FR-015, FR-029). mpv draws the video on the
 * GPU *under* the webview; while its tab is showing, this view makes its area transparent and
 * tells the core where that area is (research R6).
 *
 * - A scene tab restored after relaunch, or whose scene was replaced by another one, shows the
 *   title and a Play button and never plays on its own (research R10).
 * - Leaving the scene within its tab (back, or navigating elsewhere) stops playback, like a web
 *   page; switching tabs doesn't (the "now playing" bar takes over).
 */
export default function SceneView(props: {
  sceneId: string;
  title: string;
  /** Restored from disk: wait for Play instead of opening. */
  restored?: boolean;
  /** Leave the scene (after playback stopped). */
  onDone: () => void;
}) {
  let stage: HTMLDivElement | undefined;
  const tab = useTab();
  const [openError, setOpenError] = createSignal<AppError | null>(null);
  const [waiting, setWaiting] = createSignal(false);

  const snap = () => player.snapshot();
  const isThisScene = () => snap().sceneId === props.sceneId && player.isOpen();
  const showControls = () => isThisScene() && ["playing", "paused", "ended"].includes(snap().state);
  const shown = () => tab.isActive() && isThisScene();

  // A still of the scene under Play while waiting (restored or replaced tabs).
  const [poster] = createResource(
    () => (waiting() && !isThisScene() ? props.sceneId : false),
    async (id) => {
      const res = await commands.sceneScreenshotUrl(id);
      return res.status === "ok" ? res.data : null;
    },
  );

  function open() {
    setWaiting(false);
    setOpenError(null);
    void commands.playerOpen(props.sceneId).then((res) => {
      if (res.status === "error") setOpenError(res.error);
    });
  }

  // Open the scene unless it's loaded already. Restored tabs, and tabs opened while another scene
  // plays in the background, wait for Play.
  onMount(() => {
    if (isThisScene() && snap().state !== "error") return;
    if (props.restored || (!tab.isActive() && player.isOpen())) setWaiting(true);
    else open();
  });

  // When playback stops (✕, Escape, disconnect), leave the view. If another scene replaced this
  // one, wait for Play instead.
  createEffect(
    on(isThisScene, (now, before) => {
      if (!before || now) return;
      if (player.isOpen()) setWaiting(true);
      else props.onDone();
    }),
  );

  // Navigating away within the tab stops this scene (the view unmounts while its tab is shown).
  onCleanup(() => {
    if (tab.isActive() && isThisScene()) void commands.playerClose();
  });

  // Transparent page while this scene is on screen (002 FR-002); shell chrome hides in fullscreen.
  createEffect(() => {
    if (!tab.isActive()) return;
    document.documentElement.classList.toggle("player-open", isThisScene());
    document.documentElement.classList.toggle(
      "player-fullscreen",
      isThisScene() && snap().fullscreen,
    );
  });
  onCleanup(() => {
    if (tab.isActive()) {
      document.documentElement.classList.remove("player-open", "player-fullscreen");
    }
  });

  // Tell the core where to draw: this view's area, or the whole window in fullscreen.
  const report = () => {
    if (!shown()) return;
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
  createEffect(on([() => snap().fullscreen, shown], report));

  // The player's keyboard map, only while this scene's tab is showing (FR-009, research R7).
  onMount(() => {
    const remove = registerRawHandler("scene", (e) =>
      tab.isActive() ? handlePlayerKey(e, player.snapshot()) : false,
    );
    onCleanup(remove);
  });
  createEffect(() => {
    if (tab.isActive()) setSceneActive(() => tab.isActive() && isThisScene());
  });
  onCleanup(() => {
    if (tab.isActive()) setSceneActive(() => false);
  });

  function close() {
    void commands.playerClose();
  }

  return (
    <div ref={stage} class="player-stage">
      <div class="player-topbar">
        <span class="player-title">
          {isThisScene() ? (snap().title ?? props.title) : props.title || `Scene ${props.sceneId}`}
        </span>
        <Show when={isThisScene()}>
          <button type="button" aria-label="Close player" onClick={close}>
            ✕
          </button>
        </Show>
      </div>

      <Show when={isThisScene() && showControls()}>
        <Controls />
      </Show>

      <Show when={waiting() && !isThisScene()}>
        {/* Only while the tab is showing: WebKit lays out an image that loads in a hidden tab at
            zero size and keeps drawing it off-centre until the window is resized. */}
        <Show when={tab.isActive() && poster()}>
          {(src) => <img class="player-poster" src={src()} alt="" />}
        </Show>
        <div class="player-center">
          <button type="button" class="primary player-play" onClick={open}>
            Play
          </button>
        </div>
      </Show>

      <Show when={!waiting() && !openError() && (!isThisScene() || snap().state === "loading")}>
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
