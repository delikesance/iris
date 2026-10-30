# Contrat API Iris

Source de vérité du pont front/back. Rust : `src-tauri/src/types.rs` + `commands_{mail,auto,sign}.rs` ; TypeScript : `src/lib/types.ts` + `src/lib/api.ts` (un wrapper camelCase par commande).

## Conventions
- JSON en `snake_case` pour tous les champs de structures. Les **arguments de commande** de premier niveau sont en camelCase côté `invoke` (`doc_id` → `docId`, `dest_path` → `destPath`).
- Dates : secondes Unix UTC (`i64` / `number`), sauf `cron_describe.next_runs` (ISO 8601).
- Énumérations en `snake_case` : `FolderKind` system|sorted, `SendStatus` sent|scheduled, `ChronKind` morning_digest|reminders|newsletter_cleanup|monthly_invoices, `RecurringKind` newsletters|notifications|documents, `SenderDecision` block|recurring|inbox, `DocumentTab` todo|waiting|signed, `SignatureLevel` simple|advanced|qualified, `Security` ssl|starttls|none.
- `Option<T>` : clé absente ou `null` acceptée en entrée ; `null` en sortie.
- **Erreurs** : toute commande retourne `Result<T, String>` ; le rejet de `invoke` est une chaîne française. Les commandes pas encore implémentées rejettent `"non implémenté"`.
- Événement : le backend émet `mail://changed` (payload : nom du dossier) après sync ou exécution de règles ; l'UI recharge alors la liste.
- Aucune commande ne renvoie ni n'accepte de secret autre que `login.password`.

## Types
Voir `types.rs` / `types.ts` (miroirs). `Rule.cond` / `Rule.action` suivent `rules::Cond` / `rules::Action` (`{"type":"sender_is","value":"…"}`, `{"type":"all","of":[…]}`, `{"type":"move_to","folder":"…"}`…), portés comme JSON libre côté Rust.
`Draft` = `{to, cc?, subject, body, in_reply_to?, attachments: [chemin], remind_days?, send_at?}`. `Filters` = `{unread?, has_attach?, from?, since?, until?}`.

## Commandes mail (`commands_mail.rs`, implémentation b1 ; les 4 premières existent)
| Commande | Arguments (clés invoke) | Retour | Notes |
|---|---|---|---|
| `account` | — | `String` | Adresse du compte connecté. |
| `saved_settings` | — | `Option<Settings>` | Réglages enregistrés (sans mot de passe) ou null. |
| `login` | `settings`: Settings, `password`: String | `()` | Teste IMAP puis enregistre réglages + mot de passe (trousseau). |
| `logout` | — | `()` | Efface réglages et mot de passe. |
| `list_folders` | — | `Vec<Folder>` | Dossiers, ordre : Réception, Planifiés (virtuel), Brouillons, Envoyés, Archives, puis dossiers de tri. `label` en français. |
| `sync_folder` | `folder`: String | `()` | Rafraîchit le cache depuis IMAP puis émet `mail://changed`. |
| `list_messages` | `folder`: String, `query`: Option<String>, `filters`: Option<Filters> | `Vec<MessageRow>` | Lecture du cache, tri date décroissante. `query` filtre sujet/expéditeur. |
| `get_message` | `folder`: String, `uid`: u32 | `Full` | Message complet avec pièces jointes (`signable` = PDF/Word/ODT non remplacé). |
| `thread_get` | `threadId`: String | `Vec<ThreadMessage>` | Messages du fil, ordre chronologique. |
| `save_attachment` | `folder`: String, `uid`: u32, `index`: u32, `destPath`: String | `()` | Écrit la pièce jointe `index` dans `dest_path`. |
| `mark_seen` | `folder`: String, `uid`: u32, `seen`: bool | `()` | Pose/retire \Seen. |
| `bulk_move` | `folder`: String, `uids`: Vec<u32>, `target`: String | `()` | UID MOVE (repli COPY+EXPUNGE) ; crée `target` si absent. |
| `bulk_delete` | `folder`: String, `uids`: Vec<u32> | `()` | Supprime les UID. |
| `delete_message` | `folder`: String, `uid`: u32 | `()` | Supprime un message. |
| `send_mail` | `draft`: Draft | `SendResult` | Envoie (`status:sent`) ou planifie si `send_at` (`status:scheduled`, `id`). `remind_days` crée une relance. |
| `scheduled_list` | — | `Vec<ScheduledItem>` | Envois planifiés en attente. |
| `scheduled_cancel` | `id`: i64 | `()` | Annule un envoi planifié. |

