import { writable } from "svelte/store";
import type { AppRoute } from "./router";
import type { Contact, Draft, SigningTarget, Toast } from "./types";

export const route = writable<AppRoute>("login");
export const currentFolder = writable<string>("INBOX");
export const selectedUid = writable<number | null>(null);
export const selection = writable<Set<number>>(new Set());
/** null = composeur fermé ; sinon brouillon pré-rempli (pièces jointes incluses). */
export const composeDraft = writable<Draft | null>(null);
export const signingTarget = writable<SigningTarget | null>(null);
export const toast = writable<Toast | null>(null);
/** Contact à présélectionner à l'ouverture de l'écran Contacts. */
export const selectedContact = writable<Contact | null>(null);
/** Recherche à appliquer à l'ouverture de l'écran Mails (consommée une fois). */
export const mailQuery = writable<string>("");
