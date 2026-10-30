//! Commandes signature électronique : fines enveloppes autour de `signing_cmds`.
use crate::signing_cmds as core;
use crate::types::*;
use crate::{commands_mail, signing};
use rusqlite::Connection;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

fn now() -> i64 { chrono::Utc::now().timestamp() }

fn documents_dir(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app.path().config_dir().map_err(|e| e.to_string())?.join("iris").join("documents"))
}

/// Exécute `f` sur un thread bloquant avec la base `<config>/iris/iris.db` et le dossier des documents.
async fn run<T: Send + 'static>(
    app: AppHandle,
    f: impl FnOnce(&Connection, &Path, i64) -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    let dir = documents_dir(&app)?;
    let db_path = dir.parent().map(|p| p.join("iris.db")).ok_or("dossier de configuration introuvable")?;
    tauri::async_runtime::spawn_blocking(move || f(&crate::db::open(&db_path)?, &dir, now()))
        .await
        .map_err(|e| e.to_string())?
}

#[tauri::command]
pub async fn documents_list(app: AppHandle, tab: DocumentTab) -> Result<Vec<Document>, String> {
    run(app, move |c, _, _| core::list(c, tab)).await
}

#[tauri::command]
pub async fn document_get(app: AppHandle, id: i64) -> Result<Document, String> {
    run(app, move |c, _, _| core::get(c, id)).await
}

#[tauri::command]
pub async fn document_import(app: AppHandle, path: String) -> Result<Document, String> {
    run(app, move |c, d, t| core::import_file(c, d, Path::new(&path), None, t)).await
}

#[tauri::command]
pub async fn sign_prepare_attachment(app: AppHandle, folder: String, uid: u32, index: u32) -> Result<Document, String> {
    let msg = commands_mail::get_message(folder.clone(), uid).await?;
    let att = msg.attachments.iter().find(|a| a.index == index).ok_or("pièce jointe introuvable")?.clone();
    if let Some(newer) = &att.superseded_by {
        return Err(format!("« {} » est remplacé par {newer} et n'est plus signable", att.filename));
    }
    if !att.signable || !signing::is_signable(&att.mime, &att.filename) {
        return Err(format!("« {} » n'est pas signable", att.filename));
    }
    let tmp = documents_dir(&app)?.join(format!("incoming_{uid}_{index}"));
    std::fs::create_dir_all(&tmp).map_err(|e| e.to_string())?;
    let file = tmp.join(att.filename.rsplit(['/', '\\']).next().unwrap_or("document"));
    let saved = commands_mail::save_attachment(folder, uid, index, file.to_string_lossy().into_owned()).await;
    let origin = msg.message_id.clone();
    let result = match saved {
        Ok(()) => run(app, move |c, d, t| core::import_file(c, d, &file, Some(&origin), t)).await,
        Err(e) => Err(e),
    };
    let _ = std::fs::remove_dir_all(&tmp);
    result
}

#[tauri::command]
pub async fn sign_apply(app: AppHandle, doc_id: i64, page: u32, rect: [f32; 4], signature_png_b64: String) -> Result<Document, String> {
    run(app, move |c, d, t| core::apply(c, d, doc_id, page, rect, &signature_png_b64, t)).await
}

#[tauri::command]
pub async fn document_export_for_reply(app: AppHandle, doc_id: i64) -> Result<String, String> {
    run(app, move |c, _, t| core::export_for_reply(c, doc_id, t)).await
}

#[tauri::command]
pub async fn proof_get(app: AppHandle, doc_id: i64) -> Result<Proof, String> {
    run(app, move |c, _, _| core::proof(c, doc_id)).await
}

#[tauri::command]
pub async fn signer_add(app: AppHandle, doc_id: i64, name: String, email: String) -> Result<Document, String> {
    run(app, move |c, _, _| core::signer_add(c, doc_id, &name, &email)).await
}

#[tauri::command]
pub async fn signer_remove(app: AppHandle, doc_id: i64, email: String) -> Result<Document, String> {
    run(app, move |c, _, _| core::signer_remove(c, doc_id, &email)).await
}

#[tauri::command]
pub async fn document_remind(app: AppHandle, doc_id: i64) -> Result<(), String> {
    let draft = run(app, move |c, _, t| core::reminder_draft(c, doc_id, t)).await?;
    commands_mail::send_mail(draft).await.map(|_| ())
}

#[tauri::command]
pub async fn signature_image_save(app: AppHandle, png_b64: String) -> Result<(), String> {
    run(app, move |_, d, _| core::image_save(d, &png_b64)).await
}

#[tauri::command]
pub async fn signature_image_get(app: AppHandle) -> Result<Option<String>, String> {
    run(app, move |_, d, _| core::image_get(d)).await
}

#[tauri::command]
pub async fn soffice_available() -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(signing::soffice_available).await.map_err(|e| e.to_string())
}
