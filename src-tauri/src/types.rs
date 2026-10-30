//! Types partagés du contrat IPC (voir docs/api-contract.md). JSON en snake_case.
use serde::{Deserialize, Serialize};
use serde_json::Value;

// ---------- Mail ----------

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum FolderKind { System, Sorted }

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Folder { pub name: String, pub label: String, pub kind: FolderKind, pub unread: u32 }

#[derive(Serialize, Deserialize, Clone, Default, Debug)]
pub struct Filters {
    pub unread: Option<bool>,
    pub has_attach: Option<bool>,
    pub from: Option<String>,
    /// Secondes Unix (UTC), borne incluse.
    pub since: Option<i64>,
    pub until: Option<i64>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct MessageRow {
    pub uid: u32,
    pub folder: String,
    pub from_name: String,
    pub from_addr: String,
    pub subject: String,
    /// Secondes Unix (UTC).
    pub date: i64,
    pub seen: bool,
    pub has_attach: bool,
    pub thread_id: Option<String>,
    pub rule_id: Option<i64>,
    pub rule_name: Option<String>,
    pub sorted_into: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Attachment {
    pub index: u32,
    pub filename: String,
    pub mime: String,
    pub size: u64,
    pub signable: bool,
    /// Nom du fichier qui remplace celui-ci dans le fil (ex. contrat_v3.pdf), sinon None.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub superseded_by: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Full {
    pub uid: u32,
    pub folder: String,
    pub from: String,
    pub from_name: String,
    pub from_addr: String,
    pub to: String,
    pub cc: String,
    pub subject: String,
    pub date: i64,
    pub message_id: String,
    pub thread_id: Option<String>,
    pub text: String,
    pub html: Option<String>,
    pub attachments: Vec<Attachment>,
    pub rule_id: Option<i64>,
    pub rule_name: Option<String>,
    pub sorted_into: Option<String>,
}

/// Message d'un fil : `Full` sans règle, plus l'indicateur `superseded`.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ThreadMessage {
    pub uid: u32,
    pub folder: String,
    pub from_name: String,
    pub from_addr: String,
    pub to: String,
    pub subject: String,
    pub date: i64,
    pub message_id: String,
    pub text: String,
    pub html: Option<String>,
    pub attachments: Vec<Attachment>,
    /// Vrai si une pièce jointe de ce message est remplacée par une version plus récente du fil.
    pub superseded: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Draft {
    pub to: String,
    pub cc: Option<String>,
    pub subject: String,
    pub body: String,
    pub in_reply_to: Option<String>,
    /// Chemins locaux des pièces jointes.
    #[serde(default)]
    pub attachments: Vec<String>,
    pub remind_days: Option<u32>,
    /// Secondes Unix ; absent = envoi immédiat.
    pub send_at: Option<i64>,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum SendStatus { Sent, Scheduled }

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SendResult { pub status: SendStatus, pub id: Option<i64> }

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ScheduledItem { pub id: i64, pub send_at: i64, pub to: String, pub subject: String, pub draft: Draft }

// ---------- Automatisation ----------

/// `cond` et `action` suivent les enums `rules::Cond` / `rules::Action` (champ `type`, snake_case) ;
/// ils restent des `Value` ici pour que le contrat ne dépende pas de rules.rs.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Rule {
    pub id: Option<i64>,
    pub name: String,
    pub enabled: bool,
    pub position: i64,
    pub cond: Value,
    pub action: Value,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum ChronKind { MorningDigest, Reminders, NewsletterCleanup, MonthlyInvoices }

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Chron {
    pub id: Option<i64>,
    pub name: String,
    /// Expression cron à 5 champs.
    pub cron: String,
    pub kind: ChronKind,
    pub enabled: bool,
    pub last_run: Option<i64>,
    pub next_run: Option<i64>,
    /// Périodicité en français, calculée par le backend.
    #[serde(default)]
    pub description: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct CronDescription {
    pub ok: bool,
    pub description: Option<String>,
    /// Trois prochaines exécutions, ISO 8601.
    pub next_runs: Vec<String>,
    pub error: Option<String>,
}

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum RecurringKind { Newsletters, Notifications, Documents }

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RecurringSender {
    pub addr: String,
    pub name: String,
    pub kind: RecurringKind,
    /// Quotidien | Hebdo | Mensuel | Irrégulier
    pub cadence: String,
    pub last_subject: String,
    pub unread: u32,
    pub filed_in: Option<String>,
    pub can_unsubscribe: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct NewSender { pub addr: String, pub name: String, pub last_subject: String, pub count: u32 }

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum SenderDecision { Block, Recurring, Inbox }

/// `done` = désinscription faite par Iris ; sinon `open_url` à ouvrir dans le navigateur après confirmation.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct UnsubscribeResult { pub done: bool, pub open_url: Option<String> }

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Contact { pub id: Option<i64>, pub name: String, pub email: String, pub notes: Option<String> }

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ThreadRef { pub thread_id: String, pub subject: String, pub date: i64 }

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ContactDetail { pub contact: Contact, pub recent_threads: Vec<ThreadRef>, pub rule: Option<Rule> }

// ---------- Signature ----------

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum DocumentTab { Todo, Waiting, Signed }

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Debug)]
#[serde(rename_all = "snake_case")]
pub enum SignatureLevel { Simple, Advanced, Qualified }

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Signer { pub name: String, pub email: String, pub signed: bool }

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Document {
    pub id: i64,
    pub name: String,
    pub path: String,
    pub sha256: String,
    pub status: DocumentTab,
    pub signers: Vec<Signer>,
    pub level: SignatureLevel,
    pub origin_message_id: Option<String>,
    pub created: i64,
    /// Faux si le document est remplacé par une version plus récente.
    pub signable: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ProofEvent {
    pub id: i64,
    pub at: i64,
    /// opened | signed | sealed | sent
    pub kind: String,
    pub detail: String,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Proof { pub document: Document, pub events: Vec<ProofEvent> }
