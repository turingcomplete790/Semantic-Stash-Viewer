import { For, Match, Switch } from "solid-js";
import type { SettingsPage } from "../shell/routes";
import AboutPage from "./AboutPage";
import KeyboardPage from "./KeyboardPage";
import ServersPage from "./ServersPage";
import TroubleshootingPage from "./TroubleshootingPage";
import "../components/ConnectionForm.css";
import "./settings.css";

const PAGES: { id: SettingsPage; label: string }[] = [
  { id: "servers", label: "Servers" },
  { id: "keyboard", label: "Keyboard" },
  { id: "troubleshooting", label: "Troubleshooting" },
  { id: "about", label: "About" },
];

/**
 * Settings (constitution Principle IX; 004 US4): a page list and the page. Later features add
 * pages or entries here rather than separate screens (FR-024).
 */
export default function SettingsView(props: {
  page: SettingsPage;
  onPage: (page: SettingsPage) => void;
  onAddServer: () => void;
}) {
  return (
    <div class="view-page">
      <div class="settings connect-card">
        <nav class="settings-nav" aria-label="Settings pages">
          <h1>Settings</h1>
          <For each={PAGES}>
            {(page) => (
              <button
                type="button"
                classList={{ current: props.page === page.id }}
                aria-current={props.page === page.id ? "page" : undefined}
                onClick={() => props.onPage(page.id)}
              >
                {page.label}
              </button>
            )}
          </For>
        </nav>
        <div class="settings-body">
          <Switch>
            <Match when={props.page === "servers"}>
              <ServersPage onAdd={props.onAddServer} />
            </Match>
            <Match when={props.page === "keyboard"}>
              <KeyboardPage />
            </Match>
            <Match when={props.page === "troubleshooting"}>
              <TroubleshootingPage />
            </Match>
            <Match when={props.page === "about"}>
              <AboutPage />
            </Match>
          </Switch>
        </div>
      </div>
    </div>
  );
}
