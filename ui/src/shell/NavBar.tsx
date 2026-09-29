import { createSignal, For, onCleanup, onMount, Show } from "solid-js";
import ConnectionIndicator from "../components/ConnectionIndicator";
import { connection } from "../state/connection";
import { MoreIcon, SettingsIcon } from "./icons";
import NotificationBell, { toggleNotifications } from "./NotificationCentre";
import { register } from "./keymap";
import type { Route } from "./routes";
import { SECTIONS, sectionFor } from "./sections";
import type { Section } from "./sections";
import ServerMenu from "./ServerMenu";

/** Room kept for the "More" button when sections overflow. */
const MORE_BUTTON_WIDTH = 48;

export type NavigateOptions = { newTab: boolean };

/**
 * The navigation bar (constitution Principle IX; 004 FR-001–FR-007): the viewer's name, the
 * sections that exist, then the notification bell, Settings, and the current server.
 */
export default function NavBar(props: {
  /** The route shown in the current tab, or null while disconnected. */
  current: Route | null;
  onHome: () => void;
  onNavigate: (route: Route, options: NavigateOptions) => void;
  onOpenSettings: () => void;
  onUpdateKey: () => void;
  onManageServers: () => void;
}) {
  const connected = () => connection.snapshot().profileId !== null;
  const sections = () =>
    [...SECTIONS].filter((s) => !s.needsServer || connected()).sort((a, b) => a.order - b.order);
  const currentSection = () => (props.current ? sectionFor(props.current)?.id : undefined);
  const settingsCurrent = () => props.current?.kind === "settings";

  // Overflow (FR-007): how many sections fit; the rest go under "More".
  const [fit, setFit] = createSignal(Number.POSITIVE_INFINITY);
  const [moreOpen, setMoreOpen] = createSignal(false);
  let sectionsEl: HTMLDivElement | undefined;
  let measureEl: HTMLDivElement | undefined;
  const measure = () => {
    if (!sectionsEl || !measureEl) return;
    const available = sectionsEl.getBoundingClientRect().width;
    const widths = [...measureEl.querySelectorAll(".nav-section")].map(
      (el) => el.getBoundingClientRect().width,
    );
    const total = widths.reduce((a, b) => a + b, 0);
    if (total <= available) {
      setFit(Number.POSITIVE_INFINITY);
      return;
    }
    let used = MORE_BUTTON_WIDTH;
    let count = 0;
    for (const w of widths) {
      if (used + w > available) break;
      used += w;
      count += 1;
    }
    setFit(count);
  };
  onMount(() => {
    const observer = new ResizeObserver(measure);
    if (sectionsEl) observer.observe(sectionsEl);
    measure();
    onCleanup(() => observer.disconnect());
  });
  const visible = () => sections().slice(0, fit());
  const overflow = () => sections().slice(fit());

  function go(section: Section, e?: MouseEvent) {
    setMoreOpen(false);
    props.onNavigate(section.route, { newTab: !!e && (e.ctrlKey || e.metaKey || e.button === 1) });
  }

  // Shortcuts: Stash's `g` sequences plus `g h` and `g n` (research R7).
  onMount(() => {
    const removers = [
      ...SECTIONS.map((section) =>
        register({
          id: `section:${section.id}`,
          keys: [section.shortcut],
          scope: "shell",
          description: `Go to ${section.label}`,
          run: () => {
            if (!section.needsServer || connected()) {
              props.onNavigate(section.route, { newTab: false });
            }
          },
        }),
      ),
      register({
        id: "settings",
        keys: ["g z"],
        scope: "shell",
        description: "Open Settings",
        run: () => props.onOpenSettings(),
      }),
      register({
        id: "notifications",
        keys: ["g n"],
        scope: "shell",
        description: "Open notifications",
        run: toggleNotifications,
      }),
    ];
    onCleanup(() => removers.forEach((remove) => remove()));
  });

  const sectionButton = (section: Section) => (
    <button
      type="button"
      class="nav-section"
      classList={{ current: currentSection() === section.id }}
      aria-current={currentSection() === section.id ? "page" : undefined}
      title={`${section.label} (${section.shortcut})`}
      onClick={(e) => go(section, e)}
      onAuxClick={(e) => {
        if (e.button === 1) go(section, e);
      }}
    >
      <section.icon />
      <span>{section.label}</span>
    </button>
  );

  return (
    <nav class="navbar" aria-label="Main">
      <button type="button" class="nav-brand" onClick={() => props.onHome()}>
        Semantic Stash Viewer
      </button>

      <div class="nav-sections" ref={sectionsEl}>
        <For each={visible()}>{sectionButton}</For>
        <Show when={overflow().length > 0}>
          <div class="nav-more">
            <button
              type="button"
              class="nav-icon-button"
              aria-label="More sections"
              aria-haspopup="menu"
              aria-expanded={moreOpen()}
              onClick={() => setMoreOpen((o) => !o)}
            >
              <MoreIcon />
            </button>
            <Show when={moreOpen()}>
              <div class="nav-menu" role="menu">
                <For each={overflow()}>
                  {(section) => (
                    <button type="button" role="menuitem" onClick={(e) => go(section, e)}>
                      {section.label}
                    </button>
                  )}
                </For>
              </div>
            </Show>
          </div>
        </Show>
        {/* Off-screen copy used to measure every section's width. */}
        <div class="nav-measure" ref={measureEl} aria-hidden="true">
          <For each={sections()}>
            {(section) => (
              <span class="nav-section">
                <section.icon />
                <span>{section.label}</span>
              </span>
            )}
          </For>
        </div>
      </div>

      <div class="nav-right">
        <NotificationBell />
        <button
          type="button"
          class="nav-icon-button"
          classList={{ current: settingsCurrent() }}
          aria-label="Settings"
          aria-current={settingsCurrent() ? "page" : undefined}
          title="Settings (g z)"
          onClick={() => props.onOpenSettings()}
        >
          <SettingsIcon />
        </button>
        <ConnectionIndicator onUpdateKey={props.onUpdateKey} onManage={props.onManageServers} />
        <ServerMenu onUpdateKey={props.onUpdateKey} onManageServers={props.onManageServers} />
      </div>
    </nav>
  );
}
