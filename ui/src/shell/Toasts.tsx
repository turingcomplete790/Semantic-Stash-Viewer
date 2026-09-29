import { createEffect, createSignal, For, onCleanup, Show } from "solid-js";
import type { Notification } from "../bindings";
import { player } from "../player/state";
import { notifications } from "../state/notifications";

const TOAST_MS = 5000;
const PER_KEY_GAP_MS = 10_000;
const MAX_VISIBLE = 3;

type Toast = { id: string; notification: Notification };

/**
 * Short pop-ups for important notifications (004 FR-020): bottom-right, a polite live region,
 * never focused, gone after 5 s, at most one per condition per 10 s and 3 at once, and hidden
 * during fullscreen playback.
 */
export default function Toasts() {
  const [visible, setVisible] = createSignal<Toast[]>([]);
  const seen = new Map<string, string>(); // id → updatedAt already considered
  const lastShown = new Map<string, number>(); // key (or id) → time shown
  const timers = new Set<ReturnType<typeof setTimeout>>();
  onCleanup(() => timers.forEach(clearTimeout));

  const remove = (id: string) => setVisible((list) => list.filter((t) => t.id !== id));

  createEffect(() => {
    for (const n of notifications.list()) {
      if (seen.get(n.id) === n.updatedAt) continue;
      seen.set(n.id, n.updatedAt);
      if (!n.toast) continue;
      const group = n.key ?? n.id;
      const now = Date.now();
      const last = lastShown.get(group);
      if (last !== undefined && now - last < PER_KEY_GAP_MS) continue;
      if (visible().length >= MAX_VISIBLE) continue;
      lastShown.set(group, now);
      const toast: Toast = { id: `${n.id}:${n.updatedAt}`, notification: n };
      setVisible((list) => [...list, toast]);
      const timer = setTimeout(() => {
        timers.delete(timer);
        remove(toast.id);
      }, TOAST_MS);
      timers.add(timer);
    }
  });

  return (
    <Show when={!player.snapshot().fullscreen}>
      <div class="toasts" role="status" aria-live="polite">
        <ul>
          <For each={visible()}>
            {(t) => (
              <li class={`toast sev-${t.notification.severity}`}>
                <strong>{t.notification.title}</strong>
                <Show when={t.notification.detail}>
                  <span>{t.notification.detail}</span>
                </Show>
              </li>
            )}
          </For>
        </ul>
      </div>
    </Show>
  );
}
