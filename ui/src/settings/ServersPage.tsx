import { createSignal, For, Show, untrack } from "solid-js";
import { commands } from "../bindings";
import type { AppError, ProfileSummary } from "../bindings";
import { useFocusTrap } from "../lib/focus";
import { newRequestId } from "../lib/requestId";
import { appErrorMessage } from "../messages/failures";
import { connection } from "../state/connection";
import "../components/ConnectionForm.css";
import "../components/KeyPrompt.css";
import "../components/ProfileManager.css";

/**
 * Settings → Servers (004 FR-025; formerly the server-management dialog from 001 US4): switch,
 * edit, rename, reorder, delete (with confirmation), and add saved servers.
 */
export default function ServersPage(props: { onAdd: () => void }) {
  const [editing, setEditing] = createSignal<string | null>(null);
  const [confirmDelete, setConfirmDelete] = createSignal<ProfileSummary | null>(null);

  const activeId = () => connection.snapshot().profileId;
  let confirmBox: HTMLDivElement | undefined;
  // While the delete confirmation is open, focus stays inside it.
  useFocusTrap(() => (confirmDelete() ? confirmBox : undefined));

  function switchTo(id: string) {
    void commands.connect(id, newRequestId());
  }

  async function move(index: number, delta: number) {
    const ids = connection.profiles().map((p) => p.id);
    const target = index + delta;
    if (target < 0 || target >= ids.length) return;
    [ids[index], ids[target]] = [ids[target], ids[index]];
    await commands.reorderProfiles(ids);
  }

  async function remove(profile: ProfileSummary) {
    setConfirmDelete(null);
    await commands.deleteProfile(profile.id);
  }

  return (
    <section
      class="profile-manager settings-page"
      aria-labelledby="servers-title"
      onKeyDown={(e) => {
        if (e.key !== "Escape") return;
        // Escape backs out one level: confirmation → editor.
        if (confirmDelete()) {
          e.stopPropagation();
          setConfirmDelete(null);
        } else if (editing()) {
          e.stopPropagation();
          setEditing(null);
        }
      }}
    >
      <h2 id="servers-title">Servers</h2>

      <ul class="pm-list">
        <For each={connection.profiles()}>
          {(profile, index) => (
            <li class="pm-row" aria-label={profile.displayName}>
              <Show
                when={editing() === profile.id}
                fallback={
                  <>
                    <div class="pm-info">
                      <div class="pm-name">
                        {profile.displayName}
                        <Show when={activeId() === profile.id}>
                          <span class="badge">Active</span>
                        </Show>
                      </div>
                      <div class="pm-url">{profile.baseUrl}</div>
                      <div class="pm-meta">
                        {profile.strictTls
                          ? "Strict certificate checking"
                          : "Certificates not checked"}
                      </div>
                    </div>
                    <div class="pm-actions">
                      <Show when={activeId() !== profile.id}>
                        <button type="button" class="primary" onClick={() => switchTo(profile.id)}>
                          Switch
                        </button>
                      </Show>
                      <button type="button" onClick={() => setEditing(profile.id)}>
                        Edit
                      </button>
                      <button
                        type="button"
                        aria-label={`Move ${profile.displayName} up`}
                        disabled={index() === 0}
                        onClick={() => void move(index(), -1)}
                      >
                        ↑
                      </button>
                      <button
                        type="button"
                        aria-label={`Move ${profile.displayName} down`}
                        disabled={index() === connection.profiles().length - 1}
                        onClick={() => void move(index(), 1)}
                      >
                        ↓
                      </button>
                      <button
                        type="button"
                        class="danger"
                        onClick={() => setConfirmDelete(profile)}
                      >
                        Delete
                      </button>
                    </div>
                  </>
                }
              >
                <ProfileEditor
                  profile={profile}
                  onDone={() => setEditing(null)}
                  onOpenExisting={switchTo}
                />
              </Show>
            </li>
          )}
        </For>
      </ul>

      <div class="actions">
        <button type="button" class="primary" onClick={() => props.onAdd()}>
          Add server
        </button>
      </div>

      <Show when={confirmDelete()}>
        {(profile) => (
          <div
            ref={(el) => {
              confirmBox = el;
              // Default to the safe choice.
              queueMicrotask(() => el.querySelector<HTMLButtonElement>(".pm-cancel")?.focus());
            }}
            class="pm-confirm"
            role="alertdialog"
            aria-labelledby="pm-confirm-title"
            aria-describedby="pm-confirm-body"
          >
            <strong id="pm-confirm-title">Delete {profile().displayName}?</strong>
            <p id="pm-confirm-body">
              This removes its saved address and API key from this computer. Nothing on the Stash
              server changes.
            </p>
            <div class="actions">
              <button type="button" class="danger" onClick={() => void remove(profile())}>
                Delete
              </button>
              <button type="button" class="pm-cancel" onClick={() => setConfirmDelete(null)}>
                Cancel
              </button>
            </div>
          </div>
        )}
      </Show>
    </section>
  );
}

