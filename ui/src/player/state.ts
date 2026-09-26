import { createSignal } from "solid-js";
import { commands, events } from "../bindings";
import type { PlayerSnapshot } from "../bindings";

const idle: PlayerSnapshot = {
  sceneId: null,
  title: null,
  state: "idle",
  positionSeconds: 0,
  durationSeconds: null,
  paused: false,
  speed: 1,
  volume: 100,
  muted: false,
  fullscreen: false,
  hwdec: null,
  tracks: [],
  error: null,
};

const [snapshot, setSnapshot] = createSignal<PlayerSnapshot>(idle);

/** Player state from the core (`player-state` events). */
export const player = {
  snapshot,
  /** A scene is loaded (playing, paused, ended, loading, or failed). */
  isOpen: () => snapshot().state !== "idle",
};

let unlisten: (() => void) | undefined;

/** Subscribe to `player-state`, then hydrate. Events that arrive meanwhile win. */
export async function initPlayer(): Promise<void> {
  unlisten?.();
  let sawEvent = false;
  unlisten = await events.playerState.listen((e) => {
    sawEvent = true;
    setSnapshot(e.payload);
  });
  const res = await commands.playerSnapshot();
  if (!sawEvent && res.status === "ok") setSnapshot(res.data);
}
