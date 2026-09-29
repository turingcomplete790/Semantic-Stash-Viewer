import { Match, Show, Switch } from "solid-js";
import SettingsView from "../settings/SettingsView";
import { isRoute } from "../shell/routes";
import type { Route, SettingsPage } from "../shell/routes";
import HomeView from "./HomeView";
import SceneView from "./SceneView";
import ScenesView from "./ScenesView";

export type RouteViewActions = {
  onAddAnother: () => void;
  onRetry: () => void;
  onUpdateKey: () => void;
  onAddServer: () => void;
  onSettingsPage: (page: SettingsPage) => void;
  /** Leave the view in its tab (a scene that stopped playing). */
  onLeave: (fallback: Route) => void;
  /** Close the view's tab (an unavailable view). */
  onClose: () => void;
};

/** Renders the view for one route in one tab (004 research R1). */
export default function RouteView(props: {
  route: Route;
  actions: RouteViewActions;
  /** Restored from disk this session (scene tabs wait for Play). */
  restored?: boolean;
}) {
  return (
    <Show
      when={isRoute(props.route)}
      fallback={<Unavailable onClose={() => props.actions.onClose()} />}
    >
      <Switch>
        <Match when={props.route.kind === "home"}>
          <HomeView
            onAddAnother={props.actions.onAddAnother}
            onRetry={props.actions.onRetry}
            onUpdateKey={props.actions.onUpdateKey}
          />
        </Match>
        <Match when={props.route.kind === "scenes"}>
          <ScenesView />
        </Match>
        <Match when={props.route.kind === "scene" && props.route}>
          {(r) => {
            const scene = r() as Extract<Route, { kind: "scene" }>;
            return (
              <SceneView
                sceneId={scene.sceneId}
                title={scene.title}
                restored={props.restored}
                onDone={() => props.actions.onLeave({ kind: "scenes" })}
              />
            );
          }}
        </Match>
        <Match when={props.route.kind === "settings" && props.route}>
          {(r) => (
            <SettingsView
              page={(r() as Extract<Route, { kind: "settings" }>).page}
              onPage={props.actions.onSettingsPage}
              onAddServer={props.actions.onAddServer}
            />
          )}
        </Match>
      </Switch>
    </Show>
  );
}

function Unavailable(props: { onClose: () => void }) {
  return (
    <div class="view-page">
      <section class="connect-card" role="alert">
        <h1>This view isn't available</h1>
        <p class="lede">It may come from a different version of the viewer.</p>
        <div class="actions">
          <button type="button" onClick={() => props.onClose()}>
            Close
          </button>
        </div>
      </section>
    </div>
  );
}
