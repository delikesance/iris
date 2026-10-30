/** Aides pures du composeur : créneaux « Envoyer plus tard », pièces jointes, relance. */

export type SlotId = "tonight" | "tomorrow" | "monday" | "custom";
export type Slot = { id: SlotId; title: string; time: string; label: string; at: Date };

const DAYS = ["dimanche", "lundi", "mardi", "mercredi", "jeudi", "vendredi", "samedi"];
const MONTHS = ["janv.", "févr.", "mars", "avr.", "mai", "juin", "juil.", "août", "sept.", "oct.", "nov.", "déc."];

const pad = (n: number) => String(n).padStart(2, "0");
export const hhmm = (d: Date) => `${pad(d.getHours())}:${pad(d.getMinutes())}`;

function at(base: Date, addDays: number, h: number, m: number): Date {
  return new Date(base.getFullYear(), base.getMonth(), base.getDate() + addDays, h, m, 0, 0);
}

function slot(id: SlotId, title: string, when: Date, prefix: string): Slot {
  const time = hhmm(when);
  return { id, title, time, label: `${prefix}, ${time}`, at: when };
}

/** Créneaux rapides : ce soir 18:00 (si pas dépassé), demain 08:30, lundi 09:00 (lundi suivant, jamais aujourd'hui). */
export function quickSlots(now: Date): Slot[] {
  const out: Slot[] = [];
  const tonight = at(now, 0, 18, 0);
  if (tonight.getTime() > now.getTime()) out.push(slot("tonight", "Ce soir", tonight, "ce soir"));
  out.push(slot("tomorrow", "Demain matin", at(now, 1, 8, 30), "demain"));
  const toMonday = ((8 - now.getDay()) % 7) || 7;
  out.push(slot("monday", "Lundi", at(now, toMonday, 9, 0), "lundi"));
  return out;
}

/** Créneau choisi à la main ; null si la date est invalide ou déjà passée. */
export function customSlot(date: Date, now: Date): Slot | null {
  if (Number.isNaN(date.getTime()) || date.getTime() <= now.getTime()) return null;
  return slot("custom", "Date choisie", date, `${DAYS[date.getDay()]} ${date.getDate()} ${MONTHS[date.getMonth()]}`);
}

/** Valeur d'un <input type="datetime-local"> (heure locale) -> Date. */
export const parseLocalInput = (v: string): Date => new Date(v);

export const toUnix = (d: Date) => Math.floor(d.getTime() / 1000);

export const sendNowLabel = "Envoyer maintenant";
export const mainButtonLabel = (s: Slot | null) => (s ? `Planifier pour ${s.label}` : sendNowLabel);

export const fileName = (path: string) => path.split(/[\\/]/).pop() ?? path;

/** Une pièce issue de « Répondre avec le document signé » porte « (signé) » dans son nom. */
export const isSignedAttachment = (path: string) => /\(signé\)|[_\- ]sign(é|e|ed)\b/i.test(fileName(path));

export const reminderNote = (to: string, days: number) => {
  const who = to.split(/[,;]/)[0].replace(/<.*>/, "").trim() || "le destinataire";
  return `Si ${who} ne répond pas sous ${days} jour${days > 1 ? "s" : ""}`;
};

export const withSubjectPrefix = (subject: string) => (/^\s*re\s*:/i.test(subject) ? subject : `Re : ${subject}`);
