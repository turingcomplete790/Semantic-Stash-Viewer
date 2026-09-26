import { createSignal, Match, onCleanup, Show, Switch } from "solid-js";
import type { SecurityState } from "../bindings";
import { securityText } from "../messages/security";
import { connection } from "../state/connection";
import ConnectionDetails from "./ConnectionDetails";
import "./ConnectionIndicator.css";

/** Always-visible connection health and security in the top bar (FR-015, FR-020). */
export default function ConnectionIndicator(props: {
  profileName?: string;
  onUpdateKey: () => void;
  onManage?: () => void;
}) {
  const state = () => connection.snapshot().state;
  const security = () => connection.snapshot().security;
  const [detailsOpen, setDetailsOpen] = createSignal(false);
  let securityButton: HTMLButtonElement | undefined;
  const closeDetails = (viaKeyboard?: boolean) => {
    setDetailsOpen(false);
    if (viaKeyboard) securityButton?.focus();
  };

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
    <div class={`conn-indicator conn-${state().kind}`}>
      <span class="conn-status" role="status" aria-live="polite">
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
          </Match>
          <Match when={state().kind === "failed"}>
            <span class="conn-label">Connection failed</span>
          </Match>
        </Switch>
        <Show when={props.profileName && state().kind !== "idle"}>
          <span class="conn-profile">{props.profileName}</span>
        </Show>
      </span>

      <Show when={state().kind === "authFailed"}>
        <button type="button" class="conn-action" onClick={() => props.onUpdateKey()}>
          Update key
        </button>
      </Show>

      <Show when={connection.snapshot().profileId}>
        <button
          ref={securityButton}
          type="button"
          class={`conn-security sec-${security() ?? "unknown"}`}
          aria-haspopup="dialog"
          aria-expanded={detailsOpen()}
          title="Connection details"
          onClick={() => setDetailsOpen((open) => !open)}
        >
          <LockIcon security={security() ?? null} />
          <span>{security() ? securityText[security() as SecurityState].label : "Details"}</span>
        </button>
      </Show>

      <Show when={detailsOpen()}>
        <ConnectionDetails
          url={connection.snapshot().finalUrl ?? connection.activeProfile()?.baseUrl ?? ""}
          security={security() ?? null}
          strictTls={connection.activeProfile()?.strictTls ?? false}
          onClose={closeDetails}
          onManage={() => props.onManage?.()}
        />
      </Show>
    </div>
  );
}

/** Closed lock when encrypted; open lock when not. */
function LockIcon(props: { security: SecurityState | null }) {
  const open = () => props.security === "unencrypted";
  return (
    <svg class="lock" width="14" height="14" viewBox="0 0 16 16" aria-hidden="true">
      <rect x="3" y="7" width="10" height="8" rx="1.5" fill="currentColor" />
      <path
        d={open() ? "M5 7V4.5a3 3 0 0 1 5.8-1" : "M5 7V4.5a3 3 0 0 1 6 0V7"}
        fill="none"
        stroke="currentColor"
        stroke-width="1.6"
      />
    </svg>
  );
}
