// Fonctions pures de l'écran Mails : filtres, regroupement par jour, formats, brouillons de réponse.
import type { Draft, Full, MessageRow } from "../../lib/types";

export type FilterMode = "all" | "unread" | "attach" | "sorted";
export type FilterState = { mode: FilterMode; query: string };
export type DayGroup = { key: string; label: string; items: MessageRow[] };

export const FILTER_MODES: { mode: FilterMode; label: string }[] = [
  { mode: "all", label: "Tous les messages" },
  { mode: "unread", label: "Non lus" },
  { mode: "attach", label: "Avec pièce jointe" },
  { mode: "sorted", label: "Triés automatiquement" },
];

export const QUICK_REPLIES: { label: string; body: string }[] = [
  { label: "Bien reçu, merci", body: "Bien reçu, merci. Je reviens vers vous rapidement." },
  { label: "Je regarde ça", body: "Je regarde cela et je vous réponds dès que possible." },
  { label: "Pas disponible", body: "Je ne suis pas disponible sur ce créneau. Pouvez-vous me proposer une autre date ?" },
];

const fold = (s: string) => s.normalize("NFD").replace(/[\u0300-\u036f]/g, "").toLowerCase();

const isSorted = (m: MessageRow) => m.rule_id !== null || m.sorted_into !== null;

const modeMatches: Record<FilterMode, (m: MessageRow) => boolean> = {
  all: () => true,
  unread: (m) => !m.seen,
  attach: (m) => m.has_attach,
  sorted: isSorted,
};

/** Filtre par critère puis par recherche (sujet, nom ou adresse de l'expéditeur). */
export function applyFilters(messages: MessageRow[], { mode, query }: FilterState): MessageRow[] {
  const q = fold(query.trim());
  return messages.filter(
    (m) => modeMatches[mode](m) && (!q || fold(`${m.subject} ${m.from_name} ${m.from_addr}`).includes(q)),
  );
}

const pad = (n: number) => String(n).padStart(2, "0");
const dayKey = (d: Date) => `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;

function dayLabel(d: Date, now: Date): string {
  const key = dayKey(d);
  if (key === dayKey(now)) return "Aujourd'hui";
  if (key === dayKey(new Date(now.getFullYear(), now.getMonth(), now.getDate() - 1))) return "Hier";
  return d.toLocaleDateString("fr-FR", { day: "numeric", month: "long", year: "numeric" });
}

/** Regroupe par jour local, du plus récent au plus ancien (dates en secondes Unix). */
export function groupByDay(messages: MessageRow[], now: Date = new Date()): DayGroup[] {
  const groups = new Map<string, DayGroup>();
  for (const m of [...messages].sort((a, b) => b.date - a.date)) {
    const d = new Date(m.date * 1000);
    const key = dayKey(d);
    const group = groups.get(key) ?? { key, label: dayLabel(d, now), items: [] };
    group.items.push(m);
    groups.set(key, group);
  }
  return [...groups.values()];
}

export const formatTime = (ts: number) =>
  new Date(ts * 1000).toLocaleTimeString("fr-FR", { hour: "2-digit", minute: "2-digit" });

export const formatDateTime = (ts: number) =>
  new Date(ts * 1000).toLocaleString("fr-FR", { day: "numeric", month: "long", hour: "2-digit", minute: "2-digit" });

export function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} o`;
  if (bytes < 1024 * 1024) return `${Math.round(bytes / 1024)} Ko`;
  return `${(bytes / 1024 / 1024).toFixed(1).replace(".", ",")} Mo`;
}

export const fileExt = (filename: string) => (filename.split(".").pop() ?? "").slice(0, 4).toUpperCase();

export const replySubject = (subject: string) => (/^\s*re\s*:/i.test(subject) ? subject : `Re : ${subject}`);

const quote = (m: Full) =>
  `\n\nLe ${formatDateTime(m.date)}, ${m.from_name || m.from_addr} a écrit :\n` +
  m.text.split("\n").map((l) => `> ${l}`).join("\n");

/** Brouillon de réponse ; `intro` est placé avant la citation (réponses rapides). */
export function replyDraft(m: Full, intro = ""): Draft {
  return {
    to: m.from_addr,
    subject: replySubject(m.subject),
    body: intro + quote(m),
    in_reply_to: m.message_id,
    attachments: [],
  };
}

const escapeHtml = (s: string) => s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");

/** Document de l'iframe sandboxée : aucun script, aucune requête réseau (CSP), images `data:` seulement. */
export function messageSrcdoc(m: Pick<Full, "html" | "text">): string {
  const csp = `<meta http-equiv="Content-Security-Policy" content="default-src 'none'; img-src data:; style-src 'unsafe-inline'">`;
  const base = `<base target="_blank">`;
  if (m.html) {
    const light = `<style>body{margin:16px;background:#fff;color:#111;font:15px/1.5 system-ui,sans-serif}</style>`;
    return `<!doctype html><html><head><meta charset="utf-8">${csp}${base}${light}</head><body>${m.html}</body></html>`;
  }
  const style = `<style>body{margin:16px;font:15px/1.6 system-ui,sans-serif;color:#E9E9F2;background:#111126}pre{white-space:pre-wrap;font:inherit;margin:0}</style>`;
  return `<!doctype html><html><head><meta charset="utf-8">${csp}${style}</head><body><pre>${escapeHtml(m.text)}</pre></body></html>`;
}
