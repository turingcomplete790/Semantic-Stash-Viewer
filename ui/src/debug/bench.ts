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

/**
 * The display's frame time, from an idle animation-frame loop (median of 60 intervals). Missed
 * frames are judged against this, not against the frames being measured: if every frame slows
 * equally, comparing to their own median would report nothing missed (005 analysis M1).
 */
async function displayFrameTime(): Promise<number> {
  const intervals: number[] = [];
  let last = await new Promise<number>((r) => requestAnimationFrame(r));
  for (let i = 0; i < 60; i += 1) {
    const now = await new Promise<number>((r) => requestAnimationFrame(r));
    intervals.push(now - last);
    last = now;
  }
  intervals.sort((a, b) => a - b);
  return intervals[Math.floor(intervals.length / 2)];
}

/** Scroll `el` by `pxPerFrame` every animation frame for `ms`; every frame's interval. */
async function scrollFor(el: HTMLElement, pxPerFrame: number, ms: number): Promise<number[]> {
  const intervals: number[] = [];
  await new Promise<void>((resolve) => {
    let last = performance.now();
    const end = last + ms;
    const step = (now: number) => {
      intervals.push(now - last);
      last = now;
      const max = el.scrollHeight - el.clientHeight;
      el.scrollTop = el.scrollTop + pxPerFrame >= max ? 0 : el.scrollTop + pxPerFrame;
      if (now < end) requestAnimationFrame(step);
      else resolve();
    };
    requestAnimationFrame(step);
  });
  return intervals.slice(1);
}

/** The page whose first card is `firstTitle` is on screen, its visible thumbnails loaded. */
function pageShown(pane: HTMLElement, firstTitle: string): boolean {
  const first = pane.querySelector<HTMLElement>('.scene-grid button.scene-card[data-index="0"]');
  if (first?.getAttribute("aria-label") !== firstTitle) return false;
  const scroller = pane.querySelector<HTMLElement>(".scenes-scroller");
  const bottom = (scroller?.getBoundingClientRect().bottom ?? Infinity) + 1;
  return Array.from(pane.querySelectorAll<HTMLImageElement>(".scene-grid img")).every(
    (img) => img.getBoundingClientRect().top > bottom || img.complete,
  );
}

