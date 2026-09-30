import { getOwner, onCleanup } from "solid-js";
import { events } from "../bindings";
import { connection } from "./connection";

/**
 * Call `callback` when the cached data behind `key` changes (a background refresh brought
 * something new, or the cache was cleared: key `*`). Only for the current profile. Stops with
 * the calling component (003 contracts "view-data-changed").
 */
export function onViewDataChanged(key: string, callback: () => void): void {
  let unlisten: (() => void) | undefined;
  let disposed = false;
  void events.viewDataChanged
    .listen((e) => {
      const active = connection.snapshot().profileId;
      if (active && e.payload.profileId !== active) return;
      if (e.payload.key === key || e.payload.key === "*") callback();
    })
    .then((stop) => {
      if (disposed) stop();
      else unlisten = stop;
    });
  if (getOwner()) {
    onCleanup(() => {
      disposed = true;
      unlisten?.();
    });
  }
}
