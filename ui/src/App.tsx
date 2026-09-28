import { createEffect, createSignal, Match, onCleanup, onMount, Show, Switch } from "solid-js";
import { commands } from "./bindings";
import ConnectionForm from "./components/ConnectionForm";
import KeyPrompt from "./components/KeyPrompt";
import ProfileManager from "./components/ProfileManager";
import ProfilePicker from "./components/ProfilePicker";
import { initPlayer } from "./player/state";
import { newRequestId } from "./lib/requestId";
import { installKeymap } from "./shell/keymap";
import NavBar from "./shell/NavBar";
import Shell from "./shell/Shell";
import { currentRoute, navigate, resetTabs } from "./shell/tabs";
import { connection, initConnection, refreshProfiles } from "./state/connection";
import SettingsView from "./settings/SettingsView";
import RouteView from "./views/RouteView";
import "./App.css";

/**
 * Hosts the app shell (constitution Principle IX) and routes on the core's connection state:
 * - adding a server → connection form;
 * - no active profile → profile picker if servers are saved, otherwise the connection form;
 * - otherwise → the current route's view.
 * The core auto-connects to the last-used profile at launch (001 FR-014); this only follows
 * events.
 */
export default function App() {
  const [adding, setAdding] = createSignal(false);
  const [managerOpen, setManagerOpen] = createSignal(false);
  const [keyPromptOpen, setKeyPromptOpen] = createSignal(false);
  // Settings works without a server (004 FR-006); while disconnected it replaces the picker.
  const [settingsWhileDisconnected, setSettingsWhileDisconnected] = createSignal(false);

  onMount(() => {
    void initConnection();
    void refreshProfiles();
    void initPlayer();
    onCleanup(installKeymap());
  });

  // Debug builds: open a scene straight away (`SSV_DEBUG_OPEN`), to check the video surface.
  let debugOpened = false;
  createEffect(() => {
    if (debugOpened || connection.snapshot().state.kind !== "connected") return;
    debugOpened = true;
    void commands.debugOpenScene().then((id) => {
      if (id) navigate({ kind: "scene", sceneId: id, title: "" });
    });
  });

  // Leaving the server resets navigation (the core stops playback itself, 002 FR-007).
  createEffect(() => {
    if (connection.snapshot().profileId === null) resetTabs();
    else setSettingsWhileDisconnected(false);
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

  const nav = (
    <NavBar
      current={
        noActive()
          ? settingsWhileDisconnected()
            ? { kind: "settings", page: "servers" }
            : null
          : currentRoute()
      }
      onHome={() => {
        setAdding(false);
        if (noActive()) setSettingsWhileDisconnected(false);
        else navigate({ kind: "home" });
      }}
      onNavigate={(route) => {
        setAdding(false);
        navigate(route);
      }}
      onOpenSettings={() => {
        setAdding(false);
        if (noActive()) setSettingsWhileDisconnected(true);
        else navigate({ kind: "settings", page: "servers" });
      }}
      onUpdateKey={() => setKeyPromptOpen(true)}
      onManageServers={() => setManagerOpen(true)}
    />
  );

  return (
    <Shell nav={nav}>
      <Switch>
        <Match when={noActive() && settingsWhileDisconnected()}>
          <SettingsView onManageServers={() => setManagerOpen(true)} />
        </Match>
        <Match when={adding() || (noActive() && !hasProfiles())}>
          <div class="view-page">
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
          </div>
        </Match>
        <Match when={noActive()}>
          <div class="view-page">
            <ProfilePicker
              onConnect={connectTo}
              onAdd={startAdding}
              onManage={() => setManagerOpen(true)}
            />
          </div>
        </Match>
        <Match when={!noActive()}>
          <RouteView
            route={currentRoute()}
            actions={{
              onAddAnother: startAdding,
              onRetry: () => {
                const id = connection.snapshot().profileId;
                if (id) connectTo(id);
              },
              onUpdateKey: () => setKeyPromptOpen(true),
              onManageServers: () => setManagerOpen(true),
              onLeave: navigate,
            }}
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
    </Shell>
  );
}
