import { createResource, createSignal, Show } from "solid-js";
import { commands } from "../bindings";
import type { AppError } from "../bindings";
import { appErrorMessage } from "../messages/failures";

/**
 * Settings → Troubleshooting (004 FR-027). Things users only need when something's wrong;
 * infrastructure stays quiet everywhere else (constitution Principle IX).
 */
/** Bytes as "512 KB", "12.4 MB", "1.2 GB". */
export function formatBytes(bytes: number): string {
  const units = ["bytes", "KB", "MB", "GB", "TB"];
  let value = Math.max(0, bytes);
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  if (unit === 0) return `${Math.round(value)} bytes`;
  return `${value >= 100 ? Math.round(value) : value.toFixed(1)} ${units[unit]}`;
}

export default function TroubleshootingPage() {
  const [error, setError] = createSignal<AppError | null>(null);
  const [cacheSize, { refetch: reloadCacheSize }] = createResource(
    async () => (await commands.cacheSize()) ?? 0,
  );
  const [freed, setFreed] = createSignal<number | null>(null);

  // No confirmation: clearing only costs a reload from the server (003 FR-005).
  async function clearCache() {
    setError(null);
    setFreed(null);
    const res = await commands.clearCache();
    if (res.status === "error") {
      setError(res.error);
      return;
    }
    setFreed(res.data ?? 0);
    void reloadCacheSize();
  }

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

      <div class="settings-item">
        <div>
          <strong>Cache</strong>
          <p class="lede">
            Saved copies of screens so they open instantly
            <Show when={cacheSize.latest !== undefined}>
              {" "}
              ({formatBytes(cacheSize.latest ?? 0)})
            </Show>
            . Clearing is only needed if something looks out of date.
          </p>
          <Show when={freed() !== null}>
            <p class="lede" role="status">
              Freed {formatBytes(freed() ?? 0)}
            </p>
          </Show>
        </div>
        <button type="button" onClick={() => void clearCache()}>
          Clear cache
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
    </section>
  );
}
