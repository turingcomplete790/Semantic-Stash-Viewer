import { createSignal, Match, onCleanup, Show, Switch } from "solid-js";
import { connection } from "../state/connection";
import "./ConnectionIndicator.css";

/** Always-visible connection state in the top bar (FR-015). */
export default function ConnectionIndicator(props: {
  profileName?: string;
  onUpdateKey: () => void;
}) {
  const state = () => connection.snapshot().state;

  // Ticks once a second so the offline countdown stays current.
  const [now, setNow] = createSignal(Date.now());
  const timer = setInterval(() => setNow(Date.now()), 1000);
  onCleanup(() => clearInterval(timer));

  const retryIn = () => {
    const s = state();
    if (s.kind !== "offline") return 0;
    return Math.max(0, Math.ceil((Date.parse(s.nextRetryAt) - now()) / 1000));
  };

  return (
    <div class={`conn-indicator conn-${state().kind}`} role="status" aria-live="polite">
      <span class="conn-dot" aria-hidden="true" />
      <Switch>
        <Match when={state().kind === "idle"}>
          <span class="conn-label">Not connected</span>
        </Match>
        <Match when={state().kind === "connecting"}>
          <span class="conn-label">Connecting…</span>
        </Match>
        <Match when={state().kind === "connected"}>
          <span class="conn-label">Connected</span>
        </Match>
        <Match when={state().kind === "offline"}>
          <span class="conn-label">Offline</span>
          <span class="conn-detail">retrying in {retryIn()}s</span>
        </Match>
        <Match when={state().kind === "authFailed"}>
          <span class="conn-label">Key rejected</span>
          <button type="button" class="conn-action" onClick={() => props.onUpdateKey()}>
            Update key
          </button>
        </Match>
        <Match when={state().kind === "failed"}>
          <span class="conn-label">Connection failed</span>
        </Match>
      </Switch>
      <Show when={props.profileName && state().kind !== "idle"}>
        <span class="conn-profile">{props.profileName}</span>
      </Show>
    </div>
  );
}
