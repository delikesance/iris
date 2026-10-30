import { describe, expect, it } from "vitest";
import type { Action, Cond } from "../../lib/types";
import {
  actionKinds, blankItem, condKinds, isComplete, leaves, paramValue, parseRuleRoute, ruleRoute, toCond, withParam,
} from "./ruleBuilder";

describe("ruleBuilder", () => {
  it("crée des éléments vierges au format du contrat", () => {
    expect(blankItem<Cond>(condKinds, "sender_is")).toEqual({ type: "sender_is", value: "" });
    expect(blankItem<Cond>(condKinds, "has_attachment_ext")).toEqual({ type: "has_attachment_ext", ext: "" });
    expect(blankItem<Cond>(condKinds, "has_list_unsubscribe")).toEqual({ type: "has_list_unsubscribe" });
    expect(blankItem<Action>(actionKinds, "remind")).toEqual({ type: "remind", days: 3 });
    expect(() => blankItem(actionKinds, "nope")).toThrow();
  });

  it("modifie le paramètre sans muter, convertit les nombres", () => {
    const c = blankItem<Cond>(condKinds, "no_reply_after_days");
    expect(withParam(condKinds, c, "5")).toEqual({ type: "no_reply_after_days", days: 5 });
    expect(c).toEqual({ type: "no_reply_after_days", days: 3 });
    expect(paramValue(actionKinds, { type: "move_to", folder: "Clients" })).toBe("Clients");
    expect(paramValue(actionKinds, { type: "archive" })).toBe("");
  });

  it("n'est complet qu'avec un paramètre valide", () => {
    expect(isComplete(condKinds, { type: "sender_is", value: "  " })).toBe(false);
    expect(isComplete(condKinds, { type: "sender_is", value: "a.fr" })).toBe(true);
    expect(isComplete(condKinds, { type: "no_reply_after_days", days: 0 })).toBe(false);
    expect(isComplete(condKinds, { type: "no_reply_after_days", days: Number("x") })).toBe(false);
    expect(isComplete(actionKinds, { type: "notify" })).toBe(true);
    expect(isComplete(actionKinds, { type: "webhook", url: "" })).toBe(false);
  });

  it("aplatit et recompose les conditions", () => {
    const a: Cond = { type: "has_attachment_ext", ext: "pdf" };
    const b: Cond = { type: "subject_contains", value: "facture" };
    expect(toCond([a])).toEqual(a);
    expect(toCond([a, b])).toEqual({ type: "all", of: [a, b] });
    expect(leaves({ type: "all", of: [a, { type: "all", of: [b] }] })).toEqual([a, b]);
    expect(leaves(a)).toEqual([a]);
  });

  it("lit et écrit le lien profond settings/rules/<id>", () => {
    expect(ruleRoute(7)).toBe("settings/rules/7");
    expect(parseRuleRoute("settings/rules/7")).toBe(7);
    expect(parseRuleRoute("settings")).toBeNull();
    expect(parseRuleRoute("settings/rules/x")).toBeNull();
  });
});
