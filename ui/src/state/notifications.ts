import { createSignal } from "solid-js";
import { commands, events } from "../bindings";
import type { Notification } from "../bindings";

/** The notification centre's list, kept in sync with the core (004 US3). Newest first. */
const [list, setList] = createSignal<Notification[]>([]);

const ACTIVE = new Set(["queued", "running", "stopping"]);

export const notifications = {
  list,
  unreadCount: () => list().filter((n) => !n.read).length,
  /** Any Stash job still going (the bell's activity marker). */
  active: () => list().some((n) => n.job !== null && ACTIVE.has(n.job.status)),
};

export function isActiveJob(n: Notification): boolean {
  return n.job !== null && ACTIVE.has(n.job.status);
}

let unlisten: (() => void) | undefined;

/** Subscribe to `notifications-changed`, then hydrate. Events that arrive meanwhile win. */
export async function initNotifications(): Promise<void> {
  unlisten?.();
  let sawEvent = false;
  unlisten = await events.notificationsChanged.listen((e) => {
    sawEvent = true;
    setList(e.payload);
  });
  const current = await commands.notificationsList();
  if (!sawEvent) setList(current);
}
