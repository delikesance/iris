import { describe, expect, it } from "vitest";
import { NAV, isActive, ruleIdOf, screenOf } from "./router";

describe("router", () => {
  it("screenOf maps sub-routes to their screen", () => {
    expect(screenOf("mail")).toBe("mail");
    expect(screenOf("settings/rules/12")).toBe("settings");
    expect(screenOf("login")).toBe("login");
  });
  it("ruleIdOf reads the rule id only on rule deep links", () => {
    expect(ruleIdOf("settings/rules/12")).toBe(12);
    expect(ruleIdOf("settings")).toBeNull();
    expect(ruleIdOf("mail")).toBeNull();
  });
  it("NAV lists the six entries in order", () => {
    expect(NAV.map(e => e.label)).toEqual(["Mails", "Contacts", "Récurrents", "Chrons", "Signatures", "Réglages"]);
  });
  it("isActive highlights Réglages for a rule deep link", () => {
    const settings = NAV.find(e => e.route === "settings")!;
    expect(isActive(settings, "settings/rules/3")).toBe(true);
    expect(isActive(NAV[0], "settings/rules/3")).toBe(false);
  });
});

describe("route thread", () => {
  it("screenOf, threadRoute et threadIdOf", async () => {
    const { screenOf, threadRoute, threadIdOf } = await import("./router");
    const r = threadRoute("a/b<c>");
    expect(screenOf(r)).toBe("thread");
    expect(threadIdOf(r)).toBe("a/b<c>");
    expect(threadIdOf("mail")).toBeNull();
  });
});
