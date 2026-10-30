import { beforeEach, describe, expect, it, vi } from "vitest";
import { readFileSync } from "node:fs";

const invoke = vi.fn(async (..._a: unknown[]) => null);
vi.mock("@tauri-apps/api/core", () => ({ invoke: (...a: unknown[]) => invoke(...a) }));

import * as api from "./api";
import type { Cond, Action, Draft, Rule, Chron, Contact, Settings } from "./types";

const settings = { email: "a@b.fr", imap_host: "i", imap_port: 993, smtp_host: "s", smtp_port: 465 } satisfies Settings;
const draft: Draft = { to: "x@y.fr", subject: "s", body: "b", attachments: [] };
const cond: Cond = { type: "has_list_unsubscribe" };
const action: Action = { type: "notify" };
const rule: Rule = { id: null, name: "r", enabled: true, position: 0, cond, action };
const chron: Chron = { id: null, name: "c", cron: "0 8 * * 1-5", kind: "morning_digest", enabled: true, last_run: null, next_run: null, description: "" };
const contact: Contact = { id: null, name: "n", email: "n@x.fr", notes: null };

// [wrapper, args, commande attendue, charge utile attendue]
const cases: [string, () => unknown, string, Record<string, unknown> | undefined][] = [
  ["account", () => api.account(), "account", undefined],
  ["savedSettings", () => api.savedSettings(), "saved_settings", undefined],
  ["login", () => api.login(settings, "pw"), "login", { settings, password: "pw" }],
  ["logout", () => api.logout(), "logout", undefined],
  ["listFolders", () => api.listFolders(), "list_folders", undefined],
  ["syncFolder", () => api.syncFolder("INBOX"), "sync_folder", { folder: "INBOX" }],
  ["listMessages", () => api.listMessages("INBOX", "q", { unread: true }), "list_messages", { folder: "INBOX", query: "q", filters: { unread: true } }],
  ["getMessage", () => api.getMessage("INBOX", 4), "get_message", { folder: "INBOX", uid: 4 }],
  ["threadGet", () => api.threadGet("t1"), "thread_get", { threadId: "t1" }],
  ["saveAttachment", () => api.saveAttachment("INBOX", 4, 0, "/d"), "save_attachment", { folder: "INBOX", uid: 4, index: 0, destPath: "/d" }],
  ["markSeen", () => api.markSeen("INBOX", 4, true), "mark_seen", { folder: "INBOX", uid: 4, seen: true }],
  ["bulkMove", () => api.bulkMove("INBOX", [1, 2], "Archives"), "bulk_move", { folder: "INBOX", uids: [1, 2], target: "Archives" }],
  ["bulkDelete", () => api.bulkDelete("INBOX", [1]), "bulk_delete", { folder: "INBOX", uids: [1] }],
  ["deleteMessage", () => api.deleteMessage("INBOX", 1), "delete_message", { folder: "INBOX", uid: 1 }],
  ["sendMail", () => api.sendMail(draft), "send_mail", { draft }],
  ["scheduledList", () => api.scheduledList(), "scheduled_list", undefined],
  ["scheduledCancel", () => api.scheduledCancel(3), "scheduled_cancel", { id: 3 }],
  ["rulesList", () => api.rulesList(), "rules_list", undefined],
  ["ruleSave", () => api.ruleSave(rule), "rule_save", { rule }],
  ["ruleDelete", () => api.ruleDelete(1), "rule_delete", { id: 1 }],
  ["ruleToggle", () => api.ruleToggle(1, false), "rule_toggle", { id: 1, enabled: false }],
  ["ruleSentence", () => api.ruleSentence(cond, action), "rule_sentence", { cond, action }],
  ["chronsList", () => api.chronsList(), "chrons_list", undefined],
  ["chronSave", () => api.chronSave(chron), "chron_save", { chron }],
  ["chronToggle", () => api.chronToggle(1, true), "chron_toggle", { id: 1, enabled: true }],
  ["chronDelete", () => api.chronDelete(1), "chron_delete", { id: 1 }],
  ["cronDescribe", () => api.cronDescribe("0 9 * * *"), "cron_describe", { expr: "0 9 * * *" }],
  ["recurringList", () => api.recurringList("newsletters"), "recurring_list", { kind: "newsletters" }],
  ["newSendersList", () => api.newSendersList(), "new_senders_list", undefined],
  ["senderDecide", () => api.senderDecide("a@b.fr", "block"), "sender_decide", { addr: "a@b.fr", decision: "block" }],
  ["unsubscribe", () => api.unsubscribe("a@b.fr"), "unsubscribe", { addr: "a@b.fr" }],
  ["contactsList", () => api.contactsList("cam"), "contacts_list", { query: "cam" }],
  ["contactGet", () => api.contactGet("n@x.fr"), "contact_get", { email: "n@x.fr" }],
  ["contactSave", () => api.contactSave(contact), "contact_save", { contact }],
  ["documentsList", () => api.documentsList("todo"), "documents_list", { tab: "todo" }],
  ["documentGet", () => api.documentGet(1), "document_get", { id: 1 }],
  ["documentImport", () => api.documentImport("/f.pdf"), "document_import", { path: "/f.pdf" }],
  ["signPrepareAttachment", () => api.signPrepareAttachment("INBOX", 4, 0), "sign_prepare_attachment", { folder: "INBOX", uid: 4, index: 0 }],
  ["signApply", () => api.signApply(1, 0, [1, 2, 3, 4], "AAA"), "sign_apply", { docId: 1, page: 0, rect: [1, 2, 3, 4], signaturePngB64: "AAA" }],
  ["documentExportForReply", () => api.documentExportForReply(1), "document_export_for_reply", { docId: 1 }],
  ["proofGet", () => api.proofGet(1), "proof_get", { docId: 1 }],
  ["signerAdd", () => api.signerAdd(1, "N", "n@x.fr"), "signer_add", { docId: 1, name: "N", email: "n@x.fr" }],
  ["signerRemove", () => api.signerRemove(1, "n@x.fr"), "signer_remove", { docId: 1, email: "n@x.fr" }],
  ["documentRemind", () => api.documentRemind(1), "document_remind", { docId: 1 }],
  ["signatureImageSave", () => api.signatureImageSave("AAA"), "signature_image_save", { pngB64: "AAA" }],
  ["signatureImageGet", () => api.signatureImageGet(), "signature_image_get", undefined],
  ["sofficeAvailable", () => api.sofficeAvailable(), "soffice_available", undefined],
];

beforeEach(() => invoke.mockClear());

describe("wrappers api", () => {
  it.each(cases)("%s appelle la bonne commande", async (_n, call, command, payload) => {
    await call();
    expect(invoke).toHaveBeenCalledTimes(1);
    expect(invoke.mock.calls[0]).toEqual(payload === undefined ? [command] : [command, payload]);
  });

  it("couvre exactement les commandes déclarées dans lib.rs", () => {
    const lib = readFileSync("src-tauri/src/lib.rs", "utf8");
    const block = lib.slice(lib.indexOf("generate_handler!["), lib.indexOf("])", lib.indexOf("generate_handler![")));
    const registered = [...block.matchAll(/commands_\w+::(\w+)/g)].map((m) => m[1]).sort();
    const tested = cases.map((c) => c[2]).sort();
    expect(registered.length).toBe(47);
    expect(tested).toEqual(registered);
    expect(Object.keys(api).length).toBe(registered.length);
  });
});
