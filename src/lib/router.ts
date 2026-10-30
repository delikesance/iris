import type { Route } from "./types";

export type Screen = "login" | "thread" | "mail" | "contacts" | "recurring" | "chrons" | "signatures" | "settings";
/** Route de l'application : celles de `Route` plus `thread/<id>` (fil de discussion). */
export type AppRoute = Route | `thread/${string}`;
export type NavEntry = { route: Exclude<Route, "login" | `settings/${string}`>; label: string };

/** Entrées de la barre latérale, dans l'ordre d'affichage. */
export const NAV: NavEntry[] = [
  { route: "mail", label: "Mails" },
  { route: "contacts", label: "Contacts" },
  { route: "recurring", label: "Récurrents" },
  { route: "chrons", label: "Chrons" },
  { route: "signatures", label: "Signatures" },
  { route: "settings", label: "Réglages" },
];

/** Écran qui affiche une route (`settings/rules/3` → `settings`). */
export function screenOf(route: AppRoute): Screen {
  return route.split("/")[0] as Screen;
}

/** Identifiant de règle d'une route `settings/rules/<id>`, sinon null. */
export function ruleIdOf(route: AppRoute): number | null {
  const m = /^settings\/rules\/(\d+)$/.exec(route);
  return m ? Number(m[1]) : null;
}

export const threadRoute = (id: string): AppRoute => `thread/${encodeURIComponent(id)}`;

/** Identifiant de fil d'une route `thread/<id>`, sinon null. */
export function threadIdOf(route: AppRoute): string | null {
  return route.startsWith("thread/") ? decodeURIComponent(route.slice(7)) : null;
}

/** L'entrée de la barre latérale est active pour sa propre route et ses sous-routes. */
export const isActive = (entry: NavEntry, route: AppRoute): boolean => screenOf(route) === entry.route;
