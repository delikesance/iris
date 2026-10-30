import { describe, expect, it } from "vitest";
import type { Attachment, ThreadMessage } from "../../lib/types";
import {
  addrList, canSign, defaultOpen, groupByDay, legend, participants, pendingReminder, replyAllDraft,
  splitQuoted, supersededLabel,
} from "./helpers";

const ME = "moi@iris.fr";
const t = (d: number, h: number) => Math.floor(new Date(2026, 9, d, h, 0).getTime() / 1000);
const msg = (uid: number, from: string, to: string, date: number, extra: Partial<ThreadMessage> = {}): ThreadMessage => ({
  uid, folder: "INBOX", from_name: from.split("@")[0], from_addr: from, to, subject: "Contrat de prestation, clause 4",
  date, message_id: `<m${uid}@x>`, text: "", html: null, attachments: [], superseded: false, ...extra,
});
const thread = [
  msg(1, "camille@x.fr", `Moi <${ME}>, Thomas <thomas@x.fr>`, t(29, 12)),
  msg(2, ME, "Camille <camille@x.fr>; thomas@x.fr", t(29, 14)),
  msg(3, "thomas@x.fr", `camille@x.fr, ${ME}`, t(30, 9)),
];

describe("participants", () => {
  it("ordre d'apparition, compteurs, Vous", () => {
    const ps = participants(thread, ME);
    expect(ps.map(p => [p.name, p.count, p.me])).toEqual([["camille", 1, false], ["Vous", 1, true], ["thomas", 1, false]]);
    expect(legend(thread, ps)).toBe("3 messages · 3 participants");
    expect(legend(thread.slice(0, 1), ps.slice(0, 1))).toBe("1 message · 1 participant");
  });
  it("addrList extrait les adresses", () => {
    expect(addrList("A <A@x.fr>; b@y.fr")).toEqual(["a@x.fr", "b@y.fr"]);
  });
});

describe("regroupement", () => {
  it("par jour", () => {
    const g = groupByDay(thread);
    expect(g.map(x => [x.label, x.items.length])).toEqual([["Jeudi 29 octobre", 2], ["Vendredi 30 octobre", 1]]);
  });
  it("deux derniers dépliés", () => expect([...defaultOpen(thread)]).toEqual([2, 3]));
});

describe("citation", () => {
  it("coupe à « a écrit » et aux lignes >", () => {
    const r = splitQuoted("OK pour moi.\n\nLe jeudi 29 oct. à 12:00, Camille a écrit :\n> bonjour");
    expect(r.body).toBe("OK pour moi.");
    expect(r.quoted).toContain("> bonjour");
    expect(splitQuoted("rien")).toEqual({ body: "rien", quoted: "" });
  });
});

describe("pièces remplacées", () => {
  const att = (o: Partial<Attachment>): Attachment => ({ index: 0, filename: "c.pdf", mime: "application/pdf", size: 1, signable: true, ...o });
  it("libellé avec version", () => {
    expect(supersededLabel("Contrat de prestation v3.pdf")).toBe("Remplacée par la v3, non signable");
    expect(supersededLabel("contrat_v3.pdf")).toBe("Remplacée par la v3, non signable");
    expect(supersededLabel("contrat-2.pdf")).toBe("Remplacée par la v2, non signable");
    expect(supersededLabel(undefined)).toBe("Remplacée par une version plus récente, non signable");
  });
  it("seule la dernière version se signe", () => {
    expect(canSign(att({}), thread[0])).toBe(true);
    expect(canSign(att({ superseded_by: "c_v3.pdf" }), thread[0])).toBe(false);
    expect(canSign(att({}), { ...thread[0], superseded: true })).toBe(false);
    expect(canSign(att({ signable: false }), thread[0])).toBe(false);
  });
});

describe("Répondre à tous", () => {
  it("dernier message d'un tiers : à lui, Cc les autres sans moi", () => {
    const d = replyAllDraft(thread, ME);
    expect(d.to).toBe("thomas@x.fr");
    expect(d.cc).toBe("camille@x.fr");
    expect(d.subject).toBe("Re : Contrat de prestation, clause 4");
    expect(d.in_reply_to).toBe("<m3@x>");
  });
  it("dernier message de moi : à ses destinataires", () => {
    const d = replyAllDraft(thread.slice(0, 2), ME);
    expect(d.to).toBe("camille@x.fr, thomas@x.fr");
    expect(d.cc).toBeNull();
  });
  it("pas de double Re", () => {
    expect(replyAllDraft([{ ...thread[2], subject: "Re: x" }], ME).subject).toBe("Re: x");
  });
});

describe("pastille de relance", () => {
  it("seulement si mon message est le dernier et qu'une relance est posée", () => {
    expect(pendingReminder(thread.slice(0, 2), ME, 3)).toBe("Relance automatique dans 3 jours sans réponse");
    expect(pendingReminder(thread, ME, 3)).toBeNull();
    expect(pendingReminder(thread.slice(0, 2), ME, null)).toBeNull();
    expect(pendingReminder([], ME, 3)).toBeNull();
  });
});
