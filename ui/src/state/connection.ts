import { createSignal } from "solid-js";
import { commands, events } from "../bindings";
import type { ConnectionSnapshot, ProfileSummary } from "../bindings";

const idle: ConnectionSnapshot = {
  profileId: null,
  state: { kind: "idle" },
  security: null,
  finalUrl: null,
  server: null,
  lastContactAt: null,
};

const [snapshot, setSnapshot] = createSignal<ConnectionSnapshot>(idle);
const [profiles, setProfiles] = createSignal<ProfileSummary[]>([]);

/** Connection state from the core, plus the saved profiles. Shared by all components. */
export const connection = {
  snapshot,
  profiles,
  /** The profile the session is for, if any. */
  activeProfile: (): ProfileSummary | undefined => {
    const id = snapshot().profileId;
    return id ? profiles().find((p) => p.id === id) : undefined;
  },
};

let unlisten: (() => void) | undefined;
let unlistenProfiles: (() => void) | undefined;

/**
 * Subscribe to `connection-state` and `profiles-changed`, then hydrate with `getConnectionSnapshot()`. Events that
 * arrive while hydrating win over the (older) hydrated snapshot.
 */
export async function initConnection(): Promise<void> {
  unlisten?.();
  unlistenProfiles?.();
  unlistenProfiles = await events.profilesChanged.listen((e) => setProfiles(e.payload));
  let sawEvent = false;
  unlisten = await events.connectionState.listen((e) => {
    sawEvent = true;
    setSnapshot(e.payload);
  });
  const current = await commands.getConnectionSnapshot();
  if (!sawEvent) setSnapshot(current);
}

/** Reload the saved profiles (after create/update). */
export async function refreshProfiles(): Promise<void> {
  const res = await commands.listProfiles();
  if (res.status === "ok") setProfiles(res.data);
}
