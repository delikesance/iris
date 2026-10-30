/** Aides pures de la vue Fil de discussion. */
import type { Attachment, Draft, ThreadMessage } from "../../lib/types";
import { withSubjectPrefix } from "../compose/slots";

export type Participant = { addr: string; name: string; count: number; me: boolean };

/** Adresse nue (minuscule) d'un champ « Nom <a@b.fr> ». */
export const addrOf = (s: string) => (s.match(/<([^>]+)>/)?.[1] ?? s).trim().toLowerCase();

/** Liste d'adresses nues d'un champ To/Cc séparé par , ou ;. */
export const addrList = (field: string) => field.split(/[,;]/).map(addrOf).filter(Boolean);

const isMe = (addr: string, me: string) => addr.toLowerCase() === me.toLowerCase();

/** Expéditeurs dans l'ordre de première apparition, avec leur nombre de messages. « Vous » pour moi. */
export function participants(messages: ThreadMessage[], me: string): Participant[] {
  const map = new Map<string, Participant>();
  for (const m of messages) {
    const addr = m.from_addr.toLowerCase();
    const p = map.get(addr) ?? { addr, name: isMe(addr, me) ? "Vous" : (m.from_name || addr), count: 0, me: isMe(addr, me) };
    p.count += 1;
    map.set(addr, p);
  }
  return [...map.values()];
}

export const legend = (messages: ThreadMessage[], ps: Participant[]) =>
  `${messages.length} message${messages.length > 1 ? "s" : ""} · ${ps.length} participant${ps.length > 1 ? "s" : ""}`;

const DAYS = ["Dimanche", "Lundi", "Mardi", "Mercredi", "Jeudi", "Vendredi", "Samedi"];
const MONTHS = ["janvier", "février", "mars", "avril", "mai", "juin", "juillet", "août", "septembre", "octobre", "novembre", "décembre"];

export function dayLabel(unix: number): string {
  const d = new Date(unix * 1000);
  return `${DAYS[d.getDay()]} ${d.getDate()} ${MONTHS[d.getMonth()]}`;
}

export function timeLabel(unix: number): string {
  const d = new Date(unix * 1000);
  return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
}

export type DayGroup = { label: string; items: ThreadMessage[] };

export function groupByDay(messages: ThreadMessage[]): DayGroup[] {
  const groups: DayGroup[] = [];
  for (const m of messages) {
    const label = dayLabel(m.date);
    const last = groups[groups.length - 1];
    if (last?.label === label) last.items.push(m);
    else groups.push({ label, items: [m] });
  }
  return groups;
}

/** Ensemble des messages dépliés par défaut : les deux derniers. */
export const defaultOpen = (messages: ThreadMessage[]) => new Set(messages.slice(-2).map(m => m.uid));

/** Texte propre / citation (lignes « > » ou à partir de « Le … a écrit : »). */
export function splitQuoted(text: string): { body: string; quoted: string } {
  const lines = text.split("\n");
  const at = lines.findIndex(l => /^>/.test(l) || /^(Le .{5,}a écrit\s*:|On .{5,}wrote:)\s*$/i.test(l.trim()));
  if (at < 0) return { body: text, quoted: "" };
  return { body: lines.slice(0, at).join("\n").trimEnd(), quoted: lines.slice(at).join("\n").trim() };
}

const versionOf = (name: string) => name.replace(/\.[^.]+$/, "").match(/(?:[_\- ]v|[_\- ]rev|[_\-])(\d+)$/i)?.[1];

/** « Remplacée par la v3, non signable » (la version vient du nom de la pièce qui remplace). */
export function supersededLabel(replacedBy: string | undefined): string {
  const v = replacedBy ? versionOf(replacedBy) : undefined;
  return v ? `Remplacée par la v${v}, non signable` : "Remplacée par une version plus récente, non signable";
}

export const isSuperseded = (a: Attachment, m: ThreadMessage) => !!a.superseded_by || m.superseded;

/** Seule la dernière version d'un fichier signable propose « Signer ». */
export const canSign = (a: Attachment, m: ThreadMessage) => a.signable && !isSuperseded(a, m);

/** Brouillon « Répondre à tous » : destinataire = expéditeur du dernier message (ou ses destinataires si c'est moi), Cc = les autres. */
export function replyAllDraft(messages: ThreadMessage[], me: string): Draft {
  const last = messages[messages.length - 1];
  const from = last.from_addr.toLowerCase();
  const primary = isMe(from, me) ? addrList(last.to).filter(a => !isMe(a, me)) : [from];
  const all = messages.flatMap(m => [m.from_addr.toLowerCase(), ...addrList(m.to)]);
  const cc = [...new Set(all)].filter(a => !isMe(a, me) && !primary.includes(a));
  return {
    to: primary.join(", "), cc: cc.length ? cc.join(", ") : null, subject: withSubjectPrefix(last.subject),
    body: "", in_reply_to: last.message_id, attachments: [],
  };
}

export const pendingReminder = (messages: ThreadMessage[], me: string, days: number | null | undefined) =>
  days != null && messages.length > 0 && isMe(messages[messages.length - 1].from_addr, me)
    ? `Relance automatique dans ${days} jour${days > 1 ? "s" : ""} sans réponse`
    : null;
