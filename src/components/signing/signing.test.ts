import { describe, expect, it } from "vitest";
import { placeRect, rectToCss } from "./geometry";
import { actionOf, meSigned, needsSoffice, progressLabel, replyDraft, stateOf } from "./format";
import type { Document } from "../../lib/types";

const doc = (over: Partial<Document> = {}): Document => ({
  id: 1, name: "c.pdf", path: "/c.pdf", sha256: "", status: "todo", level: "simple", origin_message_id: null, created: 0, signable: true,
  signers: [{ name: "Moi", email: "me@x.fr", signed: true }, { name: "Camille", email: "c@x.fr", signed: false }], ...over,
});

describe("geometry", () => {
  it("centre le rectangle sur le clic et inverse l'axe Y", () => {
    expect(placeRect(300, 200, 1, 600, 800)).toEqual([225, 800 - 200 - 25, 375, 800 - 200 + 25]);
  });
  it("tient compte de l'échelle", () => {
    const r = placeRect(300, 200, 2, 600, 800);
    expect(r[0]).toBe(75); expect(r[3] - r[1]).toBe(50);
  });
  it("borne à la page", () => {
    expect(placeRect(0, 0, 1, 600, 800)).toEqual([0, 750, 150, 800]);
    expect(placeRect(9999, 9999, 1, 600, 800)).toEqual([450, 0, 600, 50]);
  });
  it("aller-retour rect -> css", () => {
    const r = placeRect(300, 200, 1.5, 600, 800);
    const c = rectToCss(r, 1.5, 800);
    expect(c.left + c.width / 2).toBeCloseTo(300); expect(c.top + c.height / 2).toBeCloseTo(200);
  });
});

describe("format", () => {
  it("1 sur 2", () => expect(progressLabel(doc())).toBe("1 sur 2"));
  it("état et action par onglet", () => {
    expect([stateOf("todo"), stateOf("waiting"), stateOf("signed")]).toEqual(["todo", "wait", "done"]);
    expect([actionOf("todo"), actionOf("waiting"), actionOf("signed")]).toEqual(["Signer", "Relancer", "Télécharger"]);
  });
  it("meSigned", () => {
    expect(meSigned(doc(), "ME@x.fr")).toBe(true);
    expect(meSigned(doc({ signers: [{ name: "Moi", email: "me@x.fr", signed: false }] }), "me@x.fr")).toBe(false);
  });
  it("Word demande soffice, pas le PDF", () => {
    expect(needsSoffice("a.docx")).toBe(true); expect(needsSoffice("a.PDF")).toBe(false);
  });
  it("brouillon de réponse", () => {
    expect(replyDraft("/s.pdf", "a@b.fr", "Contrat", "<m1>")).toEqual({ to: "a@b.fr", subject: "Re : Contrat", body: "", in_reply_to: "<m1>", attachments: ["/s.pdf"] });
    expect(replyDraft("/s.pdf", "a@b.fr", "Re: X").subject).toBe("Re: X");
  });
});
