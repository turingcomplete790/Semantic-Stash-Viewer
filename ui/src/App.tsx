import { createEffect, createSignal, Match, onMount, Show, Switch, type JSX } from "solid-js";
import { commands } from "./bindings";
import ConnectionForm from "./components/ConnectionForm";
import ConnectionIndicator from "./components/ConnectionIndicator";
import KeyPrompt from "./components/KeyPrompt";
import ProfileManager from "./components/ProfileManager";
import ProfilePicker from "./components/ProfilePicker";
import SessionView from "./components/SessionView";
import PlayerScreen from "./player/PlayerScreen";
import { initPlayer } from "./player/state";
import { newRequestId } from "./lib/requestId";
import { connection, initConnection, refreshProfiles } from "./state/connection";
import "./App.css";

/**
 * Routes on the core's connection state:
 * - adding a server → connection form;
 * - no active profile → profile picker if servers are saved (e.g. after deleting the active
 *   one), otherwise the connection form;
 * - otherwise → the session view.
 * The core auto-connects to the last-used profile at launch (FR-014); this only follows events.
 */
export default function App() {
  const [adding, setAdding] = createSignal(false);
  const [managerOpen, setManagerOpen] = createSignal(false);
  const [keyPromptOpen, setKeyPromptOpen] = createSignal(false);
  const [playerOpen, setPlayerOpen] = createSignal(false);

  onMount(() => {
    void initConnection();
    void refreshProfiles();
    void initPlayer();
  });

  // Leaving the server closes the player screen (the core stops playback itself, FR-007).
  createEffect(() => {
    if (connection.snapshot().profileId === null) setPlayerOpen(false);
  });

  const hasProfiles = () => connection.profiles().length > 0;
  const noActive = () => connection.snapshot().profileId === null;

  function connectTo(profileId: string) {
    setAdding(false);
    void commands.connect(profileId, newRequestId());
  }

  function startAdding() {
    setManagerOpen(false);
    setAdding(true);
  }

  const indicator = (
    <ConnectionIndicator
      profileName={connection.activeProfile()?.displayName}
      onUpdateKey={() => setKeyPromptOpen(true)}
      onManage={() => setManagerOpen(true)}
    />
  );

  return (
    <AppLayout indicator={indicator}>
      <Switch>
        <Match when={playerOpen() && !noActive()}>
          <PlayerScreen onExit={() => setPlayerOpen(false)} />
        </Match>
        <Match when={adding() || (noActive() && !hasProfiles())}>
          <div class="stack">
            <Show when={adding() && hasProfiles()}>
              <button type="button" class="back" onClick={() => setAdding(false)}>
                ← Back
              </button>
            </Show>
            <ConnectionForm
              onSaved={(profile) => void refreshProfiles().then(() => connectTo(profile.id))}
              onOpenExisting={connectTo}
            />
          </div>
        </Match>
        <Match when={noActive()}>
          <ProfilePicker
            onConnect={connectTo}
            onAdd={startAdding}
            onManage={() => setManagerOpen(true)}
          />
        </Match>
        <Match when={!noActive()}>
          <SessionView
            profile={connection.activeProfile()}
            onAddAnother={startAdding}
            onRetry={() => {
              const id = connection.snapshot().profileId;
              if (id) connectTo(id);
            }}
            onUpdateKey={() => setKeyPromptOpen(true)}
            onOpenPlayer={() => setPlayerOpen(true)}
          />
        </Match>
      </Switch>

      <Show when={managerOpen()}>
        <ProfileManager onClose={() => setManagerOpen(false)} onAdd={startAdding} />
      </Show>
      <Show when={keyPromptOpen() && connection.activeProfile()}>
        {(profile) => (
          <KeyPrompt
            profile={profile()}
            onDone={() => setKeyPromptOpen(false)}
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
