import { commands } from "../bindings";
import type { PlayerSnapshot } from "../bindings";
import { stepSpeed } from "./speeds";
import { seekBy } from "./state";

const TEXT_INPUT_TYPES = new Set(["text", "search", "number", "email", "url", "password", "tel"]);

/**
 * True when the event comes from a text field (or a select), where keys must edit, not control
 * playback. Sliders and buttons don't count: on the seek bar, ←/→ should still skip 10 s.
 */
export function isTyping(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  if (target instanceof HTMLInputElement) return TEXT_INPUT_TYPES.has(target.type);
  return (
    target instanceof HTMLTextAreaElement ||
    target instanceof HTMLSelectElement ||
    target.isContentEditable
  );
}

/**
 * The player's keyboard map (FR-009; contracts/player-commands.md "Keyboard map").
 * Returns true (and prevents the default action) when the key was handled.
 */
export function handlePlayerKey(event: KeyboardEvent, s: PlayerSnapshot): boolean {
  if (s.state === "idle" || isTyping(event.target)) return false;
  if (event.ctrlKey || event.altKey || event.metaKey) return false;
  // Rust f64 fields arrive as `number | null` in the bindings.
  const volume = s.volume ?? 100;
  const speed = s.speed ?? 1;

  switch (event.key) {
    case " ":
      void commands.playerTogglePause();
      break;
    case "ArrowLeft":
      seekBy(-10);
      break;
    case "ArrowRight":
      seekBy(10);
      break;
    case "ArrowUp":
      void commands.playerSetVolume(Math.min(100, volume + 5));
      break;
    case "ArrowDown":
      void commands.playerSetVolume(Math.max(0, volume - 5));
      break;
    case "]":
      void commands.playerSetSpeed(stepSpeed(speed, 1));
      break;
    case "[":
      void commands.playerSetSpeed(stepSpeed(speed, -1));
      break;
    case "\\":
      void commands.playerSetSpeed(1);
      break;
    case ".":
      if (!s.paused) return false;
      void commands.playerFrameStep("forward");
      break;
    case ",":
      if (!s.paused) return false;
      void commands.playerFrameStep("back");
      break;
    case "f":
      void commands.playerSetFullscreen(!s.fullscreen);
      break;
    case "m":
      void commands.playerSetMuted(!s.muted);
      break;
    case "Escape":
      if (s.fullscreen) void commands.playerSetFullscreen(false);
      else void commands.playerClose();
      break;
    default:
      return false;
  }
  event.preventDefault();
  return true;
}
