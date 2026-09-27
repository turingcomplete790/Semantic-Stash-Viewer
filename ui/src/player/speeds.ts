/** Playback speeds offered in the menu and stepped through by `[` / `]` (FR-008). */
export const SPEEDS = [0.25, 0.5, 0.75, 1, 1.25, 1.5, 2, 3, 4] as const;

/** The next speed in the list above (`+1`) or below (`-1`) the current one, clamped. */
export function stepSpeed(current: number, direction: 1 | -1): number {
  if (direction > 0) return SPEEDS.find((s) => s > current + 1e-9) ?? SPEEDS[SPEEDS.length - 1];
  return [...SPEEDS].reverse().find((s) => s < current - 1e-9) ?? SPEEDS[0];
}
