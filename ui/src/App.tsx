import { createSignal, Show, type JSX } from "solid-js";
import type { ProfileSummary, TestResult } from "./bindings";
import ConnectedSummary from "./components/ConnectedSummary";
import ConnectionForm from "./components/ConnectionForm";
import "./App.css";

type Session = { profile: ProfileSummary; result: TestResult };

/**
 * Routes between the connection screen (no active profile) and the connected view.
 * US2 replaces this local session with the core's connection state and auto-connect.
 */
export default function App() {
  const [session, setSession] = createSignal<Session | null>(null);

  return (
    <AppLayout indicator={null}>
      <Show
        when={session()}
        fallback={<ConnectionForm onSaved={(profile, result) => setSession({ profile, result })} />}
      >
        {(s) => (
          <ConnectedSummary
            profile={s().profile}
            result={s().result}
            onAddAnother={() => setSession(null)}
          />
        )}
      </Show>
    </AppLayout>
  );
}

export function AppLayout(props: { indicator: JSX.Element; children: JSX.Element }) {
  return (
    <div class="app">
      <header class="topbar">
        <span class="brand">Semantic Stash Viewer</span>
        <div class="topbar-indicator">{props.indicator}</div>
      </header>
      <main class="main">{props.children}</main>
    </div>
  );
}
