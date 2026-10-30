import type { Document, Draft, DocumentTab } from "../../lib/types";

export const isPdfName = (name: string) => /\.pdf$/i.test(name);
/** Word/ODT : converti en PDF par LibreOffice, donc dépend de `soffice`. */
export const needsSoffice = (name: string) => !isPdfName(name);
export const SOFFICE_MESSAGE =
  "Ce document n'est pas un PDF. Pour le signer, Iris le convertit avec LibreOffice, qui n'est pas installé sur cet ordinateur. Installez LibreOffice ou enregistrez le document en PDF.";

export const signedCount = (d: Pick<Document, "signers">) => d.signers.filter(s => s.signed).length;
export const progressLabel = (d: Pick<Document, "signers">) => `${signedCount(d)} sur ${d.signers.length}`;

export function meSigned(d: Document, myEmail: string): boolean {
  const me = d.signers.find(s => s.email.toLowerCase() === myEmail.toLowerCase());
  return me ? me.signed : d.status !== "todo";
}

export function stateOf(tab: DocumentTab): "todo" | "wait" | "done" {
  return tab === "todo" ? "todo" : tab === "waiting" ? "wait" : "done";
}
export function actionOf(tab: DocumentTab): "Signer" | "Relancer" | "Télécharger" {
  return tab === "todo" ? "Signer" : tab === "waiting" ? "Relancer" : "Télécharger";
}

export function replyDraft(path: string, to: string, subject: string, inReplyTo?: string | null): Draft {
  return { to, subject: /^re\s*:/i.test(subject) ? subject : `Re : ${subject}`, body: "", in_reply_to: inReplyTo ?? null, attachments: [path] };
}

export function formatTime(at: number): string {
  return new Date(at * 1000).toLocaleString("fr-FR", { day: "numeric", month: "short", hour: "2-digit", minute: "2-digit" });
}
