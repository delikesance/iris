import { writable } from "svelte/store";
/** Document à ouvrir sur la page détail (posé par « Voir le détail », lu par SignaturesScreen). */
export const openDocumentId = writable<number | null>(null);
