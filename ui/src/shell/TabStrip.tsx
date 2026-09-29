import { For } from "solid-js";
import { BackIcon, CloseIcon, ForwardIcon, PlusIcon } from "./icons";
import { routeIcon, routeTitle, isRoute } from "./routes";
import {
  back,
  canGoBack,
  canGoForward,
  close,
  forward,
  navigate,
  routeOf,
  select,
  selectedId,
  tabs,
} from "./tabs";
import type { TabState } from "./tabs";

function title(tab: TabState): string {
  const route = routeOf(tab);
  return isRoute(route) ? routeTitle(route) : "Unavailable";
}

/** The tab strip (004 FR-008–FR-012): back/forward, tabs, and a new-tab button. */
export default function TabStrip() {
  return (
    <div class="tab-strip">
      <button
        type="button"
        class="nav-icon-button"
        aria-label="Back"
        title="Back (Alt+Left)"
        disabled={!canGoBack()}
        onClick={back}
      >
        <BackIcon />
      </button>
      <button
        type="button"
        class="nav-icon-button"
        aria-label="Forward"
        title="Forward (Alt+Right)"
        disabled={!canGoForward()}
        onClick={forward}
      >
        <ForwardIcon />
      </button>
      {/* The + sits right after the last tab, where users look for it. */}
      <div class="tab-row">
        <div class="tab-list" role="tablist" aria-label="Open tabs">
          <For each={tabs()}>
            {(tab) => {
              const route = () => routeOf(tab);
              const Icon = () => {
                const r = route();
                const Glyph = isRoute(r) ? routeIcon(r) : null;
                return Glyph ? <Glyph /> : null;
              };
              return (
                <div class="tab" classList={{ selected: selectedId() === tab.id }}>
                  <button
                    type="button"
                    role="tab"
                    class="tab-label"
                    aria-selected={selectedId() === tab.id}
                    title={title(tab)}
                    onClick={() => select(tab.id)}
                    onAuxClick={(e) => {
                      if (e.button === 1) {
                        e.preventDefault();
                        close(tab.id);
                      }
                    }}
                  >
                    <Icon />
                    <span class="tab-title">{title(tab)}</span>
                  </button>
                  <button
                    type="button"
                    class="tab-close"
                    aria-label={`Close ${title(tab)}`}
                    title="Close tab (Ctrl+W)"
                    onClick={() => close(tab.id)}
                  >
                    <CloseIcon />
                  </button>
                </div>
              );
            }}
          </For>
        </div>
        <button
          type="button"
          class="nav-icon-button"
          aria-label="New tab"
          title="New tab (Ctrl+T)"
          onClick={() => navigate({ kind: "home" }, { newTab: true })}
        >
          <PlusIcon />
        </button>
      </div>
    </div>
  );
}
