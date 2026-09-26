import { For } from "solid-js";
import { connection } from "../state/connection";
import "./ConnectionForm.css";
import "./ProfileManager.css";

/**
 * Shown when no profile is active but saved ones exist, e.g. after deleting the active profile
 * (US4 AS3).
 */
export default function ProfilePicker(props: {
  onConnect: (id: string) => void;
  onAdd: () => void;
  onManage: () => void;
}) {
  return (
    <section class="connect-card" aria-labelledby="picker-title">
      <h1 id="picker-title">Choose a Stash server</h1>
      <p class="lede">Pick one of your saved servers, or add a new one.</p>
      <ul class="pm-list">
        <For each={connection.profiles()}>
          {(profile) => (
            <li class="pm-row" aria-label={profile.displayName}>
              <div class="pm-info">
                <div class="pm-name">{profile.displayName}</div>
                <div class="pm-url">{profile.baseUrl}</div>
              </div>
              <div class="pm-actions">
                <button type="button" class="primary" onClick={() => props.onConnect(profile.id)}>
                  Connect
                </button>
              </div>
            </li>
          )}
        </For>
      </ul>
      <div class="actions">
        <button type="button" onClick={() => props.onAdd()}>
          Add server
        </button>
        <button type="button" onClick={() => props.onManage()}>
          Manage servers
        </button>
      </div>
    </section>
  );
}
