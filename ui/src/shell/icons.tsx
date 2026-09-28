/**
 * Inline SVG icons for the app shell (004 research R9). They inherit the text colour and scale
 * with the font, so no icon package is needed.
 *
 * Glyphs follow Lucide (https://lucide.dev), ISC licence:
 * Copyright (c) for portions of Lucide are held by Cole Bemis 2013-2022 as part of Feather (MIT).
 * All other copyright (c) for Lucide are held by Lucide Contributors 2022.
 * Permission to use, copy, modify, and/or distribute this software for any purpose with or
 * without fee is hereby granted, provided that the above copyright notice and this permission
 * notice appear in all copies.
 */
import type { JSX } from "solid-js";

type IconProps = { title?: string; class?: string };

function icon(children: () => JSX.Element) {
  return (props: IconProps) => (
    <svg
      class={props.class ? `icon ${props.class}` : "icon"}
      width="1em"
      height="1em"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      stroke-width="2"
      stroke-linecap="round"
      stroke-linejoin="round"
      role={props.title ? "img" : undefined}
      aria-hidden={props.title ? undefined : "true"}
    >
      {props.title && <title>{props.title}</title>}
      {children()}
    </svg>
  );
}

export const HomeIcon = icon(() => (
  <>
    <path d="m3 9 9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" />
    <path d="M9 22V12h6v10" />
  </>
));

export const ScenesIcon = icon(() => (
  <>
    <circle cx="12" cy="12" r="10" />
    <polygon points="10 8 16 12 10 16 10 8" />
  </>
));

export const BellIcon = icon(() => (
  <>
    <path d="M6 8a6 6 0 0 1 12 0c0 7 3 9 3 9H3s3-2 3-9" />
    <path d="M10.3 21a1.94 1.94 0 0 0 3.4 0" />
  </>
));

export const SettingsIcon = icon(() => (
  <>
    <path d="M20 7h-9" />
    <path d="M14 17H5" />
    <circle cx="17" cy="17" r="3" />
    <circle cx="7" cy="7" r="3" />
  </>
));

export const ServerIcon = icon(() => (
  <>
    <rect x="2" y="2" width="20" height="8" rx="2" />
    <rect x="2" y="14" width="20" height="8" rx="2" />
    <path d="M6 6h.01" />
    <path d="M6 18h.01" />
  </>
));

export const CloseIcon = icon(() => (
  <>
    <path d="M18 6 6 18" />
    <path d="m6 6 12 12" />
  </>
));

export const PlusIcon = icon(() => (
  <>
    <path d="M5 12h14" />
    <path d="M12 5v14" />
  </>
));

export const BackIcon = icon(() => <path d="m15 18-6-6 6-6" />);

export const ForwardIcon = icon(() => <path d="m9 18 6-6-6-6" />);

export const PlayIcon = icon(() => <polygon points="6 3 20 12 6 21 6 3" />);

export const PauseIcon = icon(() => (
  <>
    <rect x="14" y="4" width="4" height="16" rx="1" />
    <rect x="6" y="4" width="4" height="16" rx="1" />
  </>
));

export const MoreIcon = icon(() => (
  <>
    <circle cx="5" cy="12" r="1" />
    <circle cx="12" cy="12" r="1" />
    <circle cx="19" cy="12" r="1" />
  </>
));
