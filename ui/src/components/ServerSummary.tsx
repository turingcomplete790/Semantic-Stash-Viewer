import { For, Show } from "solid-js";
import type { SecurityState, ServerInfo } from "../bindings";
import { securityText } from "../messages/security";
import "./ServerSummary.css";

const numberFormat = new Intl.NumberFormat("en-US");

/** Address, Stash version, and library counts (FR-007). */
export default function ServerSummary(props: {
  url: string;
  server: ServerInfo;
  security?: SecurityState | null;
}) {
  const counts = () => [
    { label: "Scenes", value: props.server.counts.scenes },
    { label: "Images", value: props.server.counts.images },
    { label: "Galleries", value: props.server.counts.galleries },
    { label: "Performers", value: props.server.counts.performers },
  ];

  return (
    <div class="server-summary">
      <dl class="server-facts">
        <dt>Address</dt>
        <dd>{props.url}</dd>
        <Show when={props.security}>
          {(sec) => (
            <>
              <dt>Connection</dt>
              <dd>{securityText[sec()].label}</dd>
            </>
          )}
        </Show>
        <dt>Stash version</dt>
        <dd>
          <span>{props.server.version}</span>
          <Show when={props.server.versionStatus === "developmentBuild"}>
            <span class="badge warn" title="Compatibility isn't guaranteed">
              development build
            </span>
          </Show>
          <Show when={props.server.versionStatus === "unknownButCompatible"}>
            <span class="badge warn" title="Compatibility isn't guaranteed">
              version not recognised
            </span>
          </Show>
        </dd>
      </dl>
      <ul class="counts" aria-label="Library summary">
        <For each={counts()}>
          {(c) => (
            <li>
              <span class="count">{numberFormat.format(c.value)}</span>
              <span class="count-label">{c.label}</span>
            </li>
          )}
        </For>
      </ul>
    </div>
  );
}
