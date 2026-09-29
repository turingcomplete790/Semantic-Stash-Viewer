import type { JSX } from "solid-js";
import "./shell.css";

/**
 * The app shell layout (constitution Principle IX; 004 research R11): navigation bar, tab strip,
 * content area, "now playing" bar, and toast region. The slots fill in as the stories land.
 */
export default function Shell(props: {
  nav: JSX.Element;
  tabs?: JSX.Element;
  nowPlaying?: JSX.Element;
  toasts?: JSX.Element;
  children: JSX.Element;
}) {
  return (
    <div class="shell">
      <header class="shell-top">{props.nav}</header>
      <div class="shell-tabs">{props.tabs}</div>
      <main class="shell-content">{props.children}</main>
      <div class="shell-now-playing">{props.nowPlaying}</div>
      <div class="shell-toasts">{props.toasts}</div>
    </div>
  );
}