## Commandes automatisation (`commands_auto.rs`, b2)
| Commande | Arguments (clés invoke) | Retour | Notes |
|---|---|---|---|
| `rules_list` | — | `Vec<Rule>` | Règles triées par `position`. |
| `rule_save` | `rule`: Rule | `Rule` | Crée (id null) ou met à jour ; retourne la règle avec son id. Erreur si webhook non https/IP privée. |
| `rule_delete` | `id`: i64 | `()` | Supprime. |
| `rule_toggle` | `id`: i64, `enabled`: bool | `()` | Active/désactive. |
| `rule_sentence` | `cond`: Value, `action`: Value | `String` | Phrase française (« Si …, alors … »). |
| `chrons_list` | — | `Vec<Chron>` | Chrons avec `description` et `next_run`. |
| `chron_save` | `chron`: Chron | `Chron` | Crée/met à jour ; erreur si expression cron invalide. |
| `chron_toggle` | `id`: i64, `enabled`: bool | `()` | Active/désactive. |
| `chron_delete` | `id`: i64 | `()` | Supprime. |
| `cron_describe` | `expr`: String | `CronDescription` | Valide une expression 5 champs ; `next_runs` = 3 dates ISO. |
| `recurring_list` | `kind`: Option<RecurringKind> | `Vec<RecurringSender>` | Expéditeurs récurrents, filtre optionnel par type. |
| `new_senders_list` | — | `Vec<NewSender>` | Expéditeurs en attente de décision. |
| `sender_decide` | `addr`: String, `decision`: SenderDecision | `()` | Enregistre la décision. |
| `unsubscribe` | `addr`: String | `UnsubscribeResult` | `done:true` si faite (mailto/one-click) ; sinon `open_url` à ouvrir après confirmation. |
| `contacts_list` | `query`: Option<String> | `Vec<Contact>` | Contacts, filtre `query` nom/e-mail. |
| `contact_get` | `email`: String | `ContactDetail` | Fiche + 20 derniers fils + règle associée. |
| `contact_save` | `contact`: Contact | `Contact` | Upsert par e-mail (insensible à la casse). |

## Commandes signature (`commands_sign.rs`, b3)
| Commande | Arguments (clés invoke) | Retour | Notes |
|---|---|---|---|
| `documents_list` | `tab`: DocumentTab | `Vec<Document>` | Documents de l'onglet. |
| `document_get` | `id`: i64 | `Document` | Un document. |
| `document_import` | `path`: String | `Document` | Copie un PDF local dans `<config>/iris/documents/`. |
| `sign_prepare_attachment` | `folder`: String, `uid`: u32, `index`: u32 | `Document` | Importe la pièce jointe (conversion Word→PDF via soffice si besoin). |
| `sign_apply` | `docId`: i64, `page`: u32, `rect`: [f32; 4], `signaturePngB64`: String | `Document` | Appose l'image (PNG base64) page `page` (0-based), `rect` = [x0,y0,x1,y1] en points PDF ; scelle et journalise. Erreur si `level` ≠ simple ou document non signable. |
| `document_export_for_reply` | `docId`: i64 | `String` | Chemin du PDF scellé à joindre. |
| `proof_get` | `docId`: i64 | `Proof` | Document + journal de preuve. |
| `signer_add` | `docId`: i64, `name`: String, `email`: String | `Document` | Ajoute un signataire. |
| `signer_remove` | `docId`: i64, `email`: String | `Document` | Retire un signataire. |
| `document_remind` | `docId`: i64 | `()` | Relance les signataires en attente. |
| `signature_image_save` | `pngB64`: String | `()` | Enregistre la signature manuscrite. |
| `signature_image_get` | — | `Option<String>` | PNG base64 ou null. |
| `soffice_available` | — | `bool` | LibreOffice (`soffice`) détecté. |

## Stores front (`src/lib/stores.ts`)
`route`, `currentFolder`, `selectedUid`, `selection` (Set d'uid), `composeDraft` (null ou `Draft`), `signingTarget` (null ou `{folder, uid, index}`), `toast`.

Total : 47 commandes (17 mail, 17 automatisation, 13 signature).
