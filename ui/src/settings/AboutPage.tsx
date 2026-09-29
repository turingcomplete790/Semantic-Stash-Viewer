import { createResource, Show } from "solid-js";
import { commands } from "../bindings";
import { connection } from "../state/connection";

/** Settings → About (004 FR-028): the viewer's version and the connected server. */
export default function AboutPage() {
  const [info] = createResource(() => commands.appInfo());
  const snap = () => connection.snapshot();
  const address = () => snap().finalUrl ?? connection.activeProfile()?.baseUrl;

  return (
    <section class="settings-page" aria-labelledby="about-title">
      <h2 id="about-title">About</h2>
      <dl class="about-list">
        <dt>Semantic Stash Viewer</dt>
        <dd>{info()?.version ?? "…"}</dd>
        <Show when={address()}>
          {(url) => (
            <>
              <dt>Server</dt>
              <dd>{url()}</dd>
            </>
          )}
        </Show>
        <Show when={snap().server}>
          {(server) => (
            <>
              <dt>Stash version</dt>
              <dd>{server().version}</dd>
            </>
          )}
        </Show>
      </dl>
    </section>
  );
}
