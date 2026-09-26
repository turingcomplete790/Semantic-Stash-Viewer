import type { SecurityState } from "../bindings";

/** Labels and explanations for the connection security state (FR-020). */
export const securityText: Record<SecurityState, { label: string; detail: string }> = {
  unencrypted: {
    label: "Unencrypted",
    detail: "Plain http. Traffic, including the API key, isn't encrypted.",
  },
  encryptedUnverified: {
    label: "Encrypted, not verified",
    detail:
      "https with certificate checking off, so the server's identity wasn't checked. Turn on strict checking for this server to verify it.",
  },
  encryptedVerified: {
    label: "Encrypted, verified",
    detail: "https, and the server's certificate was verified.",
  },
};
