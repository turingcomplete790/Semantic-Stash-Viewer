import { createSignal, Match, Show, Switch } from "solid-js";
import { commands } from "../bindings";
import type { AppError, ProfileDraft, ProfileSummary, TestResult } from "../bindings";
import { newRequestId } from "../lib/requestId";
import { appErrorMessage } from "../messages/failures";
import ServerSummary from "./ServerSummary";
import "./ConnectionForm.css";

type Phase =
  | { kind: "idle" }
  | { kind: "connecting"; requestId: string }
  | { kind: "failed"; error: AppError }
  | { kind: "connected"; result: TestResult }
  | { kind: "saving"; result: TestResult };

/** First-run / add-server screen (US1): address + optional API key → check → save. */
export default function ConnectionForm(props: {
  onSaved: (profile: ProfileSummary, result: TestResult) => void;
}) {
  const [address, setAddress] = createSignal("");
  const [apiKey, setApiKey] = createSignal("");
  const [displayName, setDisplayName] = createSignal("");
  const [phase, setPhase] = createSignal<Phase>({ kind: "idle" });

  const draft = (): ProfileDraft => ({
    address: address().trim(),
    apiKey: apiKey().trim() || null,
    displayName: displayName().trim() || null,
    strictTls: false,
  });

  async function connect() {
    if (!address().trim()) return;
    const requestId = newRequestId();
    setPhase({ kind: "connecting", requestId });
    const res = await commands.testConnection(draft(), requestId);
    // Ignore a result that arrives after the user cancelled or started another attempt.
    const current = phase();
    if (current.kind !== "connecting" || current.requestId !== requestId) return;
    if (res.status === "ok") setPhase({ kind: "connected", result: res.data });
    else if (res.error.kind === "cancelled") setPhase({ kind: "idle" });
    else setPhase({ kind: "failed", error: res.error });
  }

  async function cancel() {
    const current = phase();
    if (current.kind !== "connecting") return;
    setPhase({ kind: "idle" });
    await commands.cancelRequest(current.requestId);
  }

  async function connectWithoutKey() {
    setApiKey("");
    await connect();
  }

  async function save(result: TestResult) {
    setPhase({ kind: "saving", result });
    const res = await commands.createProfile(draft());
    if (res.status === "ok") props.onSaved(res.data, result);
    else setPhase({ kind: "failed", error: res.error });
  }

  const busy = () => phase().kind === "connecting" || phase().kind === "saving";
  const failed = () => {
    const p = phase();
    return p.kind === "failed" ? p : undefined;
  };
  const checked = () => {
    const p = phase();
    return p.kind === "connected" || p.kind === "saving" ? p : undefined;
  };

  return (
    <section class="connect-card" aria-labelledby="connect-title">
      <h1 id="connect-title">Connect to Stash</h1>
      <p class="lede">Enter your Stash server's address, and its API key if it has one.</p>

      <form
        onSubmit={(e) => {
          e.preventDefault();
          void connect();
        }}
      >
        <div class="field">
          <label for="connect-address">Server address</label>
          <input
            id="connect-address"
            type="text"
            placeholder="192.168.1.10:9999"
            autocomplete="off"
            spellcheck={false}
            value={address()}
            onInput={(e) => setAddress(e.currentTarget.value)}
            disabled={busy()}
            required
          />
        </div>
        <div class="field">
          <label for="connect-api-key">API key (optional)</label>
          <input
            id="connect-api-key"
            type="text"
            autocomplete="off"
            spellcheck={false}
            value={apiKey()}
            onInput={(e) => setApiKey(e.currentTarget.value)}
            disabled={busy()}
            aria-describedby="connect-api-key-hint"
          />
          <small id="connect-api-key-hint">In Stash: Settings → Security → API Key.</small>
        </div>
        <div class="field">
          <label for="connect-name">Display name (optional)</label>
          <input
            id="connect-name"
            type="text"
            maxLength={64}
            value={displayName()}
            onInput={(e) => setDisplayName(e.currentTarget.value)}
            disabled={busy()}
          />
        </div>

        <div class="actions">
          <Show
            when={phase().kind === "connecting"}
            fallback={
              <button type="submit" class="primary" disabled={busy() || !address().trim()}>
                Connect
              </button>
            }
          >
            <span class="status" role="status">
              Connecting…
            </span>
            <button type="button" onClick={() => void cancel()}>
              Cancel
            </button>
          </Show>
        </div>
      </form>

      <Switch>
        <Match when={failed()}>
          {(p) => {
            const msg = () => appErrorMessage(p().error);
            const isWrongKey = () => failureKind(p().error) === "apiKeyInvalidButNotRequired";
            return (
              <div class="notice error" role="alert">
                <strong>{msg().title}</strong>
                <p>{msg().detail}</p>
                <Show when={msg().hint}>
                  <p class="hint">{msg().hint}</p>
                </Show>
                <Show when={isWrongKey()}>
                  <button type="button" onClick={() => void connectWithoutKey()}>
                    Connect without a key
                  </button>
                </Show>
              </div>
            );
          }}
        </Match>
        <Match when={checked()}>
          {(p) => (
            <div class="notice ok">
              <ServerSummary url={p().result.normalizedUrl} server={p().result.server} />
              <div class="actions">
                <button
                  type="button"
                  class="primary"
                  disabled={p().kind === "saving"}
                  onClick={() => void save(p().result)}
                >
                  Save and continue
                </button>
              </div>
            </div>
          )}
        </Match>
      </Switch>
    </section>
  );
}

/** The connection failure kind inside an AppError, if it is one. */
function failureKind(error: AppError): string | undefined {
  return error.kind === "connect" ? error.failure.kind : undefined;
}
