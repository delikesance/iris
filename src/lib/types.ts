// Miroir de src-tauri/src/types.rs (voir docs/api-contract.md). JSON en snake_case.
export type Security = "ssl" | "starttls" | "none";

export type Settings = {
  email: string;
  imap_host: string;
  imap_port: number;
  smtp_host: string;
  smtp_port: number;
  imap_security?: Security;
  smtp_security?: Security;
};

export type FolderKind = "system" | "sorted";
export type Folder = { name: string; label: string; kind: FolderKind; unread: number };

/** `since` / `until` : secondes Unix (UTC). */
export type Filters = { unread?: boolean; has_attach?: boolean; from?: string; since?: number; until?: number };

export type MessageRow = {
  uid: number; folder: string; from_name: string; from_addr: string; subject: string;
  date: number; seen: boolean; has_attach: boolean; thread_id: string | null;
  rule_id: number | null; rule_name: string | null; sorted_into: string | null;
};

export type Attachment = {
  index: number; filename: string; mime: string; size: number; signable: boolean;
  superseded_by?: string;
};

export type Full = {
  uid: number; folder: string; from: string; from_name: string; from_addr: string; to: string; cc: string;
  subject: string; date: number; message_id: string; thread_id: string | null;
  text: string; html: string | null; attachments: Attachment[];
  rule_id: number | null; rule_name: string | null; sorted_into: string | null;
};

export type ThreadMessage = {
  uid: number; folder: string; from_name: string; from_addr: string; to: string; subject: string;
  date: number; message_id: string; text: string; html: string | null;
  attachments: Attachment[]; superseded: boolean;
};

export type Draft = {
  to: string; cc?: string | null; subject: string; body: string; in_reply_to?: string | null;
  attachments: string[]; remind_days?: number | null; send_at?: number | null;
};

export type SendResult = { status: "sent" | "scheduled"; id: number | null };
export type ScheduledItem = { id: number; send_at: number; to: string; subject: string; draft: Draft };

export type Cond =
  | { type: "sender_is"; value: string }
  | { type: "sender_domain"; value: string }
  | { type: "subject_contains"; value: string }
  | { type: "has_attachment_ext"; ext: string }
  | { type: "has_list_unsubscribe" }
  | { type: "no_reply_after_days"; days: number }
  | { type: "all"; of: Cond[] };

export type Action =
  | { type: "move_to"; folder: string }
  | { type: "archive" }
  | { type: "remind"; days: number }
  | { type: "notify" }
  | { type: "webhook"; url: string };

export type Rule = { id: number | null; name: string; enabled: boolean; position: number; cond: Cond; action: Action };

export type ChronKind = "morning_digest" | "reminders" | "newsletter_cleanup" | "monthly_invoices";
export type Chron = {
  id: number | null; name: string; cron: string; kind: ChronKind; enabled: boolean;
  last_run: number | null; next_run: number | null; description: string;
};
export type CronDescription = { ok: boolean; description: string | null; next_runs: string[]; error: string | null };

export type RecurringKind = "newsletters" | "notifications" | "documents";
export type Cadence = "Quotidien" | "Hebdo" | "Mensuel" | "Irrégulier";
export type RecurringSender = {
  addr: string; name: string; kind: RecurringKind; cadence: Cadence; last_subject: string;
  unread: number; filed_in: string | null; can_unsubscribe: boolean;
};
export type NewSender = { addr: string; name: string; last_subject: string; count: number };
export type SenderDecision = "block" | "recurring" | "inbox";
export type UnsubscribeResult = { done: boolean; open_url: string | null };

export type Contact = { id: number | null; name: string; email: string; notes: string | null };
export type ThreadRef = { thread_id: string; subject: string; date: number };
export type ContactDetail = { contact: Contact; recent_threads: ThreadRef[]; rule: Rule | null };

export type DocumentTab = "todo" | "waiting" | "signed";
export type SignatureLevel = "simple" | "advanced" | "qualified";
export type Signer = { name: string; email: string; signed: boolean };
export type Document = {
  id: number; name: string; path: string; sha256: string; status: DocumentTab; signers: Signer[];
  level: SignatureLevel; origin_message_id: string | null; created: number; signable: boolean;
};
export type ProofEvent = { id: number; at: number; kind: "opened" | "signed" | "sealed" | "sent"; detail: string };
export type Proof = { document: Document; events: ProofEvent[] };

export type Route =
  | "login" | "mail" | "contacts" | "recurring" | "chrons" | "signatures" | "settings"
  | `settings/rules/${number}`;

export type SigningTarget = { folder: string; uid: number; index: number };
export type Toast = { kind: "info" | "error"; text: string };
