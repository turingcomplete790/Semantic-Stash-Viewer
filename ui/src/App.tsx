import { createEffect, createSignal, Match, onCleanup, onMount, Show, Switch } from "solid-js";
import { commands } from "./bindings";
import ConnectionForm from "./components/ConnectionForm";
import KeyPrompt from "./components/KeyPrompt";
import ProfilePicker from "./components/ProfilePicker";
import { initPlayer } from "./player/state";
import { newRequestId } from "./lib/requestId";
import KeyboardHelp from "./shell/KeyboardHelp";
import { installKeymap, register } from "./shell/keymap";
import NavBar from "./shell/NavBar";
import NowPlayingBar from "./shell/NowPlayingBar";
import Shell from "./shell/Shell";
import TabPanes from "./shell/TabPanes";
import TabStrip from "./shell/TabStrip";
import {
  close,
  currentRoute,
  installMouseNavigation,
  loadTabsFor,
  navigate,
  navigateIn,
  registerTabShortcuts,
  resetTabs,
  wasRestored,
} from "./shell/tabs";
import { connection, initConnection, refreshProfiles } from "./state/connection";
import { initNotifications } from "./state/notifications";
import type { SettingsPage } from "./shell/routes";
import Toasts from "./shell/Toasts";
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
  const [keyPromptOpen, setKeyPromptOpen] = createSignal(false);
  // Settings works without a server (004 FR-006); while disconnected it replaces the picker and
  // keeps its page here (connected, the page lives in the tab's route).
  const [settingsWhileDisconnected, setSettingsWhileDisconnected] = createSignal(false);
  const [disconnectedPage, setDisconnectedPage] = createSignal<SettingsPage>("servers");

  /** Open Settings on a page: in the current tab when connected, in place of the picker if not. */
  function openSettings(page: SettingsPage = "servers") {
    setAdding(false);
    if (noActive()) {
      setDisconnectedPage(page);
      setSettingsWhileDisconnected(true);
    } else navigate({ kind: "settings", page });
  }

  onMount(() => {
    void initConnection();
    void refreshProfiles();
    void initPlayer();
    void initNotifications();
    onCleanup(installKeymap());
    onCleanup(registerTabShortcuts());
    onCleanup(installMouseNavigation());
    onCleanup(registerPlayerKeyListing());
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

  // Each server has its own tabs (004 FR-014): switching saves the old set and loads the new one;
  // leaving the server resets to one Home tab. The core stops playback itself (002 FR-007).
  let tabsProfile: string | null = null;
  createEffect(() => {
    const id = connection.snapshot().profileId;
    if (id === tabsProfile) return;
    tabsProfile = id;
    if (id === null) resetTabs();
    else {
      setSettingsWhileDisconnected(false);
      void loadTabsFor(id);
    }
  });

  const hasProfiles = () => connection.profiles().length > 0;
  const noActive = () => connection.snapshot().profileId === null;

  function connectTo(profileId: string) {
    setAdding(false);
    void commands.connect(profileId, newRequestId());
  }

  function startAdding() {
    setSettingsWhileDisconnected(false);
    setAdding(true);
  }

  const nav = (
    <NavBar
      current={
        noActive()
          ? settingsWhileDisconnected()
            ? { kind: "settings", page: disconnectedPage() }
            : null
          : currentRoute()
      }
      onHome={() => {
        setAdding(false);
        if (noActive()) setSettingsWhileDisconnected(false);
        else navigate({ kind: "home" });
      }}
      onNavigate={(route, options) => {
        setAdding(false);
        navigate(route, options);
      }}
      onOpenSettings={() => openSettings()}
      onUpdateKey={() => setKeyPromptOpen(true)}
      onManageServers={() => openSettings("servers")}
    />
  );

  return (
    <Shell
      nav={nav}
      tabs={noActive() || adding() ? undefined : <TabStrip />}
      nowPlaying={<NowPlayingBar />}
      toasts={<Toasts />}
    >
      <Switch>
        <Match when={noActive() && settingsWhileDisconnected()}>
          <SettingsView
            page={disconnectedPage()}
            onPage={setDisconnectedPage}
            onAddServer={startAdding}
          />
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
              onManage={() => openSettings("servers")}
            />
          </div>
        </Match>
        <Match when={!noActive()}>
          <TabPanes
            render={(route, tabId) => (
              <RouteView
                route={route}
                restored={wasRestored(tabId)}
                actions={{
                  onAddAnother: startAdding,
                  onRetry: () => {
                    const id = connection.snapshot().profileId;
                    if (id) connectTo(id);
                  },
                  onUpdateKey: () => setKeyPromptOpen(true),
                  onAddServer: startAdding,
                  onSettingsPage: (page) => navigateIn(tabId, { kind: "settings", page }),
                  onLeave: (fallback) => navigateIn(tabId, fallback),
                  onClose: () => close(tabId),
                }}
              />
            )}
          />
        </Match>
      </Switch>

      <KeyboardHelp />
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

/** List the player's keys in the `?` overlay and Settings → Keyboard (handled by the player). */
function registerPlayerKeyListing(): () => void {
  const keys: [string, string[], string][] = [
    ["player:pause", ["Space"], "Play or pause"],
    ["player:seek", ["ArrowLeft", "ArrowRight"], "Back or forward 10 seconds"],
    ["player:volume", ["ArrowUp", "ArrowDown"], "Volume up or down"],
    ["player:speed", ["[", "]"], "Slower or faster"],
    ["player:speed-reset", ["\\"], "Normal speed"],
    ["player:frame", [",", "."], "Previous or next frame (paused)"],
    ["player:fullscreen", ["F"], "Fullscreen"],
    ["player:mute", ["M"], "Mute"],
    ["player:escape", ["Escape"], "Leave fullscreen, then close the player"],
  ];
  const removers = keys.map(([id, k, description]) =>
    register({ id, keys: k, scope: "scene", description }),
  );
  return () => removers.forEach((remove) => remove());
}
