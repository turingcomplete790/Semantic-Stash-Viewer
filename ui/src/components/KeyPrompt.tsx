import { createSignal, onMount, Show, untrack } from "solid-js";
import { commands } from "../bindings";
import type { AppError, ProfileSummary } from "../bindings";
import { useFocusTrap } from "../lib/focus";
import { appErrorMessage } from "../messages/failures";
import "./ConnectionForm.css";
import "./KeyPrompt.css";

/**
 * "Update key" dialog shown when the saved key is rejected (US2 AS4). Saving calls
 * `update_profile`, which re-checks the key and, because this profile is the active one, makes
 * the core reconnect with it.
 */
export default function KeyPrompt(props: {
  profile: ProfileSummary;
  onDone: (updated: ProfileSummary) => void;
  onCancel: () => void;
}) {
  // Initial value only: the field is edited locally from here.
  const [key, setKey] = createSignal(untrack(() => props.profile.apiKey) ?? "");
  const [saving, setSaving] = createSignal(false);
  const [error, setError] = createSignal<AppError | null>(null);
  let input: HTMLInputElement | undefined;
  let dialog: HTMLDivElement | undefined;

  onMount(() => input?.focus());
  useFocusTrap(() => dialog);

  async function save() {
    setSaving(true);
    setError(null);
    const res = await commands.updateProfile(
      props.profile.id,
      {
        address: props.profile.baseUrl,
        apiKey: key().trim() || null,
        displayName: props.profile.displayName,
        strictTls: props.profile.strictTls,
      },
      false,
    );
    setSaving(false);
    if (res.status === "ok") props.onDone(res.data);
    else setError(res.error);
  }

  return (
    <div class="modal-backdrop">
      <div
        ref={dialog}
        class="modal connect-card"
        role="dialog"
        aria-modal="true"
        aria-labelledby="key-prompt-title"
        onKeyDown={(e) => {
          if (e.key === "Escape") props.onCancel();
        }}
      >
        <h2 id="key-prompt-title">Update API key</h2>
        <p class="lede">
          {props.profile.displayName} rejected the saved key. Paste the current key from Stash's
          Settings → Security.
        </p>
        <form
          onSubmit={(e) => {
            e.preventDefault();
            void save();
          }}
        >
          <div class="field">
            <label for="key-prompt-input">API key</label>
            <input
              id="key-prompt-input"
              ref={input}
              type="text"
              autocomplete="off"
              spellcheck={false}
              value={key()}
              onInput={(e) => setKey(e.currentTarget.value)}
              disabled={saving()}
            />
          </div>
          <Show when={error()}>
            {(err) => (
              <div class="notice error" role="alert">
                <strong>{appErrorMessage(err()).title}</strong>
                <p>{appErrorMessage(err()).detail}</p>
              </div>
            )}
          </Show>
          <div class="actions">
            <button type="submit" class="primary" disabled={saving()}>
              {saving() ? "Checking…" : "Save and reconnect"}
            </button>
            <button type="button" onClick={() => props.onCancel()} disabled={saving()}>
              Cancel
            </button>
          </div>
        </form>
      </div>
    </div>
  );
}
