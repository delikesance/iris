import type { Chron, ChronKind } from "../../lib/types";

/** Tâches que le planificateur sait exécuter (le contrat n'en connaît pas d'autres). */
export const kindOptions: { kind: ChronKind; label: string; about: string }[] = [
  { kind: "morning_digest", label: "Résumé du matin", about: "M'envoyer les messages importants de la nuit." },
  { kind: "reminders", label: "Relances", about: "Relancer les messages restés sans réponse." },
  { kind: "newsletter_cleanup", label: "Ménage des newsletters", about: "Archiver les newsletters lues depuis plus d'une semaine." },
  { kind: "monthly_invoices", label: "Factures du mois", about: "Regrouper les factures reçues et les transmettre à la comptabilité." },
];

export const aboutKind = (kind: ChronKind) => kindOptions.find((o) => o.kind === kind)?.about ?? "";

export function blankChron(): Chron {
  return {
    id: null, name: "", cron: "0 8 * * 1-5", kind: "morning_digest", enabled: true,
    last_run: null, next_run: null, description: "",
  };
}

const nextFormat = new Intl.DateTimeFormat("fr-FR", { weekday: "long", day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" });

/** « lundi 2 nov., 08:00 » ; « En pause » si le chron est désactivé ou sans prochaine date. */
export function nextRunLabel(chron: Pick<Chron, "enabled" | "next_run">): string {
  if (!chron.enabled || chron.next_run == null) return "En pause";
  return `Prochain passage : ${nextFormat.format(new Date(chron.next_run * 1000)).replace(" à ", ", ")}`;
}

/** Date ISO renvoyée par cron_describe, au même format. */
export function isoLabel(iso: string): string {
  return nextFormat.format(new Date(iso)).replace(" à ", ", ");
}

export const canSave = (name: string, ok: boolean) => name.trim() !== "" && ok;
