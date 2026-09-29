import { describe, expect, it } from "vitest";
import { isRoute, routeTitle, sameView } from "../../shell/routes";

describe("routes", () => {
  it("titles each view", () => {
    expect(routeTitle({ kind: "home" })).toBe("Home");
    expect(routeTitle({ kind: "scenes" })).toBe("Scenes");
    expect(routeTitle({ kind: "scene", sceneId: "7", title: "Beach day" })).toBe("Beach day");
    expect(routeTitle({ kind: "scene", sceneId: "7", title: "" })).toBe("Scene 7");
    expect(routeTitle({ kind: "settings", page: "about" })).toBe("Settings");
  });

  it("recognises restored routes and rejects anything else", () => {
    expect(isRoute({ kind: "home" })).toBe(true);
    expect(isRoute({ kind: "scene", sceneId: "1", title: "x" })).toBe(true);
    expect(isRoute({ kind: "settings", page: "keyboard" })).toBe(true);
    expect(isRoute({ kind: "settings", page: "nope" })).toBe(false);
    expect(isRoute({ kind: "scene", title: "no id" })).toBe(false);
    expect(isRoute({ kind: "galleries" })).toBe(false);
    expect(isRoute(null)).toBe(false);
    expect(isRoute("home")).toBe(false);
  });

  it("compares views by what they show", () => {
    expect(sameView({ kind: "home" }, { kind: "home" })).toBe(true);
    expect(
      sameView(
        { kind: "scene", sceneId: "1", title: "a" },
        { kind: "scene", sceneId: "1", title: "b" },
      ),
    ).toBe(true);
    expect(
      sameView(
        { kind: "scene", sceneId: "1", title: "a" },
        { kind: "scene", sceneId: "2", title: "a" },
      ),
    ).toBe(false);
    expect(
      sameView({ kind: "settings", page: "about" }, { kind: "settings", page: "servers" }),
    ).toBe(false);
  });
});
