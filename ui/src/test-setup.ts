import "@testing-library/jest-dom/vitest";

// jsdom has no ResizeObserver (WebKit does). A no-op stand-in; tests that need resizes stub
// their own with vi.stubGlobal.
if (typeof globalThis.ResizeObserver === "undefined") {
  globalThis.ResizeObserver = class {
    observe() {}
    unobserve() {}
    disconnect() {}
  } as unknown as typeof ResizeObserver;
}
