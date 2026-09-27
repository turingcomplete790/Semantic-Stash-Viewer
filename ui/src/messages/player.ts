import type { PlayerError } from "../bindings";
import type { FailureMessage } from "./failures";

type Kind = PlayerError["kind"];
type Of<K extends Kind> = Extract<PlayerError, { kind: K }>;

// A Record over every kind makes TypeScript reject a missing one at compile time.
const messages: { [K in Kind]: (e: Of<K>) => FailureMessage } = {
  notConnected: () => ({
    title: "Not connected to a server",
    detail: "Connect to a Stash server to play scenes.",
  }),
  sceneNotFound: () => ({
    title: "That scene doesn't exist",
    detail: "The server has no scene with that ID.",
  }),
  noPlayableFile: () => ({
    title: "This scene has no video file",
    detail: "There's no file for the player to open.",
  }),
  streamUnreachable: () => ({
    title: "Couldn't open this scene's video",
    detail: "The server didn't send the video. The file may be missing, or the connection dropped.",
    hint: "Check that the file still exists in Stash, then try again.",
  }),
  unsupportedFormat: () => ({
    title: "This video format can't be played",
    detail: "The player couldn't decode this file.",
  }),
  playbackFailed: (e) => ({
    title: "Playback stopped",
    detail: e.detail,
  }),
};

export function playerErrorMessage(error: PlayerError): FailureMessage {
  const build = messages[error.kind] as (e: PlayerError) => FailureMessage;
  return build(error);
}
