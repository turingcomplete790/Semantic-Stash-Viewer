import { createEffect, createSignal, For, onCleanup, Show } from "solid-js";
import { commands } from "../bindings";
import type { JobStatus, Notification, Severity } from "../bindings";
import { isActiveJob, notifications } from "../state/notifications";
import { BellIcon, CloseIcon } from "./icons";

const [open, setOpen] = createSignal(false);

/** Open or close the centre (the bell, and `g n`). */
export function toggleNotifications(): void {
  setOpen((o) => !o);
}

const SEVERITY_LABEL: Record<Severity, string> = {
  info: "Information",
  warning: "Warning",
  error: "Error",
};

const JOB_STATUS_LABEL: Record<JobStatus, string> = {
  queued: "Queued",
  running: "Running",
  stopping: "Stopping",
  finished: "Finished",
  failed: "Failed",
  cancelled: "Cancelled",
  unknown: "Status unknown",
};

/** "just now", "5 min ago", "3 h ago", or a date. */
export function relativeTime(iso: string, now = Date.now()): string {
  const seconds = Math.max(0, (now - Date.parse(iso)) / 1000);
  if (seconds < 60) return "just now";
  if (seconds < 3600) return `${Math.floor(seconds / 60)} min ago`;
  if (seconds < 86_400) return `${Math.floor(seconds / 3600)} h ago`;
  return new Date(iso).toLocaleDateString();
}

/**
 * The bell and the notification centre (constitution Principle IX; 004 FR-017–FR-023): unread
 * badge, activity marker while Stash jobs run, and the list with job progress and dismiss.
 */
export default function NotificationBell() {
  let root: HTMLDivElement | undefined;

  const label = () => {
    const unread = notifications.unreadCount();
    return [
      "Notifications",
      unread > 0 ? `${unread} unread` : null,
      notifications.active() ? "jobs running" : null,
    ]
      .filter(Boolean)
      .join(", ");
  };

  // Opening the centre marks what it shows as read.
  createEffect(() => {
    if (!open()) return;
    const unread = notifications
      .list()
      .filter((n) => !n.read)
      .map((n) => n.id);
    if (unread.length > 0) void commands.notificationsMarkRead(unread);
  });

  const onDocClick = (e: MouseEvent) => {
    if (open() && root && !root.contains(e.target as Node)) setOpen(false);
  };
  document.addEventListener("mousedown", onDocClick);
  onCleanup(() => {
    document.removeEventListener("mousedown", onDocClick);
    setOpen(false);
  });

  return (
    <div class="nav-notifications" ref={root}>
      <button
        type="button"
        class="nav-icon-button"
        classList={{ active: notifications.active() }}
        aria-label={label()}
        title="Notifications (g n)"
        aria-expanded={open()}
        onClick={toggleNotifications}
      >
        <BellIcon />
        <Show when={notifications.unreadCount() > 0}>
          <span class="nav-badge" aria-hidden="true">
            {notifications.unreadCount()}
          </span>
        </Show>
        <Show when={notifications.active()}>
          <span class="nav-activity" aria-hidden="true" />
        </Show>
      </button>
      <Show when={open()}>
        <div
          class="nav-panel notification-centre"
          role="dialog"
          aria-label="Notifications"
          onKeyDown={(e) => {
            if (e.key === "Escape") setOpen(false);
          }}
        >
          <div class="notification-header">
            <h2>Notifications</h2>
            <Show when={notifications.list().length > 0}>
              <button type="button" onClick={() => void commands.notificationsDismissAll()}>
                Dismiss all
              </button>
            </Show>
          </div>
          <Show
            when={notifications.list().length > 0}
            fallback={<p class="placeholder">No notifications</p>}
          >
            <ul class="notification-list">
              <For each={notifications.list()}>{(n) => <Item n={n} />}</For>
            </ul>
          </Show>
        </div>
      </Show>
    </div>
  );
}

function Item(props: { n: Notification }) {
  const job = () => props.n.job;
  const percent = () => {
    const p = job()?.progress;
    return typeof p === "number" ? Math.round(p * 100) : null;
  };
  return (
    <li class={`notification sev-${props.n.severity}`} classList={{ unread: !props.n.read }}>
      <div class="notification-main">
        <span class="notification-severity">{SEVERITY_LABEL[props.n.severity]}</span>
        <strong class="notification-title">{props.n.title}</strong>
        <Show when={props.n.detail}>
          <p class="notification-detail">{props.n.detail}</p>
        </Show>
        <Show when={job()}>
          {(j) => (
            <div class="notification-job">
              <Show when={isActiveJob(props.n)}>
                <div
                  class="job-progress"
                  classList={{ indeterminate: percent() === null }}
                  role="progressbar"
                  aria-label={`${props.n.title} progress`}
                  aria-valuemin={0}
                  aria-valuemax={100}
                  aria-valuenow={percent() ?? undefined}
                >
                  <div
                    class="job-progress-fill"
                    style={{ transform: `scaleX(${(percent() ?? 0) / 100})` }}
                  />
                </div>
              </Show>
              <span class="job-status">{JOB_STATUS_LABEL[j().status]}</span>
            </div>
          )}
        </Show>
        <time class="notification-time" dateTime={props.n.updatedAt}>
          {relativeTime(props.n.updatedAt)}
        </time>
      </div>
      <button
        type="button"
        class="notification-dismiss"
        aria-label={`Dismiss ${props.n.title}`}
        onClick={() => void commands.notificationDismiss(props.n.id)}
      >
        <CloseIcon />
      </button>
    </li>
  );
}
