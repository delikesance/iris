import type { Security, Settings } from "../../lib/types";

export type Proto = "imap" | "smtp";

export const SECURITY_OPTIONS: { id: Security; label: string }[] = [
  { id: "ssl", label: "SSL/TLS" },
  { id: "starttls", label: "STARTTLS" },
  { id: "none", label: "Aucune" },
];

const PORTS: Record<Proto, Record<Security, number>> = {
  imap: { ssl: 993, starttls: 143, none: 143 },
  smtp: { ssl: 465, starttls: 587, none: 25 },
};

export const defaultPort = (proto: Proto, security: Security): number => PORTS[proto][security];

/**
 * Port à afficher après un changement de sécurité : suit la valeur par défaut
 * sauf si l'utilisateur a saisi un port personnalisé.
 */
export function portAfterSecurityChange(proto: Proto, from: Security, to: Security, port: number): number {
  return port === defaultPort(proto, from) ? defaultPort(proto, to) : port;
}

export const DEFAULT_SERVERS = { imap_host: "imap.fastmail.com", smtp_host: "smtp.fastmail.com" };

export function defaultSettings(email = ""): Settings {
  return {
    email, ...DEFAULT_SERVERS,
    imap_port: defaultPort("imap", "ssl"), smtp_port: defaultPort("smtp", "ssl"),
    imap_security: "ssl", smtp_security: "ssl",
  };
}

export const serversSummary = (s: Settings): string => `${s.imap_host} · ${s.smtp_host}`;

export function validPort(p: number): boolean {
  return Number.isInteger(p) && p > 0 && p < 65536;
}
