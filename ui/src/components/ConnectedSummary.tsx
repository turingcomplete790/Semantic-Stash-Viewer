import type { SecurityState, ServerInfo } from "../bindings";
import ServerSummary from "./ServerSummary";
import "./ConnectionForm.css";

/** Minimal "connected" view for the MVP: which server, its version, and library counts. */
export default function ConnectedSummary(props: {
  name: string;
  url: string;
  server: ServerInfo;
  security: SecurityState | null;
  onAddAnother: () => void;
}) {
  return (
    <section class="connect-card" aria-labelledby="connected-title">
      <h1 id="connected-title">Connected to {props.name}</h1>
      <p class="lede">The viewer reconnects to this server automatically when it starts.</p>
      <ServerSummary url={props.url} server={props.server} security={props.security} />
      <div class="actions" style={{ "margin-top": "18px" }}>
        <button type="button" onClick={() => props.onAddAnother()}>
          Connect to another server
        </button>
      </div>
    </section>
  );
}
