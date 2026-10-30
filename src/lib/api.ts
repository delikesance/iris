import { invoke } from "@tauri-apps/api/core";
import type * as T from "./types";
export type * from "./types";

// Un wrapper typé par commande Tauri (voir docs/api-contract.md). Les arguments passent en camelCase côté invoke.

// mail
export const account = () => invoke<string>("account");
export const savedSettings = () => invoke<T.Settings | null>("saved_settings");
export const login = (settings: T.Settings, password: string) => invoke<void>("login", { settings, password });
export const logout = () => invoke<void>("logout");
export const listFolders = () => invoke<T.Folder[]>("list_folders");
export const syncFolder = (folder: string) => invoke<void>("sync_folder", { folder });
export const listMessages = (folder: string, query?: string, filters?: T.Filters) => invoke<T.MessageRow[]>("list_messages", { folder, query, filters });
export const getMessage = (folder: string, uid: number) => invoke<T.Full>("get_message", { folder, uid });
export const threadGet = (threadId: string) => invoke<T.ThreadMessage[]>("thread_get", { threadId });
export const saveAttachment = (folder: string, uid: number, index: number, destPath: string) => invoke<void>("save_attachment", { folder, uid, index, destPath });
export const markSeen = (folder: string, uid: number, seen: boolean) => invoke<void>("mark_seen", { folder, uid, seen });
export const bulkMove = (folder: string, uids: number[], target: string) => invoke<void>("bulk_move", { folder, uids, target });
export const bulkDelete = (folder: string, uids: number[]) => invoke<void>("bulk_delete", { folder, uids });
export const deleteMessage = (folder: string, uid: number) => invoke<void>("delete_message", { folder, uid });
export const sendMail = (draft: T.Draft) => invoke<T.SendResult>("send_mail", { draft });
export const scheduledList = () => invoke<T.ScheduledItem[]>("scheduled_list");
export const scheduledCancel = (id: number) => invoke<void>("scheduled_cancel", { id });

// auto
export const rulesList = () => invoke<T.Rule[]>("rules_list");
export const ruleSave = (rule: T.Rule) => invoke<T.Rule>("rule_save", { rule });
export const ruleDelete = (id: number) => invoke<void>("rule_delete", { id });
export const ruleToggle = (id: number, enabled: boolean) => invoke<void>("rule_toggle", { id, enabled });
export const ruleSentence = (cond: T.Cond, action: T.Action) => invoke<string>("rule_sentence", { cond, action });
export const chronsList = () => invoke<T.Chron[]>("chrons_list");
export const chronSave = (chron: T.Chron) => invoke<T.Chron>("chron_save", { chron });
export const chronToggle = (id: number, enabled: boolean) => invoke<void>("chron_toggle", { id, enabled });
export const chronDelete = (id: number) => invoke<void>("chron_delete", { id });
export const cronDescribe = (expr: string) => invoke<T.CronDescription>("cron_describe", { expr });
export const recurringList = (kind?: T.RecurringKind) => invoke<T.RecurringSender[]>("recurring_list", { kind });
export const newSendersList = () => invoke<T.NewSender[]>("new_senders_list");
export const senderDecide = (addr: string, decision: T.SenderDecision) => invoke<void>("sender_decide", { addr, decision });
export const unsubscribe = (addr: string) => invoke<T.UnsubscribeResult>("unsubscribe", { addr });
export const contactsList = (query?: string) => invoke<T.Contact[]>("contacts_list", { query });
export const contactGet = (email: string) => invoke<T.ContactDetail>("contact_get", { email });
export const contactSave = (contact: T.Contact) => invoke<T.Contact>("contact_save", { contact });

// sign
export const documentsList = (tab: T.DocumentTab) => invoke<T.Document[]>("documents_list", { tab });
export const documentGet = (id: number) => invoke<T.Document>("document_get", { id });
export const documentImport = (path: string) => invoke<T.Document>("document_import", { path });
export const signPrepareAttachment = (folder: string, uid: number, index: number) => invoke<T.Document>("sign_prepare_attachment", { folder, uid, index });
export const signApply = (docId: number, page: number, rect: [number, number, number, number], signaturePngB64: string) => invoke<T.Document>("sign_apply", { docId, page, rect, signaturePngB64 });
export const documentExportForReply = (docId: number) => invoke<string>("document_export_for_reply", { docId });
export const proofGet = (docId: number) => invoke<T.Proof>("proof_get", { docId });
export const signerAdd = (docId: number, name: string, email: string) => invoke<T.Document>("signer_add", { docId, name, email });
export const signerRemove = (docId: number, email: string) => invoke<T.Document>("signer_remove", { docId, email });
export const documentRemind = (docId: number) => invoke<void>("document_remind", { docId });
export const signatureImageSave = (pngB64: string) => invoke<void>("signature_image_save", { pngB64 });
export const signatureImageGet = () => invoke<string | null>("signature_image_get");
export const sofficeAvailable = () => invoke<boolean>("soffice_available");
