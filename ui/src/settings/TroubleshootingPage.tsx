import { createSignal, Show } from "solid-js";
import { commands } from "../bindings";
import type { AppError } from "../bindings";
import { appErrorMessage } from "../messages/failures";

/**
 * Settings → Troubleshooting (004 FR-027). Things users only need when something's wrong;
 * infrastructure stays quiet everywhere else (constitution Principle IX).
 */
export default function TroubleshootingPage() {
  const [error, setError] = createSignal<AppError | null>(null);

  async function openLogs() {
    setError(null);
    const res = await commands.openLogFolder();
    if (res.status === "error") setError(res.error);
  }

  return (
    <section class="settings-page" aria-labelledby="troubleshooting-title">
      <h2 id="troubleshooting-title">Troubleshooting</h2>

      <div class="settings-item">
        <div>
          <strong>Logs</strong>
          <p class="lede">The viewer's log files, useful when reporting a problem.</p>
        </div>
        <button type="button" onClick={() => void openLogs()}>
          Open log folder
        </button>
      </div>
      <Show when={error()}>
        {(err) => (
          <div class="notice error" role="alert">
            <strong>{appErrorMessage(err()).title}</strong>
            <p>{appErrorMessage(err()).detail}</p>
          </div>
        )}
      </Show>

      {/* Feature 003 adds "Clear cache" here (003 FR-005). */}
    </section>
  );
}
