//! Commandes automatisation (règles, chrons, récurrents, contacts) : fines enveloppes async autour de
//! `automation.rs` (E/S bloquantes dans `spawn_blocking`).
use crate::types::*;
use crate::{automation, mail};
use chrono::Local;
use rusqlite::Connection;
use serde_json::Value;

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string())?
}

/// Exécute `f` sur la base partagée, hors du thread async.
async fn db<T: Send + 'static>(f: impl FnOnce(&Connection) -> Result<T, String> + Send + 'static) -> Result<T, String> {
    blocking(move || mail::with_db(f)).await
}

#[tauri::command]
pub async fn rules_list() -> Result<Vec<Rule>, String> { db(automation::rules_list).await }

#[tauri::command]
pub async fn rule_save(rule: Rule) -> Result<Rule, String> { db(move |c| automation::rule_save(c, rule)).await }

#[tauri::command]
pub async fn rule_delete(id: i64) -> Result<(), String> { db(move |c| automation::rule_delete(c, id)).await }

#[tauri::command]
pub async fn rule_toggle(id: i64, enabled: bool) -> Result<(), String> { db(move |c| automation::rule_toggle(c, id, enabled)).await }

#[tauri::command]
pub async fn rule_sentence(cond: Value, action: Value) -> Result<String, String> { automation::rule_sentence(&cond, &action) }

#[tauri::command]
pub async fn chrons_list() -> Result<Vec<Chron>, String> { db(|c| automation::chrons_list(c, &Local::now())).await }

#[tauri::command]
pub async fn chron_save(chron: Chron) -> Result<Chron, String> { db(move |c| automation::chron_save(c, chron, &Local::now())).await }

#[tauri::command]
pub async fn chron_toggle(id: i64, enabled: bool) -> Result<(), String> { db(move |c| automation::chron_toggle(c, id, enabled, mail::now())).await }

#[tauri::command]
pub async fn chron_delete(id: i64) -> Result<(), String> { db(move |c| automation::chron_delete(c, id)).await }

#[tauri::command]
pub async fn cron_describe(expr: String) -> Result<CronDescription, String> { Ok(automation::cron_describe(&expr)) }

#[tauri::command]
pub async fn recurring_list(kind: Option<RecurringKind>) -> Result<Vec<RecurringSender>, String> {
    blocking(move || { let acc = automation::account()?; mail::with_db(|c| automation::recurring_list(c, &acc, kind)) }).await
}

#[tauri::command]
pub async fn new_senders_list() -> Result<Vec<NewSender>, String> {
    blocking(|| { let acc = automation::account()?; mail::with_db(|c| automation::new_senders_list(c, &acc)) }).await
}

/// `block` crée une règle vers les indésirables et y range les messages déjà en Réception.
#[tauri::command]
pub async fn sender_decide(addr: String, decision: SenderDecision) -> Result<(), String> {
    blocking(move || automation::decide_and_apply(&addr, decision)).await
}

#[tauri::command]
pub async fn unsubscribe(addr: String) -> Result<UnsubscribeResult, String> { blocking(move || automation::unsubscribe(&addr)).await }

#[tauri::command]
pub async fn contacts_list(query: Option<String>) -> Result<Vec<Contact>, String> { db(move |c| automation::contacts_list(c, query.as_deref())).await }

#[tauri::command]
pub async fn contact_get(email: String) -> Result<ContactDetail, String> {
    blocking(move || { let acc = automation::account()?; mail::with_db(|c| automation::contact_get(c, &acc, &email)) }).await
}

#[tauri::command]
pub async fn contact_save(contact: Contact) -> Result<Contact, String> { db(move |c| automation::contact_save(c, contact)).await }
