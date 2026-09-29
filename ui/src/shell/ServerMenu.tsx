import { createSignal, For, onCleanup, Show } from "solid-js";
import { commands } from "../bindings";
import { newRequestId } from "../lib/requestId";
import { connection } from "../state/connection";
import { ServerIcon } from "./icons";

/**
 * The current server in the navigation bar (004 FR-005): switch to another saved server,
 * update the API key, or open server management.
 */
export default function ServerMenu(props: {
  onUpdateKey: () => void;
  onManageServers: () => void;
}) {
  const [open, setOpen] = createSignal(false);
  let root: HTMLDivElement | undefined;

  const active = () => connection.activeProfile();
  const others = () => connection.profiles().filter((p) => p.id !== active()?.id);
  const label = () => active()?.displayName ?? "No server";

  const onDocClick = (e: MouseEvent) => {
    if (root && !root.contains(e.target as Node)) setOpen(false);
  };
  document.addEventListener("mousedown", onDocClick);
  onCleanup(() => document.removeEventListener("mousedown", onDocClick));

  function choose(action: () => void) {
    setOpen(false);
    action();
  }

  return (
    <div class="nav-server" ref={root}>
      <button
        type="button"
        class="nav-icon-button nav-server-button"
        aria-label={`Server: ${label()}`}
        aria-haspopup="menu"
        aria-expanded={open()}
        onClick={() => setOpen((o) => !o)}
        onKeyDown={(e) => {
          if (e.key === "Escape") setOpen(false);
        }}
      >
        <ServerIcon />
        <span class="nav-server-name">{label()}</span>
      </button>
      <Show when={open()}>
        <div
          class="nav-menu"
          role="menu"
          onKeyDown={(e) => {
            if (e.key === "Escape") setOpen(false);
          }}
        >
          <For each={others()}>
            {(profile) => (
              <button
                type="button"
                role="menuitem"
                onClick={() => choose(() => void commands.connect(profile.id, newRequestId()))}
              >
                Switch to {profile.displayName}
              </button>
            )}
          </For>
          <Show when={active()}>
            <button type="button" role="menuitem" onClick={() => choose(props.onUpdateKey)}>
              Update API key
            </button>
          </Show>
          <button type="button" role="menuitem" onClick={() => choose(props.onManageServers)}>
            Manage servers
          </button>
        </div>
      </Show>
    </div>
  );
}
