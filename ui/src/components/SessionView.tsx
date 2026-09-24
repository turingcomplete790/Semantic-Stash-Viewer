import { Match, Show, Switch } from "solid-js";
import type { ConnectFailure, ProfileSummary } from "../bindings";
import { connectFailureMessage } from "../messages/failures";
import { connection } from "../state/connection";
import ConnectedSummary from "./ConnectedSummary";
import ServerSummary from "./ServerSummary";
import "./ConnectionForm.css";

/**
 * Main area while a profile is active. The UI stays usable in every state, including offline
 * at launch (edge case "launch with the last server offline").
 */
export default function SessionView(props: {
  profile: ProfileSummary | undefined;
  onAddAnother: () => void;
  onRetry: () => void;
  onUpdateKey: () => void;
}) {
  const snap = () => connection.snapshot();
  const name = () => props.profile?.displayName ?? "the server";
  const url = () => snap().finalUrl ?? props.profile?.baseUrl ?? "";
  const failure = (): ConnectFailure | undefined => {
    const s = snap().state;
    return s.kind === "failed" || s.kind === "authFailed" ? s.failure : undefined;
  };

  const otherServer = (
    <button type="button" onClick={() => props.onAddAnother()}>
      Connect to another server
    </button>
  );

  return (
    <Switch>
      <Match when={snap().state.kind === "connected" && snap().server}>
        {(server) => (
          <ConnectedSummary
            name={name()}
            url={url()}
            server={server()}
            onAddAnother={props.onAddAnother}
          />
        )}
      </Match>
      <Match when={snap().state.kind === "connecting"}>
        <section class="connect-card">
          <h1>Connecting to {name()}…</h1>
          <p class="lede">{url()}</p>
          <div class="actions">{otherServer}</div>
        </section>
      </Match>
      <Match when={snap().state.kind === "offline"}>
        <section class="connect-card">
          <h1>Can't reach {name()}</h1>
          <p class="lede">
            The viewer keeps retrying on its own and reconnects as soon as the server is back.
          </p>
          <Show when={snap().server}>
            {(server) => <ServerSummary url={url()} server={server()} />}
          </Show>
          <div class="actions" style={{ "margin-top": "18px" }}>
            {otherServer}
          </div>
        </section>
      </Match>
      <Match when={failure()}>
        {(f) => {
          const msg = () => connectFailureMessage(f());
          const isAuth = () => snap().state.kind === "authFailed";
          return (
            <section class="connect-card">
              <h1>{name()}</h1>
              <div class="notice error" role="alert">
                <strong>{msg().title}</strong>
                <p>{msg().detail}</p>
                <Show when={msg().hint}>
                  <p class="hint">{msg().hint}</p>
                </Show>
              </div>
              <div class="actions" style={{ "margin-top": "18px" }}>
                <Show
                  when={isAuth()}
                  fallback={
                    <button type="button" class="primary" onClick={() => props.onRetry()}>
                      Try again
                    </button>
                  }
                >
                  <button type="button" class="primary" onClick={() => props.onUpdateKey()}>
                    Update key
                  </button>
                </Show>
                {otherServer}
              </div>
            </section>
          );
        }}
      </Match>
    </Switch>
  );
}
