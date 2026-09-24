import { createSignal, onMount, Show, type JSX } from "solid-js";
import { commands } from "./bindings";
import ConnectionForm from "./components/ConnectionForm";
import ConnectionIndicator from "./components/ConnectionIndicator";
import KeyPrompt from "./components/KeyPrompt";
import SessionView from "./components/SessionView";
import { newRequestId } from "./lib/requestId";
import { connection, initConnection, refreshProfiles } from "./state/connection";
import "./App.css";

/**
 * Routes on the core's connection state: the connection screen when no profile is active (or
 * the user chose to add another server), otherwise the session view. The core auto-connects to
 * the last-used profile at launch (FR-014); this only follows its events.
 */
export default function App() {
  const [adding, setAdding] = createSignal(false);
  const [keyPromptOpen, setKeyPromptOpen] = createSignal(false);

  onMount(() => {
    void initConnection();
    void refreshProfiles();
  });

  const showForm = () => adding() || connection.snapshot().profileId === null;

  function connectTo(profileId: string) {
    void commands.connect(profileId, newRequestId());
  }

  async function addAnother() {
    await commands.disconnect();
    setAdding(true);
  }

  const indicator = (
    <ConnectionIndicator
      profileName={connection.activeProfile()?.displayName}
      onUpdateKey={() => setKeyPromptOpen(true)}
    />
  );

  return (
    <AppLayout indicator={indicator}>
      <Show
        when={!showForm()}
        fallback={
          <ConnectionForm
            onSaved={(profile) => {
              setAdding(false);
              void refreshProfiles().then(() => connectTo(profile.id));
            }}
          />
        }
      >
        <SessionView
          profile={connection.activeProfile()}
          onAddAnother={() => void addAnother()}
          onRetry={() => {
            const id = connection.snapshot().profileId;
            if (id) connectTo(id);
          }}
          onUpdateKey={() => setKeyPromptOpen(true)}
        />
      </Show>
      <Show when={keyPromptOpen() && connection.activeProfile()}>
        {(profile) => (
          <KeyPrompt
            profile={profile()}
            onDone={() => {
              setKeyPromptOpen(false);
              void refreshProfiles();
            }}
            onCancel={() => setKeyPromptOpen(false)}
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
