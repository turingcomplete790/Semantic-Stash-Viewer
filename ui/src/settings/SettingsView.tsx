import "../components/ConnectionForm.css";

/**
 * Settings (004 US4). Foundational placeholder: server management still opens the existing
 * dialog; US4 turns this into the Servers / Keyboard / Troubleshooting / About pages.
 */
export default function SettingsView(props: { onManageServers: () => void }) {
  return (
    <div class="view-page">
      <section class="connect-card" aria-labelledby="settings-title">
        <h1 id="settings-title">Settings</h1>
        <div class="actions">
          <button type="button" onClick={() => props.onManageServers()}>
            Manage servers
          </button>
        </div>
      </section>
    </div>
  );
}
