import { describe, expect, it } from "vitest";
import { customSlot, fileName, isSignedAttachment, mainButtonLabel, quickSlots, reminderNote, toUnix } from "./slots";

const fri10 = new Date(2026, 9, 30, 10, 0); // vendredi 30 oct. 2026 10:00

describe("quickSlots", () => {
  it("vendredi matin : ce soir, demain, lundi", () => {
    const s = quickSlots(fri10);
    expect(s.map(x => x.label)).toEqual(["ce soir, 18:00", "demain, 08:30", "lundi, 09:00"]);
    expect(s[0].at).toEqual(new Date(2026, 9, 30, 18, 0));
    expect(s[1].at).toEqual(new Date(2026, 9, 31, 8, 30));
    expect(s[2].at).toEqual(new Date(2026, 10, 2, 9, 0));
  });
  it("après 18:00 : plus de « ce soir »", () => {
    expect(quickSlots(new Date(2026, 9, 30, 18, 0)).map(x => x.id)).toEqual(["tomorrow", "monday"]);
  });
  it("un lundi : lundi = lundi suivant", () => {
    const mon = new Date(2026, 10, 2, 7, 0);
    expect(quickSlots(mon).find(x => x.id === "monday")!.at).toEqual(new Date(2026, 10, 9, 9, 0));
  });
  it("dimanche : lundi = le lendemain", () => {
    const sun = new Date(2026, 10, 1, 12, 0);
    expect(quickSlots(sun).find(x => x.id === "monday")!.at).toEqual(new Date(2026, 10, 2, 9, 0));
  });
  it("passage de mois", () => {
    const d = new Date(2026, 11, 31, 20, 0);
    expect(quickSlots(d)[0].at).toEqual(new Date(2027, 0, 1, 8, 30));
  });
});

describe("customSlot / libellé", () => {
  it("date future : libellé complet", () => {
    const s = customSlot(new Date(2026, 10, 12, 14, 5), fri10)!;
    expect(s.label).toBe("jeudi 12 nov., 14:05");
    expect(mainButtonLabel(s)).toBe("Planifier pour jeudi 12 nov., 14:05");
  });
  it("date passée ou invalide : null", () => {
    expect(customSlot(new Date(2026, 9, 1), fri10)).toBeNull();
    expect(customSlot(new Date("x"), fri10)).toBeNull();
  });
  it("sans créneau : Envoyer maintenant", () => expect(mainButtonLabel(null)).toBe("Envoyer maintenant"));
  it("toUnix en secondes", () => expect(toUnix(new Date(5000))).toBe(5));
});

describe("pièces jointes et relance", () => {
  it("fileName unix et windows", () => {
    expect(fileName("/a/b/devis.pdf")).toBe("devis.pdf");
    expect(fileName("C:\\x\\devis.pdf")).toBe("devis.pdf");
  });
  it("détecte le suffixe (signé)", () => {
    expect(isSignedAttachment("/d/Devis atelier, option B (signé).pdf")).toBe(true);
    expect(isSignedAttachment("/d/contrat_signed.pdf")).toBe(true);
    expect(isSignedAttachment("/d/contrat.pdf")).toBe(false);
  });
  it("note de relance", () => {
    expect(reminderNote("Léa Martin <lea@x.fr>", 3)).toBe("Si Léa Martin ne répond pas sous 3 jours");
    expect(reminderNote("", 1)).toBe("Si le destinataire ne répond pas sous 1 jour");
  });
});
