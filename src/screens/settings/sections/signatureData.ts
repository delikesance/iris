export const PREFIX = "data:image/png;base64,";

/** Retire l'en-tête d'une URL de données PNG : le backend attend le base64 seul. */
export function toBase64(dataUrl: string): string {
  if (!dataUrl.startsWith(PREFIX)) throw new Error("image PNG attendue");
  return dataUrl.slice(PREFIX.length);
}
