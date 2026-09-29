import type { AppError, ConnectFailure } from "../bindings";

/** A plain-language explanation of a failure (spec FR-005, SC-003). */
export type FailureMessage = {
  title: string;
  detail: string;
  hint?: string;
};

type Kind = ConnectFailure["kind"];
type Of<K extends Kind> = Extract<ConnectFailure, { kind: K }>;

// A Record over every `kind` makes TypeScript reject a missing variant at compile time.
const connectMessages: { [K in Kind]: (f: Of<K>) => FailureMessage } = {
  invalidAddress: (f) => ({
    title: "That address can't be used",
    detail: `The address isn't valid: ${f.reason}.`,
    hint: "Enter something like 192.168.1.10:9999 or https://stash.example.com.",
  }),
  unreachable: (f) => ({
    title: "Nothing answered at this address",
    detail:
      f.tried.length > 1
        ? `Tried ${f.tried.join(" and ")}, but nothing responded.`
        : `Tried ${f.tried[0] ?? "the address"}, but nothing responded.`,
    hint: "Check the address and port, whether Stash is running, and whether it uses http or https.",
  }),
  timeout: () => ({
    title: "The server didn't respond in time",
    detail: "No response arrived within 15 seconds.",
    hint: "Check that the server is reachable from this computer, then try again.",
  }),
  notStash: (f) => ({
    title: "Something answered, but it isn't Stash",
    detail:
      f.status != null
        ? `The server replied with HTTP ${f.status}, not a Stash response.`
        : "The server's reply didn't look like Stash.",
    hint: "Check the port, and remove any extra path from the address.",
  }),
  apiKeyRequired: () => ({
    title: "This server requires an API key",
    detail: "Stash has authentication turned on, so an API key is needed to connect.",
    hint: "In Stash's web UI, open Settings → Security and copy the API key.",
  }),
  apiKeyRejected: () => ({
    title: "The API key was rejected",
    detail: "Stash didn't accept this API key.",
    hint: "The key may have been regenerated. Copy the current one from Settings → Security.",
  }),
  apiKeyInvalidButNotRequired: () => ({
    title: "This server doesn't need an API key, but the one entered is wrong",
    detail: "Stash has no authentication turned on, but it still rejects an incorrect key.",
    hint: "Connect without a key, or fix the key.",
  }),
  unsupportedVersion: (f) => ({
    title: `Stash ${f.found} is older than the minimum supported ${f.minimum}`,
    detail: "This viewer needs a newer version of Stash.",
    hint: `Update Stash to ${f.minimum} or later.`,
  }),
  serverNotReady: (f) => ({
    title: "Stash isn't ready yet",
    detail: `Stash reports status "${f.status}".`,
    hint: "Finish setup or the database migration in Stash's web UI first.",
  }),
  certificateNotVerified: () => ({
    title: "The server's certificate couldn't be verified",
    detail: "Strict certificate checking is on, and this server's certificate isn't trusted.",
    hint: 'Turn off "Verify certificate (strict)" for this server to connect anyway.',
  }),
  duplicateProfile: () => ({
    title: "A profile for this server already exists",
    detail: "You've already saved this server.",
    hint: "Open the existing profile instead.",
  }),
};

export function connectFailureMessage(failure: ConnectFailure): FailureMessage {
  const build = connectMessages[failure.kind] as (f: ConnectFailure) => FailureMessage;
  return build(failure);
}

export function appErrorMessage(error: AppError): FailureMessage {
  switch (error.kind) {
    case "connect":
      return connectFailureMessage(error.failure);
    case "invalidDisplayName":
      return { title: "That name can't be used", detail: `The display name ${error.reason}.` };
    case "sceneNotFound":
      return {
        title: "That scene doesn't exist",
        detail: `The server has no scene with ID ${error.id}.`,
        hint: "Check the ID, or pick a scene from the list.",
      };
    case "tabSetInvalid":
      return {
        title: "Couldn't save your tabs",
        detail: `The tabs couldn't be saved (${error.reason}).`,
      };
    case "openFailed":
      return { title: "Couldn't open the folder", detail: error.detail };
    case "noPlayableFile":
      return {
        title: "This scene has no video file",
        detail: `Scene ${error.id} has no file the player can open.`,
      };
    case "profileNotFound":
      return {
        title: "That profile no longer exists",
        detail: "It may have been deleted in another window.",
      };
    case "unsupportedProfilesVersion":
      return {
        title: "Saved profiles are from a newer version",
        detail: `profiles.json has version ${error.found}, which this version can't read. It was left untouched.`,
        hint: "Update Semantic Stash Viewer.",
      };
    case "storage":
      return { title: "Couldn't save settings", detail: error.message };
    case "notConnected":
      return {
        title: "Can't reach the server",
        detail: "This needs the server. The viewer reconnects on its own as soon as it's back.",
      };
    case "cancelled":
      return { title: "Cancelled", detail: "The connection attempt was cancelled." };
    case "internal":
      return { title: "Something went wrong", detail: error.message };
  }
}
