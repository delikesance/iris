import { describe, expect, it } from "vitest";
import { aboutKind, blankChron, canSave, isoLabel, kindOptions, nextRunLabel } from "./chronForm";

describe("chronForm", () => {
  it("affiche En pause pour un chron désactivé ou sans date", () => {
    expect(nextRunLabel({ enabled: false, next_run: 1_800_000_000 })).toBe("En pause");
    expect(nextRunLabel({ enabled: true, next_run: null })).toBe("En pause");
  });

  it("formate le prochain passage en français", () => {
    const label = nextRunLabel({ enabled: true, next_run: Date.UTC(2026, 10, 2, 7, 0) / 1000 });
    expect(label).toMatch(/^Prochain passage : lundi 2 nov\.?,? \d{2}:\d{2}$/);
    expect(isoLabel("2026-11-02T07:00:00Z")).toMatch(/lundi 2 nov/);
  });

  it("propose les 4 tâches du contrat et un brouillon vide valide pour l'API", () => {
    expect(kindOptions.map((o) => o.kind)).toEqual(["morning_digest", "reminders", "newsletter_cleanup", "monthly_invoices"]);
    expect(aboutKind("reminders")).toMatch(/Relancer/);
    expect(blankChron()).toMatchObject({ id: null, enabled: true, kind: "morning_digest" });
  });

  it("n'autorise l'enregistrement qu'avec un nom et une expression valide", () => {
    expect(canSave("  ", true)).toBe(false);
    expect(canSave("Matin", false)).toBe(false);
    expect(canSave("Matin", true)).toBe(true);
  });
});
