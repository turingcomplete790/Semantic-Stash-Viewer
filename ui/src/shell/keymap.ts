import { createSignal } from "solid-js";
import { isTyping } from "../player/keyboard";

/**
 * One registry for every keyboard shortcut (004 research R7). It drives dispatch, the `?`
 * overlay, and Settings → Keyboard, so they can't disagree.
 *
 * Key notation:
 * - chords: `Ctrl+T`, `Ctrl+Shift+T`, `Alt+ArrowLeft`, `Ctrl+PageDown` (letters upper-case);
 * - sequences: `g s` (two keys within 1 s);
 * - single keys: `?`.
 *
 * Sequences and single keys are ignored while a text field has focus; chords still work.
 * `scene` bindings only fire while a scene view is active. `scenes-grid` entries are listings
 * only: the scene grid handles its own keys while it has focus (005 research R8).
 */
export type Scope = "shell" | "scene" | "scenes-grid";

export type Binding = {
  id: string;
  /** Alternatives; the first is shown in listings. */
  keys: string[];
  scope: Scope;
  description: string;
  /** Omit for display-only entries handled by a raw handler (the player's map). */
  run?: () => void;
};

/** A handler that decides for itself (the player's keyboard map). Returns true if handled. */
export type RawHandler = (event: KeyboardEvent) => boolean;

const SEQUENCE_TIMEOUT_MS = 1000;

const [registered, setRegistered] = createSignal<Binding[]>([]);
const rawHandlers: Record<Scope, RawHandler[]> = { shell: [], scene: [], "scenes-grid": [] };
let sceneActive: () => boolean = () => false;
let pendingPrefix: { key: string; at: number } | null = null;

/** Every registered binding, for listings. Reactive. */
export function bindings(): Binding[] {
  return registered();
}

/** Register a binding; returns a function that removes it. Re-registering an id replaces it. */
export function register(binding: Binding): () => void {
  setRegistered((list) => [...list.filter((b) => b.id !== binding.id), binding]);
  return () => setRegistered((list) => list.filter((b) => b !== binding));
}

export function registerRawHandler(scope: Scope, handler: RawHandler): () => void {
  rawHandlers[scope].push(handler);
  return () => {
    rawHandlers[scope] = rawHandlers[scope].filter((h) => h !== handler);
  };
}

/** Tell the keymap how to know whether a scene view is active. */
export function setSceneActive(check: () => boolean): void {
  sceneActive = check;
}

function active(scope: Scope): boolean {
  return scope === "shell" || (scope === "scene" && sceneActive());
}

function keyName(event: KeyboardEvent): string {
  return event.key.length === 1 ? event.key.toUpperCase() : event.key;
}

/** `Ctrl+Shift+T` style name for an event with Ctrl, Alt, or Meta held. */
export function chordName(event: KeyboardEvent): string {
  const parts: string[] = [];
  if (event.ctrlKey) parts.push("Ctrl");
  if (event.altKey) parts.push("Alt");
  if (event.metaKey) parts.push("Meta");
  if (event.shiftKey) parts.push("Shift");
  parts.push(keyName(event));
  return parts.join("+");
}

function find(key: string): Binding | undefined {
  return registered().find((b) => b.run && active(b.scope) && b.keys.includes(key));
}

function run(binding: Binding, event: KeyboardEvent): true {
  event.preventDefault();
  binding.run?.();
  return true;
}

/** Dispatch one keydown. Returns true if a binding handled it. Exported for tests. */
export function dispatch(event: KeyboardEvent): boolean {
  if (event.defaultPrevented) return false;

  if (event.ctrlKey || event.altKey || event.metaKey) {
    const binding = find(chordName(event));
    return binding ? run(binding, event) : false;
  }
  if (isTyping(event.target)) {
    pendingPrefix = null;
    return false;
  }

  const key = event.key;
  if (pendingPrefix && Date.now() - pendingPrefix.at <= SEQUENCE_TIMEOUT_MS) {
    const binding = find(`${pendingPrefix.key} ${key}`);
    pendingPrefix = null;
    if (binding) return run(binding, event);
  }
  pendingPrefix = null;
  const startsSequence = registered().some(
    (b) => b.run && active(b.scope) && b.keys.some((k) => k.startsWith(`${key} `)),
  );
  if (startsSequence) {
    pendingPrefix = { key, at: Date.now() };
    return true;
  }

  const single = find(key);
  if (single) return run(single, event);

  for (const scope of ["scene", "shell"] as const) {
    if (!active(scope)) continue;
    for (const handler of rawHandlers[scope]) if (handler(event)) return true;
  }
  return false;
}

/** Install the single document listener; returns an uninstaller. */
export function installKeymap(): () => void {
  const listener = (e: KeyboardEvent) => void dispatch(e);
  document.addEventListener("keydown", listener);
  return () => document.removeEventListener("keydown", listener);
}

/** Clear everything (tests). */
export function resetKeymap(): void {
  setRegistered([]);
  rawHandlers.shell = [];
  rawHandlers.scene = [];
  rawHandlers["scenes-grid"] = [];
  sceneActive = () => false;
  pendingPrefix = null;
}
