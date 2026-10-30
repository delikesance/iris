import type { Action, Cond, Route } from "../../lib/types";

/** Paramètre unique d'une condition ou d'une action (la clé JSON qui le porte). */
export type Param = { key: string; kind: "text" | "number"; placeholder: string; initial: string | number };
export type Kind = { type: string; label: string; param?: Param };

const text = (key: string, placeholder: string): Param => ({ key, kind: "text", placeholder, initial: "" });
const days = (initial: number): Param => ({ key: "days", kind: "number", placeholder: "jours", initial });

export const condKinds: Kind[] = [
  { type: "sender_is", label: "l'expéditeur est", param: text("value", "atelier-norge.fr") },
  { type: "sender_domain", label: "le domaine de l'expéditeur est", param: text("value", "exemple.fr") },
  { type: "subject_contains", label: "l'objet contient", param: text("value", "facture") },
  { type: "has_attachment_ext", label: "une pièce jointe est de type", param: text("ext", "pdf") },
  { type: "has_list_unsubscribe", label: "le message contient un lien de désinscription" },
  { type: "no_reply_after_days", label: "je n'ai pas répondu après", param: days(3) },
];

export const actionKinds: Kind[] = [
  { type: "move_to", label: "classer dans", param: text("folder", "Clients") },
  { type: "archive", label: "archiver" },
  { type: "remind", label: "relancer après", param: days(3) },
  { type: "notify", label: "notifier" },
  { type: "webhook", label: "appeler un webhook", param: text("url", "https://…") },
];

type Item = Cond | Action;
const bag = (item: Item) => item as unknown as Record<string, string | number>;
const kindOf = (kinds: Kind[], item: Item) => kinds.find((k) => k.type === item.type);

export function blankItem<T extends Item>(kinds: Kind[], type: string): T {
  const kind = kinds.find((k) => k.type === type);
  if (!kind) throw new Error(`type inconnu : ${type}`);
  return (kind.param ? { type, [kind.param.key]: kind.param.initial } : { type }) as unknown as T;
}

export function paramValue(kinds: Kind[], item: Item): string {
  const p = kindOf(kinds, item)?.param;
  return p ? String(bag(item)[p.key] ?? "") : "";
}

export function withParam<T extends Item>(kinds: Kind[], item: T, raw: string): T {
  const p = kindOf(kinds, item)?.param;
  if (!p) return item;
  return { ...item, [p.key]: p.kind === "number" ? Number(raw) : raw };
}

export function isComplete(kinds: Kind[], item: Item): boolean {
  const p = kindOf(kinds, item)?.param;
  if (!kindOf(kinds, item)) return false;
  if (!p) return true;
  const v = bag(item)[p.key];
  return p.kind === "number" ? Number.isInteger(v) && (v as number) > 0 : String(v ?? "").trim() !== "";
}

/** Aplatit une condition composée en liste de conditions simples. */
export function leaves(cond: Cond): Cond[] {
  return cond.type === "all" ? cond.of.flatMap(leaves) : [cond];
}

/** Une condition seule reste telle quelle ; plusieurs sont réunies (toutes vraies). */
export function toCond(list: Cond[]): Cond {
  return list.length === 1 ? list[0] : { type: "all", of: list };
}

export const ruleRoute = (id: number): Route => `settings/rules/${id}`;

export function parseRuleRoute(route: string): number | null {
  const m = /^settings\/rules\/(\d+)$/.exec(route);
  return m ? Number(m[1]) : null;
}
