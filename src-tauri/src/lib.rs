mod automation;
mod commands_auto;
mod commands_mail;
mod commands_sign;
mod config;
mod db;
mod mail;
mod recurring;
mod rules;
mod scheduler;
mod signing;
mod signing_cmds;
mod threading;
mod types;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    config::migrate_legacy();
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            automation::start(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands_mail::account,
            commands_mail::saved_settings,
            commands_mail::login,
            commands_mail::logout,
            commands_mail::list_folders,
            commands_mail::sync_folder,
            commands_mail::list_messages,
            commands_mail::get_message,
            commands_mail::thread_get,
            commands_mail::save_attachment,
            commands_mail::mark_seen,
            commands_mail::bulk_move,
            commands_mail::bulk_delete,
            commands_mail::delete_message,
            commands_mail::send_mail,
            commands_mail::scheduled_list,
            commands_mail::scheduled_cancel,
            commands_auto::rules_list,
            commands_auto::rule_save,
            commands_auto::rule_delete,
            commands_auto::rule_toggle,
            commands_auto::rule_sentence,
            commands_auto::chrons_list,
            commands_auto::chron_save,
            commands_auto::chron_toggle,
            commands_auto::chron_delete,
            commands_auto::cron_describe,
            commands_auto::recurring_list,
            commands_auto::new_senders_list,
            commands_auto::sender_decide,
            commands_auto::unsubscribe,
            commands_auto::contacts_list,
            commands_auto::contact_get,
            commands_auto::contact_save,
            commands_sign::documents_list,
            commands_sign::document_get,
            commands_sign::document_import,
            commands_sign::sign_prepare_attachment,
            commands_sign::sign_apply,
            commands_sign::document_export_for_reply,
            commands_sign::proof_get,
            commands_sign::signer_add,
            commands_sign::signer_remove,
            commands_sign::document_remind,
            commands_sign::signature_image_save,
            commands_sign::signature_image_get,
            commands_sign::soffice_available
        ])
        .run(tauri::generate_context!())
        .expect("erreur au lancement de Iris");
}
