import { describe, expect, it } from "vitest";
import type { PlayerError } from "../../bindings";
import { playerErrorMessage } from "../../messages/player";

// One sample per PlayerError kind; the mapped type makes TypeScript flag a missing kind.
const samples: { [K in PlayerError["kind"]]: Extract<PlayerError, { kind: K }> } = {
  notConnected: { kind: "notConnected" },
  sceneNotFound: { kind: "sceneNotFound" },
  noPlayableFile: { kind: "noPlayableFile" },
  streamUnreachable: { kind: "streamUnreachable" },
  unsupportedFormat: { kind: "unsupportedFormat" },
  playbackFailed: { kind: "playbackFailed", detail: "mpv error -1" },
};

describe("player error messages (FR-006)", () => {
  it.each(Object.values(samples))("$kind has a plain-language message", (error) => {
    const msg = playerErrorMessage(error);
    expect(msg.title.length).toBeGreaterThan(0);
    expect(msg.detail.length).toBeGreaterThan(0);
  });

  it("gives every kind a distinct title", () => {
    const titles = Object.values(samples).map((e) => playerErrorMessage(e).title);
    expect(new Set(titles).size).toBe(titles.length);
  });
});