/** The first card's title on the page currently shown. */
function firstTitle(pane: HTMLElement): string | null {
  return (
    pane
      .querySelector<HTMLElement>('.scene-grid button.scene-card[data-index="0"]')
      ?.getAttribute("aria-label") ?? null
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
    // when it takes over 1.5 of the display's frame time, measured idle beforehand, so timer
    // jitter around vsync doesn't count, any refresh rate works, and a uniform slowdown still
    // counts (003 research R8). Frames per second are reported too.
    const baseline = await displayFrameTime();
    const { runScrollBench } = await import("./ScrollBench");
    const frames = await runScrollBench();
    const missed = frames.filter((f) => f > baseline * 1.5).length;
    const elapsed = frames.reduce((a, b) => a + b, 0);
    report("scroll-frame-time", {
      ...summary(frames, 1),
      baselineMs: Math.round(baseline * 10) / 10,
      fps: elapsed > 0 ? Math.round((frames.length / elapsed) * 10000) / 10 : null,
      missedPercent: frames.length ? Math.round((missed / frames.length) * 1000) / 10 : null,
    });

    // The paged Scenes view (005 SC-002, SC-003; research R10): page changes at 50 per page until
    // the new page and its visible thumbnails are shown, jumps to random pages, and scrolling a
    // 1000-card page in grid and list mode against the idle baseline.
    navigate({ kind: "scenes" }, { newTab: true });
    const scenesTab = selectedId();
    const pane = () => document.querySelector<HTMLElement>(".tab-pane:not([hidden])");
    const ready = await until(
      () => pane() != null && firstTitle(pane() as HTMLElement) !== null,
      15_000,
    );
    const view = pane();
    if (ready && view) {
      const button = (name: string) =>
        view.querySelector<HTMLButtonElement>(`.scenes-toolbar button[aria-label="${name}"]`);
      const changes: number[] = [];
      for (let i = 0; i < 20; i += 1) {
        // Glance at the page first, as a person would; meanwhile the next page and its
        // thumbnails are loaded ahead (research R4).
        await new Promise((r) => setTimeout(r, 1000));
        const before = firstTitle(view);
        const target = button(i < 10 ? "Next page" : "Previous page");
        if (!target || target.disabled) break;
        const t0 = performance.now();
        target.click();
        const shown = await until(() => {
          const now = firstTitle(view);
          return now !== null && now !== before && pageShown(view, now);
        }, 5000);
        await nextFrame();
        if (shown) changes.push(performance.now() - t0);
      }
      report("scenes-page-change", summary(changes.length ? changes : [NaN]));

      const jumps: number[] = [];
      const go = view.querySelector<HTMLInputElement>(
        '.scenes-toolbar input[aria-label="Go to page"]',
      );
      const lastPage = Number(go?.max ?? 1);
      let seed = 11;
      for (let i = 0; i < 5 && go; i += 1) {
        seed = (seed * 7919 + 13) % 10007;
        const target = 1 + (seed % Math.max(1, lastPage));
        const before = firstTitle(view);
        const t0 = performance.now();
        go.value = String(target);
        go.dispatchEvent(new Event("input", { bubbles: true }));
        go.form?.requestSubmit();
        const shown = await until(() => {
          const now = firstTitle(view);
          // SC-003: its cards are shown (thumbnails may still be arriving from Stash).
          return now !== null && now !== before;
        }, 5000);
        await nextFrame();
        if (shown) jumps.push(performance.now() - t0);
      }
      report("scenes-page-jump", summary(jumps.length ? jumps : [NaN]));

      // A 1000-card page, scrolled in each mode.
      const perPage = view.querySelector<HTMLSelectElement>(
        '.scenes-toolbar select[aria-label="Per page"]',
      );
      const scroller = view.querySelector<HTMLDivElement>(".scenes-scroller");
      if (perPage && scroller) {
        perPage.value = "1000";
        perPage.dispatchEvent(new Event("change", { bubbles: true }));
        await until(
          () => view.querySelectorAll(".scene-grid button.scene-card").length >= 1000,
          15_000,
        );
        // Scrolling is measured on a loaded page; how long its thumbnails take to arrive is
        // reported alongside (005 T024: WebKitGTK costs frames for every thumbnail that loads
        // during the scroll, so the page loads them all at once rather than lazily).
        const t0 = performance.now();
        const loaded = await until(
          () =>
            Array.from(view.querySelectorAll<HTMLImageElement>(".scene-grid img")).every(
              (img) => img.complete,
            ),
          60_000,
        );
        report("scenes-thumbs-1000", {
          loaded,
          ms: Math.round(performance.now() - t0),
        });
        for (const mode of ["grid", "list"] as const) {
          view
            .querySelectorAll<HTMLButtonElement>(".scenes-mode button")
            [mode === "grid" ? 0 : 1]?.click();
          await nextFrame();
          scroller.scrollTop = 0;
          await nextFrame();
          const base = await displayFrameTime();
          const frames = await scrollFor(scroller, 64, 5000);
          const missed = frames.filter((f) => f > base * 1.5).length;
          const elapsed = frames.reduce((a, b) => a + b, 0);
          report(`scenes-scroll-1000-${mode}`, {
            ...summary(frames, 1),
            baselineMs: Math.round(base * 10) / 10,
            fps: elapsed > 0 ? Math.round((frames.length / elapsed) * 10000) / 10 : null,
            missedPercent: frames.length ? Math.round((missed / frames.length) * 1000) / 10 : null,
          });
        }
        view.querySelectorAll<HTMLButtonElement>(".scenes-mode button")[0]?.click();
        perPage.value = "50";
        perPage.dispatchEvent(new Event("change", { bubbles: true }));
        await nextFrame();
      }
    }
    close(scenesTab);
    await nextFrame();

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