/** Inline editor for one profile. The check runs first; "Save anyway" forces it (FR-013). */
function ProfileEditor(props: {
  profile: ProfileSummary;
  onDone: () => void;
  onOpenExisting: (id: string) => void;
}) {
  // Initial values only: the fields are edited locally from here.
  const initial = untrack(() => ({ ...props.profile }));
  const [name, setName] = createSignal(initial.displayName);
  const [address, setAddress] = createSignal(initial.baseUrl);
  const [apiKey, setApiKey] = createSignal(initial.apiKey ?? "");
  const [strict, setStrict] = createSignal(initial.strictTls);
  const [saving, setSaving] = createSignal(false);
  const [error, setError] = createSignal<AppError | null>(null);

  async function save(force: boolean) {
    setSaving(true);
    setError(null);
    const res = await commands.updateProfile(
      initial.id,
      {
        displayName: name().trim() || null,
        address: address().trim(),
        apiKey: apiKey().trim() || null,
        strictTls: strict(),
      },
      force,
    );
    setSaving(false);
    if (res.status === "ok") props.onDone();
    else setError(res.error);
  }

  const duplicateOf = () => {
    const e = error();
    return e?.kind === "connect" && e.failure.kind === "duplicateProfile"
      ? e.failure.existingId
      : undefined;
  };
  const canForce = () => {
    const e = error();
    return (
      e?.kind === "connect" &&
      e.failure.kind !== "duplicateProfile" &&
      e.failure.kind !== "invalidAddress"
    );
  };
  const id = (field: string) => `pm-${initial.id}-${field}`;

  return (
    <form
      class="pm-editor"
      onSubmit={(e) => {
        e.preventDefault();
        void save(false);
      }}
    >
      <div class="field">
        <label for={id("name")}>Display name</label>
        <input
          id={id("name")}
          type="text"
          maxLength={64}
          value={name()}
          onInput={(e) => setName(e.currentTarget.value)}
        />
      </div>
      <div class="field">
        <label for={id("address")}>Server address</label>
        <input
          id={id("address")}
          type="text"
          spellcheck={false}
          value={address()}
          onInput={(e) => setAddress(e.currentTarget.value)}
        />
      </div>
      <div class="field">
        <label for={id("key")}>API key</label>
        <input
          id={id("key")}
          type="text"
          autocomplete="off"
          spellcheck={false}
          value={apiKey()}
          onInput={(e) => setApiKey(e.currentTarget.value)}
        />
      </div>
      <div class="field field-check">
        <label>
          <input
            type="checkbox"
            checked={strict()}
            onChange={(e) => setStrict(e.currentTarget.checked)}
          />
          Verify certificate (strict)
        </label>
      </div>

      <Show when={error()}>
        {(err) => (
          <div class="notice error" role="alert">
            <strong>{appErrorMessage(err()).title}</strong>
            <p>{appErrorMessage(err()).detail}</p>
            <Show when={duplicateOf()}>
              {(existing) => (
                <button type="button" onClick={() => props.onOpenExisting(existing())}>
                  Open existing
                </button>
              )}
            </Show>
            <Show when={canForce()}>
              <button type="button" onClick={() => void save(true)} disabled={saving()}>
                Save anyway
              </button>
            </Show>
          </div>
        )}
      </Show>

      <div class="actions">
        <button type="submit" class="primary" disabled={saving()}>
          {saving() ? "Checking…" : "Save"}
        </button>
        <button type="button" onClick={() => props.onDone()} disabled={saving()}>
          Cancel
        </button>
      </div>
    </form>
  );
}
