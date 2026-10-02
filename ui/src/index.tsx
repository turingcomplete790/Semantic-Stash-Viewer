import { render } from "solid-js/web";
import App from "./App";
import { commands } from "./bindings";
import "./styles.css";

// Keep whole stacks (WebKit keeps 100 frames by default), so a runaway loop shows where it starts.
(Error as unknown as { stackTraceLimit: number }).stackTraceLimit = 400;

// Debug builds print UI errors to the terminal (`MEASURE {"uiError": …}`); release builds ignore
// them. WebKit's console isn't visible otherwise.
window.addEventListener("error", (e) => {
  void commands.debugReport(
    JSON.stringify({
      uiError: String(e.message),
      at: `${e.filename}:${e.lineno}:${e.colno}`,
      stack: String((e.error as Error | undefined)?.stack ?? "")
        .split("\n")
        .slice(0, 6),
    }),
  );
});
/** A stack summary: its top and bottom, and which frames repeat most (recursion). */
function stackSummary(stack: string) {
  const frames = stack.split("\n").map((f) => f.replace(/http:\/\/localhost:5173/g, ""));
  const counts = new Map<string, number>();
  for (const f of frames) counts.set(f, (counts.get(f) ?? 0) + 1);
  const repeated = [...counts.entries()]
    .filter(([, n]) => n > 3)
    .sort((a, b) => b[1] - a[1])
    .slice(0, 15)
    .map(([f, n]) => `${n}× ${f}`);
  return { depth: frames.length, top: frames.slice(0, 8), bottom: frames.slice(-25), repeated };
}

window.addEventListener("unhandledrejection", (e) => {
  const reason = e.reason as Error | undefined;
  void commands.debugReport(
    JSON.stringify({
      uiRejection: String(reason?.message ?? reason),
      ...stackSummary(String(reason?.stack ?? "")),
    }),
  );
});

const root = document.getElementById("root");
if (!root) throw new Error("#root element missing from index.html");

render(() => <App />, root);
