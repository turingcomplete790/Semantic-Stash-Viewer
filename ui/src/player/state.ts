import { createSignal, untrack } from "solid-js";
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

const [snapshot, setSnapshotRaw] = createSignal<PlayerSnapshot>(idle);

/**
 * Where a seek is headed, shown by the seek bar until mpv reports it's there. Without it the
 * bar snaps back to the old position until the seek completes — about a second on files
 * decoded in software (e.g. WMV), where exact seeks are slow.
 */
const [seekTarget, setSeekTarget] = createSignal<number | null>(null);
let seekTargetTimer: ReturnType<typeof setTimeout> | undefined;

/** Give up on a target after this long, e.g. if the seek failed. */
const SEEK_TARGET_TIMEOUT_MS = 5000;
/** Close enough to count as arrived. */
const SEEK_ARRIVED_SECONDS = 1;

function clearSeekTarget() {
  clearTimeout(seekTargetTimer);
  setSeekTarget(null);
}

function setSnapshot(next: PlayerSnapshot) {
  setSnapshotRaw(next);
  const target = untrack(seekTarget);
  if (target === null) return;
  const arrived = Math.abs((next.positionSeconds ?? 0) - target) < SEEK_ARRIVED_SECONDS;
  if (arrived || next.state === "idle" || next.state === "loading" || next.state === "error") {
    clearSeekTarget();
  }
}

/** Player state from the core (`player-state` events). */
export const player = {
  snapshot,
  /** A scene is loaded (playing, paused, ended, loading, or failed). */
  isOpen: () => snapshot().state !== "idle",
  /** Position to display: an in-flight seek's target, otherwise mpv's position. */
  displayPosition: () => seekTarget() ?? snapshot().positionSeconds ?? 0,
};

/** Remember where a seek is headed so the seek bar shows it right away. */
export function expectPosition(target: number) {
  const duration = snapshot().durationSeconds;
  const clamped = Math.max(0, duration ? Math.min(target, duration) : target);
  clearTimeout(seekTargetTimer);
  setSeekTarget(clamped);
  seekTargetTimer = setTimeout(() => setSeekTarget(null), SEEK_TARGET_TIMEOUT_MS);
}

/** Absolute seek that the seek bar reflects immediately. */
export function seekTo(position: number, exact: boolean) {
  expectPosition(position);
  void commands.playerSeek(position, exact);
}

/** Relative seek (±10 s) that the seek bar reflects immediately. */
export function seekBy(seconds: number) {
  expectPosition(player.displayPosition() + seconds);
  void commands.playerSeekRelative(seconds);
}

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
