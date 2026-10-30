import type { RecurringKind, RecurringSender } from "../../lib/types";

export type Tab = "all" | RecurringKind;

export const TABS: { id: Tab; label: string }[] = [
  { id: "all", label: "Tous" },
  { id: "newsletters", label: "Newsletters" },
  { id: "notifications", label: "Notifications" },
  { id: "documents", label: "Documents" },
];

export const KIND_LABEL: Record<RecurringKind, string> = {
  newsletters: "Newsletters",
  notifications: "Notifications",
  documents: "Documents",
};

const KINDS: RecurringKind[] = ["newsletters", "notifications", "documents"];

export function countFor(rows: RecurringSender[], tab: Tab): number {
  return tab === "all" ? rows.length : rows.filter(r => r.kind === tab).length;
}

export function tabItems(rows: RecurringSender[]) {
  return TABS.map(t => ({ id: t.id, label: `${t.label} ${countFor(rows, t.id)}` }));
}

/** Groupes non vides, dans l'ordre Newsletters, Notifications, Documents. */
export function groupRows(rows: RecurringSender[], tab: Tab): { kind: RecurringKind; rows: RecurringSender[] }[] {
  return KINDS.filter(k => tab === "all" || tab === k)
    .map(kind => ({ kind, rows: rows.filter(r => r.kind === kind) }))
    .filter(g => g.rows.length > 0);
}

export function unreadLabel(n: number): string {
  return n === 1 ? "1 non lu" : `${n} non lus`;
}
