import type { ProfileSummary, TestResult } from "../bindings";
import ServerSummary from "./ServerSummary";
import "./ConnectionForm.css";

/** Minimal "connected" view for the MVP: which server, its version, and library counts. */
export default function ConnectedSummary(props: {
  profile: ProfileSummary;
  result: TestResult;
  onAddAnother: () => void;
}) {
  return (
    <section class="connect-card" aria-labelledby="connected-title">
      <h1 id="connected-title">Connected to {props.profile.displayName}</h1>
      <p class="lede">Saved. The viewer will use this server next time you open it.</p>
      <ServerSummary url={props.result.normalizedUrl} server={props.result.server} />
      <div class="actions" style={{ "margin-top": "18px" }}>
        <button type="button" onClick={() => props.onAddAnother()}>
          Connect to another server
        </button>
      </div>
    </section>
  );
}
