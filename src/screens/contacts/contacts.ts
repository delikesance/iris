import type { Draft } from "../../lib/types";

/** Brouillon vierge adressé à un contact. */
export function draftTo(email: string): Draft {
  return { to: email, subject: "", body: "", attachments: [] };
}

export function validContact(name: string, email: string): boolean {
  return name.trim() !== "" && /^[^\s@]+@[^\s@]+\.[^\s@]+$/.test(email.trim());
}

export function formatDay(unix: number, now = new Date()): string {
  const d = new Date(unix * 1000);
  const days = Math.round((new Date(now.getFullYear(), now.getMonth(), now.getDate()).getTime()
    - new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime()) / 86400000);
  if (days === 0) return "Aujourd'hui";
  if (days === 1) return "Hier";
  return d.toLocaleDateString("fr-FR", { day: "numeric", month: "short" });
}
