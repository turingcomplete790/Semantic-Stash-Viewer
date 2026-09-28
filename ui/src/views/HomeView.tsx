import SessionView from "../components/SessionView";
import { navigate } from "../shell/tabs";
import { connection } from "../state/connection";

/** Home: the server summary and connection state (formerly the whole connected screen). */
export default function HomeView(props: {
  onAddAnother: () => void;
  onRetry: () => void;
  onUpdateKey: () => void;
}) {
  return (
    <div class="view-page">
      <SessionView
        profile={connection.activeProfile()}
        onAddAnother={props.onAddAnother}
        onRetry={props.onRetry}
        onUpdateKey={props.onUpdateKey}
        onOpenPlayer={() => navigate({ kind: "scenes" })}
      />
    </div>
  );
}
