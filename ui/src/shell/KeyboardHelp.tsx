import { createSignal, For, onCleanup, onMount, Show } from "solid-js";
import { bindings, register } from "./keymap";
import type { Binding } from "./keymap";

/** Readable key names for listings. */
export function keyLabel(key: string): string {
  return key
    .replace("ArrowLeft", "←")
    .replace("ArrowRight", "→")
    .replace("ArrowUp", "↑")
    .replace("ArrowDown", "↓")
    .replace("PageDown", "PgDn")
    .replace("PageUp", "PgUp");
}

/** Bindings worth listing: described, grouped by scope, in registration order. */
export function listedBindings(): { scope: string; items: Binding[] }[] {
  const listed = bindings().filter((b) => b.description);
  return [
    { scope: "App", items: listed.filter((b) => b.scope === "shell") },
    { scope: "Playing a scene", items: listed.filter((b) => b.scope === "scene") },
  ].filter((g) => g.items.length > 0);
}

/** Keyboard shortcuts, as a table (shared by the `?` overlay and Settings → Keyboard). */
export function ShortcutTable() {
  return (
    <For each={listedBindings()}>
      {(group) => (
        <section class="shortcut-group" aria-label={group.scope}>
          <h3>{group.scope}</h3>
          <dl>
            <For each={group.items}>
              {(b) => (
                <>
                  <dt>
                    <For each={b.keys}>{(k) => <kbd>{keyLabel(k)}</kbd>}</For>
                  </dt>
                  <dd>{b.description}</dd>
                </>
              )}
            </For>
          </dl>
        </section>
      )}
    </For>
  );
}

/** The `?` overlay (004 FR-012). */
export default function KeyboardHelp() {
  const [open, setOpen] = createSignal(false);
  onMount(() => {
    const remove = register({
      id: "help",
      keys: ["?"],
      scope: "shell",
      description: "Show keyboard shortcuts",
      run: () => setOpen((o) => !o),
    });
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape" && open()) {
        e.preventDefault();
        e.stopImmediatePropagation();
        setOpen(false);
      }
    };
    document.addEventListener("keydown", onKey, true);
    onCleanup(() => {
      remove();
      document.removeEventListener("keydown", onKey, true);
    });
  });

  return (
    <Show when={open()}>
      <div class="dialog-backdrop" onClick={() => setOpen(false)}>
        <div
          class="keyboard-help"
          role="dialog"
          aria-label="Keyboard shortcuts"
          onClick={(e) => e.stopPropagation()}
        >
          <div class="pm-header">
            <h2>Keyboard shortcuts</h2>
            <button type="button" onClick={() => setOpen(false)}>
              Close
            </button>
          </div>
          <ShortcutTable />
        </div>
      </div>
    </Show>
  );
}
