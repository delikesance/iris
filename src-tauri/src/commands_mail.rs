//! Commandes mail : fines enveloppes async autour de `mail.rs` (E/S bloquantes dans `spawn_blocking`).
use crate::types::*;
use crate::{config, mail};
use tauri::Emitter;

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string())?
}

#[tauri::command]
pub fn account() -> Result<String, String> { config::load().map(|c| c.user) }

#[tauri::command]
pub fn saved_settings() -> Option<config::Settings> { config::read_settings() }

#[tauri::command]
pub async fn login(settings: config::Settings, password: String) -> Result<(), String> {
    blocking(move || {
        mail::test_login(&config::Config::from_settings(&settings, &password))?;
        config::save(&settings, &password)
    }).await
}

#[tauri::command]
pub fn logout() { config::clear() }

#[tauri::command]
pub async fn list_folders() -> Result<Vec<Folder>, String> { blocking(mail::folders).await }

/// Rafraîchit le cache puis émet `mail://changed` (payload : le dossier).
#[tauri::command]
pub async fn sync_folder(app: tauri::AppHandle, folder: String) -> Result<(), String> {
    let name = folder.clone();
    blocking(move || mail::sync_folder(&name).map(|_| ())).await?;
    app.emit("mail://changed", folder).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn list_messages(folder: String, query: Option<String>, filters: Option<Filters>) -> Result<Vec<MessageRow>, String> {
    blocking(move || mail::messages(&folder, query.as_deref(), filters.as_ref())).await
}

#[tauri::command]
pub async fn get_message(folder: String, uid: u32) -> Result<Full, String> { blocking(move || mail::message(&folder, uid)).await }

#[tauri::command]
pub async fn thread_get(thread_id: String) -> Result<Vec<ThreadMessage>, String> { blocking(move || mail::thread(&thread_id)).await }

#[tauri::command]
pub async fn save_attachment(folder: String, uid: u32, index: u32, dest_path: String) -> Result<(), String> {
    blocking(move || mail::save_attachment(&folder, uid, index, &dest_path)).await
}

#[tauri::command]
pub async fn mark_seen(folder: String, uid: u32, seen: bool) -> Result<(), String> { blocking(move || mail::mark_seen(&folder, uid, seen)).await }

#[tauri::command]
pub async fn bulk_move(folder: String, uids: Vec<u32>, target: String) -> Result<(), String> {
    blocking(move || mail::bulk_move(&folder, &uids, &target)).await
}

#[tauri::command]
pub async fn bulk_delete(folder: String, uids: Vec<u32>) -> Result<(), String> { blocking(move || mail::bulk_delete(&folder, &uids)).await }

#[tauri::command]
pub async fn delete_message(folder: String, uid: u32) -> Result<(), String> { blocking(move || mail::bulk_delete(&folder, &[uid])).await }

#[tauri::command]
pub async fn send_mail(draft: Draft) -> Result<SendResult, String> { blocking(move || mail::send(&draft)).await }

#[tauri::command]
pub async fn scheduled_list() -> Result<Vec<ScheduledItem>, String> { blocking(|| mail::with_db(mail::scheduled_pending)).await }

#[tauri::command]
pub async fn scheduled_cancel(id: i64) -> Result<(), String> { blocking(move || mail::with_db(|c| mail::scheduled_cancel(c, id))).await }
