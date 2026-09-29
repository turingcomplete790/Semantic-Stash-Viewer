import { commands } from "../bindings";
import { player } from "../player/state";
import { close, navigate, select, selectedId, suspendSaving, tabs } from "../shell/tabs";

/**
 * Debug-only UI bench (004 T063; `SSV_DEBUG_BENCH=1`): times tab switches with 20 tabs open
 * (SC-002, SC-007), a control press (SC-003), and the now-playing bar (SC-008) in the real app,
 * and prints `MEASURE` lines through the core. Tab changes aren't saved during the run.
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

function summary(samples: number[]) {
  const sorted = [...samples].sort((a, b) => a - b);
  const at = (q: number) => sorted[Math.min(sorted.length - 1, Math.floor(q * sorted.length))];
  return {
    n: sorted.length,
    median: Math.round(at(0.5)),
    p95: Math.round(at(0.95)),
    max: Math.round(sorted[sorted.length - 1]),
  };
}

function report(name: string, value: unknown) {
  void commands.debugReport(JSON.stringify({ bench: name, ...(value as object) }));
}

export async function runBench(): Promise<void> {
  suspendSaving(true);
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

    // Now playing: switch away from a playing scene and back.
    void commands.playerSetMuted(true);
    navigate({ kind: "scene", sceneId: "10861", title: "Bench" }, { newTab: true });
    const sceneTab = selectedId();
    const playing = await until(() => player.snapshot().state === "playing", 20_000);
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
    report("done", {});
  }
}
