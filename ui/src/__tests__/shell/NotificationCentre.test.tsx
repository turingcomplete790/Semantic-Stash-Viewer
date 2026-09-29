import { cleanup, fireEvent, render, screen, within } from "@solidjs/testing-library";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Notification } from "../../bindings";

const mocks = vi.hoisted(() => {
  let handler: ((e: { payload: unknown }) => void) | undefined;
  return {
    emit: (payload: unknown) => handler?.({ payload }),
    commands: {
      notificationsList: vi.fn(),
      notificationsMarkRead: vi.fn(() => Promise.resolve(null)),
      notificationDismiss: vi.fn(() => Promise.resolve(null)),
      notificationsDismissAll: vi.fn(() => Promise.resolve(null)),
    },
    events: {
      notificationsChanged: {
        listen: vi.fn((h: (e: { payload: unknown }) => void) => {
          handler = h;
          return Promise.resolve(() => {});
        }),
      },
    },
  };
});
vi.mock("../../bindings", () => ({ commands: mocks.commands, events: mocks.events }));

import NotificationBell from "../../shell/NotificationCentre";
import { initNotifications } from "../../state/notifications";

function n(partial: Partial<Notification>): Notification {
  return {
    id: "1",
    key: null,
    profileId: null,
    kind: "connection",
    severity: "info",
    title: "Title",
    detail: null,
    createdAt: new Date().toISOString(),
    updatedAt: new Date().toISOString(),
    read: false,
    toast: false,
    job: null,
    ...partial,
  };
}

const unreachable = n({
  id: "a",
  severity: "warning",
  title: "Server unreachable",
  detail: "Retrying.",
});
const scan = n({
  id: "b",
  kind: "job",
  title: "Scanning...",
  read: true,
  job: { status: "running", progress: 0.4, startedAt: null, endedAt: null },
});
const generate = n({
  id: "c",
  kind: "job",
  title: "Generating...",
  read: true,
  job: { status: "running", progress: null, startedAt: null, endedAt: null },
});

beforeEach(async () => {
  vi.clearAllMocks();
  mocks.commands.notificationsList.mockResolvedValue([unreachable, scan]);
  await initNotifications();
});
afterEach(cleanup);

describe("notification centre", () => {
  it("shows the unread count and an activity marker while a job runs", () => {
    render(() => <NotificationBell />);
    const bell = screen.getByRole("button", { name: /Notifications/ });
    expect(bell).toHaveAccessibleName("Notifications, 1 unread, jobs running");
    expect(within(bell).getByText("1")).toBeInTheDocument();
  });

  it("lists newest first with severity, title, detail, and time, and marks them read", () => {
    render(() => <NotificationBell />);
    fireEvent.click(screen.getByRole("button", { name: /Notifications/ }));
    const panel = screen.getByRole("dialog", { name: "Notifications" });
    const items = within(panel).getAllByRole("listitem");
    expect(items[0]).toHaveTextContent("Server unreachable");
    expect(items[0]).toHaveTextContent("Retrying.");
    expect(items[0]).toHaveTextContent("Warning");
    expect(items[0]).toHaveTextContent("just now");
    expect(mocks.commands.notificationsMarkRead).toHaveBeenCalledWith(["a"]);
  });

  it("shows job progress, determinate or not, with the status", () => {
    mocks.emit([generate, scan]);
    render(() => <NotificationBell />);
    fireEvent.click(screen.getByRole("button", { name: /Notifications/ }));
    const bars = screen.getAllByRole("progressbar");
    expect(bars[0]).not.toHaveAttribute("aria-valuenow"); // indeterminate
    expect(bars[1]).toHaveAttribute("aria-valuenow", "40");
    expect(screen.getAllByText("Running")).toHaveLength(2);
  });

  it("dismisses one or all", () => {
    render(() => <NotificationBell />);
    fireEvent.click(screen.getByRole("button", { name: /Notifications/ }));
    fireEvent.click(screen.getByRole("button", { name: "Dismiss Server unreachable" }));
    expect(mocks.commands.notificationDismiss).toHaveBeenCalledWith("a");
    fireEvent.click(screen.getByRole("button", { name: "Dismiss all" }));
    expect(mocks.commands.notificationsDismissAll).toHaveBeenCalled();
  });

  it("updates when the core sends a new list", async () => {
    render(() => <NotificationBell />);
    mocks.emit([]);
    const bell = screen.getByRole("button", { name: /Notifications/ });
    expect(bell).toHaveAccessibleName("Notifications");
    fireEvent.click(bell);
    expect(screen.getByText("No notifications")).toBeInTheDocument();
  });
});
