import { onCleanup, onMount, Show } from "solid-js";
import type { SecurityState } from "../bindings";
import { securityText } from "../messages/security";
import "./ConnectionDetails.css";

/** Popover from the indicator: address, security state, and strict setting (FR-021). */
export default function ConnectionDetails(props: {
  url: string;
  security: SecurityState | null;
  strictTls: boolean;
  onClose: () => void;
}) {
  let panel: HTMLDivElement | undefined;

  const onKey = (e: KeyboardEvent) => {
    if (e.key === "Escape") props.onClose();
  };
  const onPointer = (e: PointerEvent) => {
    if (panel && !panel.contains(e.target as Node)) props.onClose();
  };
  onMount(() => {
    document.addEventListener("keydown", onKey);
    // Defer so the click that opened the popover doesn't immediately close it.
    setTimeout(() => document.addEventListener("pointerdown", onPointer), 0);
  });
  onCleanup(() => {
    document.removeEventListener("keydown", onKey);
    document.removeEventListener("pointerdown", onPointer);
  });

  return (
    <div ref={panel} class="conn-details" role="dialog" aria-label="Connection details">
      <dl>
        <dt>Server</dt>
        <dd>{props.url || "—"}</dd>
        <dt>Security</dt>
        <dd>
          <Show when={props.security} fallback="Not known until connected">
            {(sec) => (
              <>
                <strong>{securityText[sec()].label}</strong>
                <p>{securityText[sec()].detail}</p>
              </>
            )}
          </Show>
        </dd>
        <dt>Strict certificate checking</dt>
        <dd>{props.strictTls ? "On" : "Off"}</dd>
      </dl>
    </div>
  );
}
