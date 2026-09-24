import { describe, expect, it } from "vitest";
import type { AppError, ConnectFailure } from "../bindings";
import { appErrorMessage, connectFailureMessage } from "../messages/failures";

// One sample of every ConnectFailure variant. `satisfies` + the Record below make TypeScript
// fail if a variant is added to the bindings without a sample here.
const samples: { [K in ConnectFailure["kind"]]: Extract<ConnectFailure, { kind: K }> } = {
  invalidAddress: { kind: "invalidAddress", reason: "missing host" },
  unreachable: { kind: "unreachable", tried: ["https://x:1", "http://x:1"] },
  timeout: { kind: "timeout" },
  notStash: { kind: "notStash", status: 404 },
  apiKeyRequired: { kind: "apiKeyRequired" },
  apiKeyRejected: { kind: "apiKeyRejected" },
  apiKeyInvalidButNotRequired: { kind: "apiKeyInvalidButNotRequired" },
  unsupportedVersion: { kind: "unsupportedVersion", found: "v0.30.1", minimum: "v0.31.1" },
  serverNotReady: { kind: "serverNotReady", status: "NEEDS_MIGRATION" },
  certificateNotVerified: { kind: "certificateNotVerified" },
  duplicateProfile: { kind: "duplicateProfile", existingId: "abc" },
};

describe("connect failure messages (SC-003)", () => {
  it.each(Object.values(samples))("$kind has a distinct plain-language message", (failure) => {
    const msg = connectFailureMessage(failure);
    expect(msg.title.length).toBeGreaterThan(0);
    expect(msg.detail.length).toBeGreaterThan(0);
  });

  it("gives every variant a distinct title", () => {
    const titles = Object.values(samples).map((f) => connectFailureMessage(f).title);
    expect(new Set(titles).size).toBe(titles.length);
  });

  it("names both versions for unsupportedVersion", () => {
    expect(connectFailureMessage(samples.unsupportedVersion).title).toBe(
      "Stash v0.30.1 is older than the minimum supported v0.31.1",
    );
  });

  it("explains how to turn strict checking off", () => {
    expect(connectFailureMessage(samples.certificateNotVerified).hint).toMatch(
      /Verify certificate/,
    );
  });
});

describe("app error messages", () => {
  const errors: AppError[] = [
    { kind: "connect", failure: samples.timeout },
    { kind: "invalidDisplayName", reason: "must be at most 64 characters" },
    { kind: "profileNotFound", id: "x" },
    { kind: "unsupportedProfilesVersion", found: 2 },
    { kind: "storage", message: "disk full" },
    { kind: "cancelled" },
    { kind: "internal", message: "boom" },
  ];
  it.each(errors)("$kind has a message", (error) => {
    expect(appErrorMessage(error).title.length).toBeGreaterThan(0);
  });
});
