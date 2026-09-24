let counter = 0;

/** A unique id for a cancellable request (`cancel_request`). */
export function newRequestId(): string {
  counter += 1;
  return globalThis.crypto?.randomUUID?.() ?? `req-${Date.now()}-${counter}`;
}
