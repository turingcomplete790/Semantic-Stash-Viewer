import { commands } from "../bindings";
import { player } from "../player/state";
import { close, navigate, select, selectedId, suspendSaving, tabs } from "../shell/tabs";

/**
 * Debug-only UI bench (004 T063; `SSV_DEBUG_BENCH=1`): times tab switches with 20 tabs open
 * (SC-002, SC-007), section navigation (003), a control press (SC-003), scrolling a
 * 10,000-row list (003), and the now-playing bar (SC-008) in the real app, and prints `MEASURE`
 * lines through the core. Tab changes aren't saved during the run.
 *
 * A measurement taken while the window was hidden is marked invalid (003 FR-016): hidden
 * windows don't paint, so their frame timings mean nothing.
 */

/** Resolves after the next painted frame (two animation frames). */
function nextFrame(): Promise<void> {
  return new Promise((resolve) =>
    requestAnimationFrame(() => requestAnimationFrame(() => resolve())),
  );
}

async function until(check: () => boolean, timeoutMs: number): Promise<boolean> {
  const end = performance.now() + timeoutMs;
  while (performance.now() < end) {
    if (check()) return true;
    await nextFrame();
  }
  return check();
}

function summary(samples: number[], decimals = 0) {
  const sorted = [...samples].sort((a, b) => a - b);
  const at = (q: number) => sorted[Math.min(sorted.length - 1, Math.floor(q * sorted.length))];
  const round = (v: number) => Math.round(v * 10 ** decimals) / 10 ** decimals;
  return {
    n: sorted.length,
    median: round(at(0.5)),
    p95: round(at(0.95)),
    max: round(sorted[sorted.length - 1]),
  };
}

/** Set when the window was hidden at any point since the last report. */
let hiddenSinceReport = false;
function watchVisibility(): () => void {
  hiddenSinceReport = document.visibilityState !== "visible";
  const onChange = () => {
    if (document.visibilityState !== "visible") hiddenSinceReport = true;
  };
  document.addEventListener("visibilitychange", onChange);
  return () => document.removeEventListener("visibilitychange", onChange);
}

function report(name: string, value: unknown) {
  const hidden = hiddenSinceReport || document.visibilityState !== "visible";
  hiddenSinceReport = document.visibilityState !== "visible";
  const line = {
    bench: name,
    ...(value as object),
    ...(hidden ? { invalid: "window hidden" } : {}),
  };
  void commands.debugReport(JSON.stringify(line));
}

export async function runBench(): Promise<void> {
  suspendSaving(true);
  const stopWatching = watchVisibility();
  const original = tabs().map((t) => t.id);
  const originalSelected = selectedId();
  try {
    // 20 tabs open.
    const kinds = [
      { kind: "scenes" } as const,
      { kind: "home" } as const,
      { kind: "settings", page: "keyboard" } as const,
    ];
    while (tabs().length < 20) navigate(kinds[tabs().length % kinds.length], { newTab: true });
    await nextFrame();

    // Tab switches: a fixed pseudo-random walk over all 20, including unmounted ones (> 8).
    const switches: number[] = [];
    let seed = 7;
    for (let i = 0; i < 40; i += 1) {
      seed = (seed * 13 + 5) % 20;
      if (tabs()[seed].id === selectedId()) continue;
      const t0 = performance.now();
      select(seed);
      await nextFrame();
      switches.push(performance.now() - t0);
    }
    report("tab-switch-20-tabs", summary(switches));

    // Section navigation: Home ↔ Scenes in one tab, each to its next painted frame (warm cache).
    navigate({ kind: "home" }, { newTab: true });
    const navTab = selectedId();
    await nextFrame();
    const navs: number[] = [];
    for (let i = 0; i < 20; i += 1) {
      const t0 = performance.now();
      navigate(i % 2 === 0 ? { kind: "scenes" } : { kind: "home" });
      await nextFrame();
      navs.push(performance.now() - t0);
    }
    report("nav-first-paint", summary(navs));
    close(navTab);
    await nextFrame();

    // Control press: the bell opens its panel.
    const presses: number[] = [];
    for (let i = 0; i < 10; i += 1) {
      const bell = document.querySelector<HTMLButtonElement>('button[aria-label^="Notifications"]');
      if (!bell) break;
      const t0 = performance.now();
      bell.click();
      await nextFrame();
      presses.push(performance.now() - t0);
      bell.click();
      await nextFrame();
    }
    report("control-press", summary(presses));

    // Scrolling a 10,000-row list. 60 fps means no missed frames: a frame counts as missed
    // when it takes over 1.5 of the display's frame time (the median interval), so timer
    // jitter around vsync doesn't count and any refresh rate works (003 research R8).
    const { runScrollBench } = await import("./ScrollBench");
    const frames = await runScrollBench();
    const frameTime = summary(frames, 1).median;
    const missed = frames.filter((f) => f > frameTime * 1.5).length;
    report("scroll-frame-time", {
      ...summary(frames, 1),
      missedPercent: frames.length ? Math.round((missed / frames.length) * 1000) / 10 : null,
    });

    // Now playing: switch away from a playing scene (the newest in the library) and back.
    void commands.playerSetMuted(true);
    const recent = await commands.listRecentScenes();
    const benchScene = recent.status === "ok" ? recent.data.data[0]?.id : undefined;
    navigate({ kind: "scene", sceneId: benchScene ?? "", title: "Bench" }, { newTab: true });
    const sceneTab = selectedId();
    const playing =
      benchScene !== undefined &&
      (await until(() => player.snapshot().state === "playing", 20_000));
    const away: number[] = [];
    const back: number[] = [];
    if (playing) {
      for (let i = 0; i < 10; i += 1) {
        const t0 = performance.now();
        select(0);
        await until(() => document.querySelector(".now-playing") !== null, 2000);
        await nextFrame();
        away.push(performance.now() - t0);
        const t1 = performance.now();
        select(sceneTab);
        await nextFrame();
        back.push(performance.now() - t1);
      }
    }
    report("now-playing-appears", { playing, ...summary(away.length ? away : [NaN]) });
    report("back-to-scene", summary(back.length ? back : [NaN]));
    close(sceneTab);
  } finally {
    // Put the tabs back as they were.
    for (const t of [...tabs()]) if (!original.includes(t.id)) close(t.id);
    select(originalSelected);
    suspendSaving(false);
    stopWatching();
    report("done", {});
  }
}
