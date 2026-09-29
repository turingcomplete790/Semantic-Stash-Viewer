import { cleanup, render, screen } from "@solidjs/testing-library";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Notification, PlayerSnapshot } from "../../bindings";

const mocks = vi.hoisted(() => {
  const handlers: Record<string, (e: { payload: unknown }) => void> = {};
  const listen = (name: string) =>
    vi.fn((h: (e: { payload: unknown }) => void) => {
      handlers[name] = h;
      return Promise.resolve(() => {});
    });
  return {
    emit: (name: string, payload: unknown) => handlers[name]?.({ payload }),
    commands: {
      notificationsList: vi.fn(() => Promise.resolve([])),
      playerSnapshot: vi.fn(),
    },
    events: {
      notificationsChanged: { listen: listen("notifications") },
      playerState: { listen: listen("player") },
    },
  };
});
vi.mock("../../bindings", () => ({ commands: mocks.commands, events: mocks.events }));

import { initPlayer } from "../../player/state";
import Toasts from "../../shell/Toasts";
import { initNotifications } from "../../state/notifications";

let clock = 0;
function n(id: string, key: string | null, title: string, toast = true): Notification {
  clock += 1;
  const at = new Date(Date.UTC(2026, 8, 28, 12, 0, clock)).toISOString();
  return {
    id,
    key,
    profileId: null,
    kind: "connection",
    severity: "warning",
    title,
    detail: null,
    createdAt: at,
    updatedAt: at,
    read: false,
    toast,
    job: null,
  };
}

const idlePlayer: PlayerSnapshot = {
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

beforeEach(async () => {
  vi.clearAllMocks();
  vi.useFakeTimers({ shouldAdvanceTime: true });
  mocks.commands.playerSnapshot.mockResolvedValue({ status: "ok", data: idlePlayer });
  await initPlayer();
  mocks.emit("player", idlePlayer);
  await initNotifications();
});
afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

describe("toasts", () => {
  it("shows a toast in a polite live region without taking focus", () => {
    const input = document.createElement("input");
    document.body.appendChild(input);
    input.focus();
    render(() => <Toasts />);
    mocks.emit("notifications", [n("a", "connection:p", "Server unreachable")]);
    const region = screen.getByRole("status");
    expect(region).toHaveAttribute("aria-live", "polite");
    expect(region).toHaveTextContent("Server unreachable");
    expect(document.activeElement).toBe(input);
    input.remove();
  });

  it("ignores notifications that don't ask for a toast", () => {
    render(() => <Toasts />);
    mocks.emit("notifications", [n("a", null, "Quiet", false)]);
    expect(screen.queryByText("Quiet")).not.toBeInTheDocument();
  });

  it("hides after 5 seconds", () => {
    render(() => <Toasts />);
    mocks.emit("notifications", [n("a", null, "Hello")]);
    expect(screen.getByText("Hello")).toBeInTheDocument();
    vi.advanceTimersByTime(5100);
    expect(screen.queryByText("Hello")).not.toBeInTheDocument();
  });

  it("shows at most one toast per key per 10 s, and at most 3 at once", () => {
    render(() => <Toasts />);
    mocks.emit("notifications", [n("a", "connection:p", "Server unreachable")]);
    mocks.emit("notifications", [n("a", "connection:p", "Reconnected")]);
    expect(screen.queryByText("Reconnected")).not.toBeInTheDocument();
    vi.advanceTimersByTime(10_100);
    mocks.emit("notifications", [n("a", "connection:p", "Reconnected")]);
    expect(screen.getByText("Reconnected")).toBeInTheDocument();

    mocks.emit("notifications", [
      n("b", null, "One"),
      n("c", null, "Two"),
      n("d", null, "Three"),
      n("e", null, "Four"),
    ]);
    expect(screen.getAllByRole("listitem")).toHaveLength(3);
  });

  it("stays hidden in fullscreen playback", () => {
    render(() => <Toasts />);
    mocks.emit("player", { ...idlePlayer, sceneId: "1", state: "playing", fullscreen: true });
    mocks.emit("notifications", [n("a", null, "Hidden")]);
    expect(screen.queryByText("Hidden")).not.toBeInTheDocument();
  });
});
