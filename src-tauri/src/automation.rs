//! Automatisation : règles de tri après synchronisation, boucle du planificateur (chrons, envois planifiés,
//! relances), expéditeurs récurrents / nouveaux, désinscription et contacts.
//!
//! Les fonctions qui touchent la base prennent une `&Connection` (testées sur une base en mémoire) ;
//! les E/S réseau (IMAP, SMTP, webhooks, notifications) restent dans de fines couches d'orchestration
//! qui n'occupent jamais le verrou de la base pendant l'appel réseau.
use crate::recurring::{self, SenderHistory};
use crate::types::{self, ChronKind, RecurringKind, SenderDecision};
use crate::{config, mail, rules, scheduler};
use chrono::{DateTime, Local, TimeZone};
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;
use std::time::Duration;
use tauri::Emitter;
use tauri_plugin_notification::NotificationExt;

const INBOX: &str = "INBOX";
const REMIND_FOLDER: &str = "À relancer";
const INVOICE_FOLDER: &str = "Factures";
const TICK: Duration = Duration::from_secs(30);
const DAY: i64 = 86_400;
/// Au-delà, un message ne déclenche plus de notification ni de webhook (première synchronisation).
const FRESH_SECS: i64 = DAY;
const WEBHOOK_TIMEOUT: Duration = Duration::from_secs(5);

fn e<T: std::fmt::Display>(x: T) -> String { x.to_string() }

static APP: OnceLock<tauri::AppHandle> = OnceLock::new();

/// Compte courant en minuscules (clé du cache `message`).
pub fn account() -> Result<String, String> { Ok(config::load()?.user.to_lowercase()) }

fn emit_changed(folder: &str) {
    if let Some(app) = APP.get() { let _ = app.emit("mail://changed", folder.to_string()); }
}

fn notify(title: &str, body: &str) {
    let Some(app) = APP.get() else { return };
    if let Err(x) = app.notification().builder().title(title).body(body).show() { eprintln!("iris: notification : {x}"); }
}

// ---------- Règles : CRUD ----------

fn parse_rule(cond: &serde_json::Value, action: &serde_json::Value) -> Result<(rules::Cond, rules::Action), String> {
    let c: rules::Cond = serde_json::from_value(cond.clone()).map_err(|x| format!("condition invalide : {x}"))?;
    let a: rules::Action = serde_json::from_value(action.clone()).map_err(|x| format!("action invalide : {x}"))?;
    match &a {
        rules::Action::Webhook { url } => rules::validate_webhook(url)?,
        rules::Action::MoveTo { folder } if folder.trim().is_empty() => return Err("dossier de destination manquant".into()),
        _ => {}
    }
    Ok((c, a))
}

pub fn rule_sentence(cond: &serde_json::Value, action: &serde_json::Value) -> Result<String, String> {
    let (c, a) = parse_rule(cond, action)?;
    Ok(rules::sentence(&c, &a))
}

type RuleRow = (i64, String, bool, i64, String, String);

fn rule_rows(conn: &Connection, only_enabled: bool) -> Result<Vec<RuleRow>, String> {
    let sql = format!(
        "SELECT id, COALESCE(name,''), COALESCE(enabled,1), COALESCE(position,0), cond_json, action_json FROM rule {} ORDER BY position, id",
        if only_enabled { "WHERE COALESCE(enabled,1)=1" } else { "" });
    let mut st = conn.prepare(&sql).map_err(e)?;
    let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?))).map_err(e)?;
    rows.collect::<Result<_, _>>().map_err(e)
}

fn to_rule((id, name, enabled, position, cond, action): RuleRow) -> Result<types::Rule, String> {
    Ok(types::Rule {
        id: Some(id), name, enabled, position,
        cond: serde_json::from_str(&cond).map_err(e)?, action: serde_json::from_str(&action).map_err(e)?,
    })
}

pub fn rules_list(conn: &Connection) -> Result<Vec<types::Rule>, String> {
    rule_rows(conn, false)?.into_iter().map(to_rule).collect()
}

fn rule_get(conn: &Connection, id: i64) -> Result<types::Rule, String> {
    rule_rows(conn, false)?.into_iter().find(|r| r.0 == id).map(to_rule).ok_or("règle introuvable")?
}

/// Règles actives, dans l'ordre de position. Une règle illisible est ignorée (journalisée).
pub fn active_rules(conn: &Connection) -> Result<Vec<(i64, rules::Rule)>, String> {
    Ok(rule_rows(conn, true)?.into_iter().filter_map(|r| {
        let id = r.0;
        match to_rule(r).and_then(|t| parse_rule(&t.cond, &t.action)) {
            Ok((cond, action)) => Some((id, rules::Rule { cond, action })),
            Err(x) => { eprintln!("iris: règle {id} ignorée : {x}"); None }
        }
    }).collect())
}

pub fn rule_save(conn: &Connection, rule: types::Rule) -> Result<types::Rule, String> {
    let (cond, action) = parse_rule(&rule.cond, &rule.action)?;
    let name = if rule.name.trim().is_empty() { rules::sentence(&cond, &action) } else { rule.name.trim().to_string() };
    let (cj, aj) = (serde_json::to_string(&cond).map_err(e)?, serde_json::to_string(&action).map_err(e)?);
    let id = match rule.id {
        Some(id) => {
            let n = conn.execute("UPDATE rule SET name=?2, enabled=?3, position=?4, cond_json=?5, action_json=?6 WHERE id=?1",
                params![id, name, rule.enabled, rule.position, cj, aj]).map_err(e)?;
            if n == 0 { return Err("règle introuvable".into()); }
            id
        }
        None => {
            let position = if rule.position > 0 { rule.position }
                else { conn.query_row("SELECT COALESCE(MAX(position),0)+1 FROM rule", [], |r| r.get(0)).map_err(e)? };
            conn.execute("INSERT INTO rule(name, enabled, position, cond_json, action_json) VALUES(?1,?2,?3,?4,?5)",
                params![name, rule.enabled, position, cj, aj]).map_err(e)?;
            conn.last_insert_rowid()
        }
    };
    rule_get(conn, id)
}

pub fn rule_delete(conn: &Connection, id: i64) -> Result<(), String> {
    conn.execute("UPDATE message SET rule_id=NULL WHERE rule_id=?1", params![id]).map_err(e)?;
    conn.execute("DELETE FROM rule WHERE id=?1", params![id]).map(|_| ()).map_err(e)
}

pub fn rule_toggle(conn: &Connection, id: i64, enabled: bool) -> Result<(), String> {
    match conn.execute("UPDATE rule SET enabled=?2 WHERE id=?1", params![id, enabled]).map_err(e)? {
        0 => Err("règle introuvable".into()),
        _ => Ok(()),
    }
}

// ---------- Règles : exécution ----------

/// Un message reçu tel que le cache le connaît.
#[derive(Clone, Debug)]
pub struct Incoming {
    pub uid: u32, pub folder: String, pub message_id: String, pub thread_id: Option<String>,
    pub from_name: String, pub from_addr: String, pub subject: String, pub date: i64,
    pub has_attach: bool, pub list_unsubscribe: bool,
}

pub fn incoming(conn: &Connection, acc: &str, folder: &str, uids: &[u32]) -> Result<Vec<Incoming>, String> {
    let mut st = conn.prepare(
        "SELECT message_id, thread_id, from_name, from_addr, subject, date, has_attach, COALESCE(list_unsubscribe,'')<>''
         FROM message WHERE account=?1 AND folder=?2 AND uid=?3").map_err(e)?;
    let mut out = Vec::new();
    for uid in uids {
        let row = st.query_row(params![acc, folder, uid], |r| {
            let s = |i: usize| r.get::<_, Option<String>>(i).map(Option::unwrap_or_default);
            Ok(Incoming {
                uid: *uid, folder: folder.to_string(), message_id: s(0)?, thread_id: r.get(1)?, from_name: s(2)?, from_addr: s(3)?,
                subject: s(4)?, date: r.get::<_, Option<i64>>(5)?.unwrap_or(0),
                has_attach: r.get::<_, Option<bool>>(6)?.unwrap_or(false), list_unsubscribe: r.get(7)?,
            })
        }).optional().map_err(e)?;
        out.extend(row);
    }
    Ok(out)
}

fn cond_uses_ext(c: &rules::Cond) -> bool {
    match c {
        rules::Cond::HasAttachmentExt { .. } => true,
        rules::Cond::All { of } => of.iter().any(cond_uses_ext),
        _ => false,
    }
}

fn ext_of(filename: &str) -> Option<String> {
    filename.rsplit_once('.').map(|(_, x)| x.trim().to_lowercase()).filter(|x| !x.is_empty())
}

/// Extensions des pièces jointes (sans marquer le message lu : le fil est lu en `BODY.PEEK`).
fn attachment_exts(m: &Incoming) -> Vec<String> {
    let Some(tid) = m.thread_id.as_deref().filter(|_| m.has_attach) else { return vec![] };
    let msgs = mail::thread(tid).unwrap_or_else(|x| { eprintln!("iris: pièces jointes illisibles : {x}"); vec![] });
    msgs.iter().filter(|t| t.uid == m.uid && t.folder == m.folder)
        .flat_map(|t| t.attachments.iter().filter_map(|a| ext_of(&a.filename))).collect()
}

pub fn facts(m: &Incoming, exts: Vec<String>) -> rules::MsgFacts {
    rules::MsgFacts {
        from_addr: m.from_addr.clone(), subject: m.subject.clone(), has_attach_exts: exts,
        has_list_unsubscribe: m.list_unsubscribe, date: m.date, replied: false,
    }
}

#[derive(Debug, PartialEq)]
pub enum Filing { Folder(String), Archive }

/// Ce qu'il faut faire d'un message : classement (avec la règle responsable), notification, webhooks, relance.
#[derive(Debug, Default, PartialEq)]
pub struct Plan { pub filing: Option<(Filing, i64)>, pub notify: bool, pub webhooks: Vec<String>, pub remind_days: Option<u32> }

pub fn plan(active: &[(i64, rules::Rule)], f: &rules::MsgFacts, now: i64) -> Plan {
    let list: Vec<rules::Rule> = active.iter().map(|(_, r)| r.clone()).collect();
    let mut p = Plan::default();
    for action in rules::evaluate(&list, f, now) {
        match action {
            rules::Action::MoveTo { folder } => p.filing = Some((Filing::Folder(folder), 0)),
            rules::Action::Archive => p.filing = Some((Filing::Archive, 0)),
            rules::Action::Notify => p.notify = true,
            rules::Action::Webhook { url } => p.webhooks.push(url),
            rules::Action::Remind { days } => { p.remind_days.get_or_insert(days); }
        }
    }
    let filing_rule = active.iter()
        .find(|(_, r)| matches!(r.action, rules::Action::MoveTo { .. } | rules::Action::Archive) && rules::matches(&r.cond, f, now));
    if let (Some((rid, _)), Some((_, slot))) = (filing_rule, p.filing.as_mut()) { *slot = *rid; }
    p
}

/// Une règle de classement vise-t-elle ce dossier ?
fn targets_folder(a: &rules::Action, folder: &str) -> bool {
    match a {
        rules::Action::MoveTo { folder: f } => f == folder,
        rules::Action::Archive => mail::role_of(folder, &[]) == Some(mail::Role::Archive),
        _ => false,
    }
}

/// Règle qui explique la présence d'un message dans `folder` (classement par une règle active).
pub fn sorting_rule(active: &[(i64, rules::Rule)], folder: &str, f: &rules::MsgFacts, now: i64) -> Option<i64> {
    active.iter().find(|(_, r)| targets_folder(&r.action, folder) && rules::matches(&r.cond, f, now)).map(|(id, _)| *id)
}

pub fn notification_text(items: &[&Incoming]) -> (String, String) {
    match items {
        [one] => (if one.from_name.is_empty() { one.from_addr.clone() } else { one.from_name.clone() }, one.subject.clone()),
        many => ("Iris".to_string(), format!("{} nouveaux messages", many.len())),
    }
}

/// POST JSON, https uniquement (voir `rules::validate_webhook`), 5 s, sans redirection.
pub fn post_webhook(url: &str, payload: &serde_json::Value) -> Result<(), String> {
    rules::validate_webhook(url)?;
    ureq::AgentBuilder::new().timeout(WEBHOOK_TIMEOUT).redirects(0).build()
        .post(url).send_json(payload.clone()).map(|_| ()).map_err(e)
}

fn webhook_payload(m: &Incoming) -> serde_json::Value {
    serde_json::json!({ "event": "new_message", "folder": m.folder, "message_id": m.message_id,
        "from_name": m.from_name, "from_addr": m.from_addr, "subject": m.subject, "date": m.date })
}

fn special_folder(label: &str, default: &str) -> String {
    mail::folders().ok().and_then(|v| v.into_iter().find(|f| f.kind == types::FolderKind::System && f.label == label))
        .map(|f| f.name).unwrap_or_else(|| default.to_string())
}

fn spawn_webhooks(jobs: Vec<(String, serde_json::Value)>) {
    if jobs.is_empty() { return; }
    std::thread::spawn(move || {
        for (url, body) in jobs { if let Err(x) = post_webhook(&url, &body) { eprintln!("iris: webhook {url} : {x}"); } }
    });
}

/// Applique les plans d'un lot de nouveaux messages de la Réception.
fn apply_plans(folder: &str, items: Vec<(Incoming, Plan)>) {
    let t = mail::now();
    let fresh = |m: &Incoming| t - m.date <= FRESH_SECS;
    let reminders: Vec<(&str, u32)> = items.iter().filter(|(m, _)| !m.message_id.is_empty())
        .filter_map(|(m, p)| p.remind_days.map(|d| (m.message_id.as_str(), d))).collect();
    if !reminders.is_empty() {
        let r = mail::with_db(|c| reminders.iter().try_for_each(|(id, d)| mail::add_reminder(c, id, t + i64::from(*d) * DAY).map(|_| ())));
        if let Err(x) = r { eprintln!("iris: relance : {x}"); }
    }
    let notif: Vec<&Incoming> = items.iter().filter(|(m, p)| p.notify && fresh(m)).map(|(m, _)| m).collect();
    if !notif.is_empty() { let (title, body) = notification_text(&notif); notify(&title, &body); }
    spawn_webhooks(items.iter().filter(|(m, _)| fresh(m))
        .flat_map(|(m, p)| p.webhooks.iter().map(|u| (u.clone(), webhook_payload(m)))).collect());
    file_messages(folder, &items);
}

fn file_messages(folder: &str, items: &[(Incoming, Plan)]) {
    let mut by_target: HashMap<String, Vec<u32>> = HashMap::new();
    let mut archive: Option<String> = None;
    for (m, p) in items {
        let target = match &p.filing {
            Some((Filing::Folder(f), _)) => f.clone(),
            Some((Filing::Archive, _)) => archive.get_or_insert_with(|| special_folder("Archives", "Archive")).clone(),
            None => continue,
        };
        if target != folder { by_target.entry(target).or_default().push(m.uid); }
    }
    for (target, uids) in by_target {
        match mail::bulk_move(folder, &uids, &target) {
            // La synchronisation du dossier cible rattache les messages à leur règle (voir `tag_sorted`).
            Ok(()) => {
                if let Err(x) = mail::sync_folder(&target) { eprintln!("iris: synchronisation de {target} : {x}"); }
                emit_changed(folder);
                emit_changed(&target);
            }
            Err(x) => eprintln!("iris: classement dans {target} : {x}"),
        }
    }
}

/// Faits d'un message ; les pièces jointes ne sont lues sur le serveur que si une règle en a besoin.
fn facts_for(active: &[(i64, rules::Rule)], m: &Incoming) -> rules::MsgFacts {
    let exts = if active.iter().any(|(_, r)| cond_uses_ext(&r.cond)) { attachment_exts(m) } else { vec![] };
    facts(m, exts)
}

fn run_rules(folder: &str, active: &[(i64, rules::Rule)], items: Vec<Incoming>) {
    if active.is_empty() { return; }
    let now = mail::now();
    let planned: Vec<(Incoming, Plan)> = items.into_iter().map(|m| { let p = plan(active, &facts_for(active, &m), now); (m, p) }).collect();
    apply_plans(folder, planned);
}

/// Dossier de tri : rattache à leur règle les messages qui viennent d'y arriver.
fn tag_sorted(acc: &str, folder: &str, active: &[(i64, rules::Rule)], items: &[Incoming]) -> Result<(), String> {
    let now = mail::now();
    let tags: Vec<(u32, i64)> = items.iter()
        .filter_map(|m| sorting_rule(active, folder, &facts_for(active, m), now).map(|id| (m.uid, id))).collect();
    if tags.is_empty() { return Ok(()); }
    mail::with_db(|c| tags.iter().try_for_each(|(uid, id)| c.execute(
        "UPDATE message SET rule_id=?4 WHERE account=?1 AND folder=?2 AND uid=?3", params![acc, folder, uid, id]).map(|_| ()).map_err(e)))?;
    emit_changed(folder);
    Ok(())
}

fn handle_sync(folder: &str, new: &[u32]) -> Result<(), String> {
    let acc = account()?;
    let is_inbox = folder.eq_ignore_ascii_case(INBOX);
    let (active, items) = mail::with_db(|c| {
        let items = incoming(c, &acc, folder, new)?;
        if is_inbox { upsert_contacts(c, &acc, &items)?; }
        Ok((active_rules(c)?, items))
    })?;
    if is_inbox { run_rules(folder, &active, items); Ok(()) } else { tag_sorted(&acc, folder, &active, &items) }
}

/// Rappel branché sur `mail::set_after_sync` : contacts puis règles. Les échecs sont journalisés, jamais fatals.
fn after_sync(folder: &str, new: &[u32]) {
    if new.is_empty() { return; }
    if let Err(x) = handle_sync(folder, new) { eprintln!("iris: automatisation après synchronisation : {x}"); }
}

// ---------- Chrons ----------

fn kind_str(k: ChronKind) -> String { serde_json::to_value(k).ok().and_then(|v| v.as_str().map(str::to_string)).unwrap_or_default() }

fn kind_from(s: &str) -> Result<ChronKind, String> {
    serde_json::from_value(serde_json::Value::String(s.to_string())).map_err(|_| format!("type de chron inconnu : {s}"))
}

type ChronRow = (i64, String, String, ChronKind, bool, Option<i64>);

fn chron_rows(conn: &Connection) -> Result<Vec<ChronRow>, String> {
    let mut st = conn.prepare("SELECT id, COALESCE(name,''), cron, kind, COALESCE(enabled,1), last_run FROM chron ORDER BY id").map_err(e)?;
    let raw: Vec<(i64, String, String, String, bool, Option<i64>)> = st
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?))).map_err(e)?
        .collect::<Result<_, _>>().map_err(e)?;
    raw.into_iter().map(|(id, name, cron, kind, en, last)| Ok((id, name, cron, kind_from(&kind)?, en, last))).collect()
}

fn to_chron<Tz: TimeZone>((id, name, cron, kind, enabled, last_run): ChronRow, now: &DateTime<Tz>) -> types::Chron {
    let next_run = if enabled { scheduler::next_run(&cron, now).ok().map(|d| d.timestamp()) } else { None };
    types::Chron { id: Some(id), name, description: scheduler::describe(&cron), cron, kind, enabled, last_run, next_run }
}

pub fn chrons_list<Tz: TimeZone>(conn: &Connection, now: &DateTime<Tz>) -> Result<Vec<types::Chron>, String> {
    Ok(chron_rows(conn)?.into_iter().map(|r| to_chron(r, now)).collect())
}

pub fn chron_save<Tz: TimeZone>(conn: &Connection, chron: types::Chron, now: &DateTime<Tz>) -> Result<types::Chron, String> {
    let cron = chron.cron.trim().to_string();
    scheduler::validate(&cron)?;
    let name = if chron.name.trim().is_empty() { scheduler::describe(&cron) } else { chron.name.trim().to_string() };
    let kind = kind_str(chron.kind);
    let id = match chron.id {
        // Changer l'expression repart de maintenant : pas de rattrapage d'une ancienne échéance.
        Some(id) => {
            let n = conn.execute("UPDATE chron SET name=?2, kind=?4, enabled=?5, last_run=CASE WHEN cron<>?3 THEN ?6 ELSE last_run END, cron=?3 WHERE id=?1",
                params![id, name, cron, kind, chron.enabled, now.timestamp()]).map_err(e)?;
            if n == 0 { return Err("chron introuvable".into()); }
            id
        }
        None => {
            conn.execute("INSERT INTO chron(name, cron, kind, enabled, last_run) VALUES(?1,?2,?3,?4,?5)",
                params![name, cron, kind, chron.enabled, now.timestamp()]).map_err(e)?;
            conn.last_insert_rowid()
        }
    };
    chron_rows(conn)?.into_iter().find(|r| r.0 == id).map(|r| to_chron(r, now)).ok_or_else(|| "chron introuvable".into())
}

pub fn chron_toggle(conn: &Connection, id: i64, enabled: bool, now_ts: i64) -> Result<(), String> {
    // Réactiver repart de maintenant : pas de rattrapage d'une échéance passée en pause.
    let n = conn.execute("UPDATE chron SET enabled=?2, last_run=CASE WHEN ?2 THEN ?3 ELSE last_run END WHERE id=?1", params![id, enabled, now_ts]).map_err(e)?;
    if n == 0 { Err("chron introuvable".into()) } else { Ok(()) }
}

pub fn chron_delete(conn: &Connection, id: i64) -> Result<(), String> {
    conn.execute("DELETE FROM chron WHERE id=?1", params![id]).map(|_| ()).map_err(e)
}

pub fn cron_describe(expr: &str) -> types::CronDescription {
    match scheduler::validate(expr) {
        Err(error) => types::CronDescription { ok: false, description: None, next_runs: vec![], error: Some(error) },
        Ok(()) => {
            let mut runs = Vec::new();
            let mut after = Local::now();
            while runs.len() < 3 {
                let Ok(n) = scheduler::next_run(expr, &after) else { break };
                runs.push(n.to_rfc3339());
                after = n;
            }
            types::CronDescription { ok: true, description: Some(scheduler::describe(expr)), next_runs: runs, error: None }
        }
    }
}

/// Les quatre chrons par défaut, seulement si la table est vide (le rapport mensuel démarre en pause).
pub fn seed_defaults(conn: &Connection, now_ts: i64) -> Result<(), String> {
    let n: i64 = conn.query_row("SELECT COUNT(*) FROM chron", [], |r| r.get(0)).map_err(e)?;
    if n > 0 { return Ok(()); }
    let defaults = [
        ("Résumé du matin", "0 8 * * 1-5", ChronKind::MorningDigest, true),
        ("Relances", "0 9 * * *", ChronKind::Reminders, true),
        ("Nettoyage des newsletters", "0 20 * * 0", ChronKind::NewsletterCleanup, true),
        ("Factures du mois", "0 7 1 * *", ChronKind::MonthlyInvoices, false),
    ];
    for (name, cron, kind, enabled) in defaults {
        conn.execute("INSERT INTO chron(name, cron, kind, enabled, last_run) VALUES(?1,?2,?3,?4,?5)", params![name, cron, kind_str(kind), enabled, now_ts]).map_err(e)?;
    }
    Ok(())
}

/// Chrons échus : marque leur exécution à `now` et renvoie ceux à jouer (rattrapage limité à 24 h).
pub fn claim_due_chrons<Tz: TimeZone>(conn: &Connection, now: &DateTime<Tz>) -> Result<Vec<ChronKind>, String> {
    let rows = chron_rows(conn)?;
    let sched: Vec<scheduler::Chron> = rows.iter().map(|r| scheduler::Chron { id: r.0, expr: r.2.clone(), enabled: r.4, last_run: r.5 }).collect();
    let mut kinds = Vec::new();
    for id in scheduler::due(&sched, now) {
        conn.execute("UPDATE chron SET last_run=?2 WHERE id=?1", params![id, now.timestamp()]).map_err(e)?;
        if let Some(r) = rows.iter().find(|r| r.0 == id).filter(|r| scheduler::catch_up(&r.2, r.5, now)) { kinds.push(r.3); }
    }
    Ok(kinds)
}

// ---------- Chrons : tâches ----------

pub fn unread_count(conn: &Connection, acc: &str, folder: &str) -> Result<u32, String> {
    conn.query_row("SELECT COUNT(*) FROM message WHERE account=?1 AND folder=?2 AND COALESCE(seen,0)=0", params![acc, folder], |r| r.get(0)).map_err(e)
}

pub fn digest_text(unread: u32) -> Option<String> {
    match unread {
        0 => None,
        1 => Some("1 message non lu dans votre boîte de réception".into()),
        n => Some(format!("{n} messages non lus dans votre boîte de réception")),
    }
}

/// UID de la Réception : lus, issus d'une liste de diffusion, vieux de plus de 7 jours.
pub fn cleanup_candidates(conn: &Connection, acc: &str, now: i64) -> Result<Vec<u32>, String> {
    inbox_uids_where(conn, acc, "seen=1 AND COALESCE(list_unsubscribe,'')<>'' AND date<=?3", now - 7 * DAY)
}

/// `cond` filtre les messages de la Réception ; `?3` y désigne `bound`.
fn inbox_uids_where(conn: &Connection, acc: &str, cond: &str, bound: i64) -> Result<Vec<u32>, String> {
    let mut st = conn.prepare(&format!("SELECT uid FROM message WHERE account=?1 AND folder=?2 AND {cond} ORDER BY uid")).map_err(e)?;
    let rows = st.query_map(params![acc, INBOX, bound], |r| r.get(0)).map_err(e)?;
    rows.collect::<Result<_, _>>().map_err(e)
}

/// UID de la Réception reçus depuis 31 jours, avec pièce jointe, d'expéditeurs de type « documents ».
pub fn invoice_candidates(conn: &Connection, acc: &str, now: i64) -> Result<Vec<u32>, String> {
    let docs: HashSet<String> = senders(conn, acc)?.into_iter()
        .filter(|s| recurring::classify(&s.hist) == Some(recurring::Kind::Documents)).map(|s| s.hist.addr).collect();
    let recent = inbox_uids_where(conn, acc, "has_attach=1 AND date>=?3", now - 31 * DAY)?;
    let mut st = conn.prepare("SELECT LOWER(from_addr) FROM message WHERE account=?1 AND folder=?2 AND uid=?3").map_err(e)?;
    let mut out = Vec::new();
    for uid in recent {
        let from: Option<String> = st.query_row(params![acc, INBOX, uid], |r| r.get(0)).optional().map_err(e)?;
        if from.is_some_and(|f| docs.contains(&f)) { out.push(uid); }
    }
    Ok(out)
}

fn run_chron(kind: ChronKind) -> Result<(), String> {
    let (acc, now) = (account()?, mail::now());
    match kind {
        ChronKind::MorningDigest => {
            if let Some(text) = digest_text(mail::with_db(|c| unread_count(c, &acc, INBOX))?) { notify("Iris — résumé du matin", &text); }
            Ok(())
        }
        ChronKind::Reminders => process_reminders(now),
        ChronKind::NewsletterCleanup => {
            let uids = mail::with_db(|c| cleanup_candidates(c, &acc, now))?;
            move_inbox(&uids, &special_folder("Archives", "Archive"))
        }
        ChronKind::MonthlyInvoices => move_inbox(&mail::with_db(|c| invoice_candidates(c, &acc, now))?, INVOICE_FOLDER),
    }
}

fn move_inbox(uids: &[u32], target: &str) -> Result<(), String> {
    if uids.is_empty() { return Ok(()); }
    mail::bulk_move(INBOX, uids, target)?;
    emit_changed(INBOX);
    Ok(())
}

// ---------- Envois planifiés ----------

/// Envois échus : passés à `sending` avant l'envoi, pour ne jamais partir deux fois après un plantage.
pub fn claim_due_sends(conn: &Connection, now: i64) -> Result<Vec<types::ScheduledItem>, String> {
    let due = mail::scheduled_due(conn, now)?;
    for it in &due { mail::mark_scheduled(conn, it.id, "sending")?; }
    Ok(due)
}

pub fn settle_send(conn: &Connection, id: i64, result: &Result<String, String>) -> Result<(), String> {
    mail::mark_scheduled(conn, id, if result.is_ok() { "sent" } else { "failed" })
}

fn process_sends(now: i64) -> Result<(), String> {
    let due = mail::with_db(|c| claim_due_sends(c, now))?;
    for it in due {
        let result = mail::send_now(&it.draft);
        if let Err(x) = &result { eprintln!("iris: envoi planifié {} : {x}", it.id); notify("Envoi planifié échoué", &it.subject); }
        mail::with_db(|c| settle_send(c, it.id, &result))?;
        emit_changed(mail::SCHEDULED_FOLDER);
    }
    Ok(())
}

// ---------- Relances ----------

#[derive(Debug, PartialEq)]
pub struct ReminderJob { pub id: i64, pub replied: bool, pub origin: Option<(String, u32)>, pub subject: String }

fn like_escape(s: &str) -> String { s.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_") }

/// Une réponse (en-tête In-Reply-To ou References) cite-t-elle ce message ?
pub fn has_reply(conn: &Connection, acc: &str, message_id: &str) -> Result<bool, String> {
    conn.query_row(
        "SELECT EXISTS(SELECT 1 FROM message WHERE account=?1 AND COALESCE(message_id,'')<>?2
           AND (in_reply_to=?2 OR (' '||COALESCE(refs,'')||' ') LIKE ?3 ESCAPE '\\'))",
        params![acc, message_id, format!("% {} %", like_escape(message_id))], |r| r.get(0)).map_err(e)
}

/// Relances échues, marquées `replied` ou `done` ; l'origine est le message en cache portant ce Message-ID.
pub fn claim_due_reminders(conn: &Connection, acc: &str, now: i64) -> Result<Vec<ReminderJob>, String> {
    let mut st = conn.prepare("SELECT id, COALESCE(message_id,'') FROM reminder WHERE status='pending' AND due_at<=?1 ORDER BY due_at, id").map_err(e)?;
    let due: Vec<(i64, String)> = st.query_map(params![now], |r| Ok((r.get(0)?, r.get(1)?))).map_err(e)?.collect::<Result<_, _>>().map_err(e)?;
    let mut jobs = Vec::new();
    for (id, mid) in due {
        let replied = !mid.is_empty() && has_reply(conn, acc, &mid)?;
        let origin: Option<(String, u32, String)> = conn.query_row(
            "SELECT folder, uid, COALESCE(subject,'') FROM message WHERE account=?1 AND message_id=?2 LIMIT 1", params![acc, mid],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).optional().map_err(e)?;
        conn.execute("UPDATE reminder SET status=?2 WHERE id=?1", params![id, if replied { "replied" } else { "done" }]).map_err(e)?;
        let (origin, subject) = match origin { Some((f, u, s)) => (Some((f, u)), s), None => (None, String::new()) };
        jobs.push(ReminderJob { id, replied, origin, subject });
    }
    Ok(jobs)
}

fn process_reminders(now: i64) -> Result<(), String> {
    let acc = account()?;
    let jobs = mail::with_db(|c| claim_due_reminders(c, &acc, now))?;
    for job in jobs.into_iter().filter(|j| !j.replied) {
        if let Some((folder, uid)) = &job.origin {
            if let Err(x) = mail::bulk_move(folder, &[*uid], REMIND_FOLDER) { eprintln!("iris: déplacement vers {REMIND_FOLDER} : {x}"); }
            emit_changed(folder);
        }
        notify("Sans réponse", if job.subject.is_empty() { "Un message attend une réponse" } else { &job.subject });
    }
    Ok(())
}

// ---------- Boucle ----------

fn tick() {
    if config::load().is_err() { return; }
    let (now_ts, now) = (mail::now(), Local::now());
    if let Err(x) = process_sends(now_ts) { eprintln!("iris: envois planifiés : {x}"); }
    match mail::with_db(|c| claim_due_chrons(c, &now)) {
        Ok(kinds) => for k in kinds { if let Err(x) = run_chron(k) { eprintln!("iris: chron {k:?} : {x}"); } },
        Err(x) => eprintln!("iris: chrons : {x}"),
    }
}

/// Branche l'automatisation : rappel après synchronisation (règles, contacts), chrons par défaut,
/// puis un thread qui exécute envois planifiés et chrons toutes les 30 s (tant que l'application tourne).
pub fn start(app: &tauri::AppHandle) {
    let _ = APP.set(app.clone());
    mail::set_after_sync(after_sync);
    if let Err(x) = mail::with_db(|c| seed_defaults(c, mail::now())) { eprintln!("iris: chrons par défaut : {x}"); }
    let spawned = std::thread::Builder::new().name("iris-scheduler".into()).spawn(|| loop {
        std::thread::sleep(TICK);
        tick();
    });
    if let Err(x) = spawned { eprintln!("iris: planificateur non démarré : {x}"); }
}

// ---------- Expéditeurs récurrents et nouveaux ----------

pub struct Sender {
    pub name: String, pub hist: SenderHistory, pub last_subject: String, pub last_date: i64, pub last_folder: String,
    pub unread: u32, pub can_unsubscribe: bool, pub in_inbox: bool,
}

fn is_mailbox_folder(folder: &str) -> bool {
    !matches!(mail::role_of(folder, &[]), Some(mail::Role::Sent | mail::Role::Drafts | mail::Role::Trash | mail::Role::Junk))
}

type SenderRow = (String, String, String, String, i64, bool, bool, bool);

/// Historique par expéditeur (adresse en minuscules) sur le cache, hors envoyés, brouillons, corbeille et indésirables.
/// Le cache ne garde pas les noms de pièces jointes : toute pièce jointe est comptée comme un `pdf`.
pub fn senders(conn: &Connection, acc: &str) -> Result<Vec<Sender>, String> {
    let mut st = conn.prepare(
        "SELECT folder, LOWER(COALESCE(from_addr,'')), COALESCE(from_name,''), COALESCE(subject,''), COALESCE(date,0),
                COALESCE(seen,0), COALESCE(has_attach,0), COALESCE(list_unsubscribe,'')<>''
         FROM message WHERE account=?1 ORDER BY date, rowid").map_err(e)?;
    let rows: Vec<SenderRow> = st.query_map(params![acc], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?, r.get(7)?)))
        .map_err(e)?.collect::<Result<_, _>>().map_err(e)?;
    let mut map: HashMap<String, Sender> = HashMap::new();
    for (folder, addr, name, subject, date, seen, attach, unsub) in rows {
        if addr.is_empty() || addr == acc || !is_mailbox_folder(&folder) { continue; }
        let s = map.entry(addr.clone()).or_insert_with(|| Sender {
            name: String::new(), hist: SenderHistory { addr, ..Default::default() }, last_subject: String::new(), last_date: 0,
            last_folder: String::new(), unread: 0, can_unsubscribe: false, in_inbox: false,
        });
        if !name.is_empty() { s.name = name; }
        s.hist.subjects.push(subject.clone());
        s.hist.dates.push(date);
        s.hist.has_list_unsubscribe |= unsub;
        if attach && s.hist.attachment_exts.is_empty() { s.hist.attachment_exts.push("pdf".into()); }
        s.in_inbox |= folder.eq_ignore_ascii_case(INBOX);
        (s.last_subject, s.last_date, s.last_folder) = (subject, date, folder);
        s.unread += u32::from(!seen);
        s.can_unsubscribe |= unsub;
    }
    let mut out: Vec<Sender> = map.into_values().collect();
    out.sort_by(|a, b| b.last_date.cmp(&a.last_date).then_with(|| a.hist.addr.cmp(&b.hist.addr)));
    Ok(out)
}

fn decisions(conn: &Connection) -> Result<HashMap<String, String>, String> {
    let mut st = conn.prepare("SELECT LOWER(addr), decision FROM sender_decision").map_err(e)?;
    let rows = st.query_map([], |r| Ok((r.get(0)?, r.get(1)?))).map_err(e)?;
    rows.collect::<Result<_, _>>().map_err(e)
}

fn kind_out(k: recurring::Kind) -> RecurringKind {
    match k {
        recurring::Kind::Newsletters => RecurringKind::Newsletters,
        recurring::Kind::Notifications => RecurringKind::Notifications,
        recurring::Kind::Documents => RecurringKind::Documents,
    }
}

/// Expéditeurs récurrents (≥ 3 messages détectés, ou classés « Récurrents » par l'utilisateur), hors bloqués.
pub fn recurring_list(conn: &Connection, acc: &str, kind: Option<RecurringKind>) -> Result<Vec<types::RecurringSender>, String> {
    let decided = decisions(conn)?;
    let mut out = Vec::new();
    for s in senders(conn, acc)? {
        let decision = decided.get(&s.hist.addr).map(String::as_str);
        if decision == Some("block") { continue; }
        let detected = recurring::classify(&s.hist).or_else(|| (decision == Some("recurring")).then_some(
            if s.hist.has_list_unsubscribe { recurring::Kind::Newsletters } else { recurring::Kind::Notifications }));
        let Some(k) = detected.map(kind_out).filter(|k| kind.is_none_or(|w| w == *k)) else { continue };
        out.push(types::RecurringSender {
            cadence: recurring::cadence(&s.hist.dates).label().to_string(), kind: k, last_subject: s.last_subject, unread: s.unread,
            filed_in: (!s.last_folder.eq_ignore_ascii_case(INBOX)).then_some(s.last_folder),
            can_unsubscribe: s.can_unsubscribe, name: s.name, addr: s.hist.addr,
        });
    }
    Ok(out)
}

/// Nouveaux expéditeurs de la Réception : ≤ 2 messages, ni contact enregistré, ni décision prise.
pub fn new_senders_list(conn: &Connection, acc: &str) -> Result<Vec<types::NewSender>, String> {
    let decided: HashSet<String> = decisions(conn)?.into_keys().collect();
    let mut st = conn.prepare("SELECT LOWER(email) FROM contact WHERE manual=1").map_err(e)?;
    let known: HashSet<String> = st.query_map([], |r| r.get(0)).map_err(e)?.collect::<Result<_, _>>().map_err(e)?;
    Ok(senders(conn, acc)?.into_iter()
        .filter(|s| s.in_inbox && recurring::is_new_sender(&s.hist.addr, s.hist.dates.len(), &known, &decided))
        .map(|s| types::NewSender { count: s.hist.dates.len() as u32, addr: s.hist.addr, name: s.name, last_subject: s.last_subject })
        .collect())
}

fn block_rule_name(addr: &str) -> String { format!("Bloquer {addr}") }

/// Enregistre la décision. `Block` crée (une fois) la règle « expéditeur → dossier indésirables », placée en tête ;
/// `Inbox` retire cette règle.
pub fn sender_decide(conn: &Connection, addr: &str, decision: SenderDecision, junk_folder: &str) -> Result<(), String> {
    let addr = addr.trim().to_lowercase();
    if !addr.contains('@') { return Err("adresse invalide".into()); }
    let label = match decision { SenderDecision::Block => "block", SenderDecision::Recurring => "recurring", SenderDecision::Inbox => "inbox" };
    conn.execute("INSERT INTO sender_decision(addr, decision) VALUES(?1,?2) ON CONFLICT(addr) DO UPDATE SET decision=excluded.decision", params![addr, label]).map_err(e)?;
    let name = block_rule_name(&addr);
    let existing: Option<i64> = conn.query_row("SELECT id FROM rule WHERE name=?1", params![name], |r| r.get(0)).optional().map_err(e)?;
    match (decision, existing) {
        (SenderDecision::Block, None) => {
            let first: i64 = conn.query_row("SELECT COALESCE(MIN(position),1)-1 FROM rule", [], |r| r.get(0)).map_err(e)?;
            rule_save(conn, types::Rule {
                id: None, name, enabled: true, position: first,
                cond: serde_json::json!({ "type": "sender_is", "value": addr }),
                action: serde_json::json!({ "type": "move_to", "folder": junk_folder }),
            }).map(|_| ())
        }
        (SenderDecision::Block, Some(id)) => rule_toggle(conn, id, true),
        (_, Some(id)) => rule_delete(conn, id),
        (_, None) => Ok(()),
    }
}

/// UID de la Réception (cache) venant de cet expéditeur.
pub fn inbox_uids_from(conn: &Connection, acc: &str, addr: &str) -> Result<Vec<u32>, String> {
    let mut st = conn.prepare("SELECT uid FROM message WHERE account=?1 AND folder=?2 AND LOWER(from_addr)=?3 ORDER BY uid").map_err(e)?;
    let rows = st.query_map(params![acc, INBOX, addr.trim().to_lowercase()], |r| r.get(0)).map_err(e)?;
    rows.collect::<Result<_, _>>().map_err(e)
}

/// Décision + déplacement des messages déjà en Réception quand on bloque.
pub fn decide_and_apply(addr: &str, decision: SenderDecision) -> Result<(), String> {
    let acc = account()?;
    let junk = if decision == SenderDecision::Block { special_folder("Indésirables", "Junk") } else { String::new() };
    let uids = mail::with_db(|c| {
        sender_decide(c, addr, decision, &junk)?;
        if decision == SenderDecision::Block { inbox_uids_from(c, &acc, addr) } else { Ok(vec![]) }
    })?;
    move_inbox(&uids, &junk)
}

// ---------- Désinscription ----------

#[derive(Debug, PartialEq)]
pub enum Unsub { Mailto { to: String, subject: String }, Https(String) }

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        let hex = if b[i] == b'%' { s.get(i + 1..i + 3).and_then(|h| u8::from_str_radix(h, 16).ok()) } else { None };
        match hex { Some(v) => { out.push(v); i += 3; } None => { out.push(b[i]); i += 1; } }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn mailto(entry: &str) -> Option<Unsub> {
    let rest = entry.get(..7).filter(|p| p.eq_ignore_ascii_case("mailto:")).map(|_| &entry[7..])?;
    let (to, query) = rest.split_once('?').unwrap_or((rest, ""));
    let to = percent_decode(to);
    let valid = to.contains('@') && !to.chars().any(|c| c.is_whitespace() || c.is_control() || matches!(c, ',' | ';' | '<' | '>'));
    let subject = query.split('&').filter_map(|kv| kv.split_once('=')).find(|(k, _)| k.eq_ignore_ascii_case("subject"))
        .map(|(_, v)| percent_decode(v)).filter(|s| !s.contains(['\r', '\n'])).unwrap_or_else(|| "unsubscribe".into());
    valid.then_some(Unsub::Mailto { to, subject })
}

/// Choix dans l'en-tête `List-Unsubscribe` (`<https://…>, <mailto:…>`) : adresse mailto d'abord, sinon lien https.
/// Les liens non https sont ignorés.
pub fn unsub_target(header: &str) -> Option<Unsub> {
    let entries: Vec<&str> = header.split(',').map(|p| p.trim().trim_start_matches('<').trim_end_matches('>').trim()).collect();
    entries.iter().find_map(|x| mailto(x)).or_else(|| entries.iter()
        .find(|x| x.get(..8).is_some_and(|p| p.eq_ignore_ascii_case("https://")) && !x.chars().any(char::is_whitespace))
        .map(|x| Unsub::Https((*x).to_string())))
}

pub fn latest_unsubscribe_header(conn: &Connection, acc: &str, addr: &str) -> Result<Option<String>, String> {
    conn.query_row("SELECT list_unsubscribe FROM message WHERE account=?1 AND LOWER(from_addr)=?2 AND COALESCE(list_unsubscribe,'')<>'' ORDER BY date DESC LIMIT 1",
        params![acc, addr.trim().to_lowercase()], |r| r.get(0)).optional().map_err(e)
}

/// mailto : message envoyé par Iris (`done`). https : lien renvoyé à l'interface (le cache ne garde pas
/// `List-Unsubscribe-Post`, donc jamais de POST one-click automatique).
pub fn unsubscribe(addr: &str) -> Result<types::UnsubscribeResult, String> {
    let acc = account()?;
    let header = mail::with_db(|c| latest_unsubscribe_header(c, &acc, addr))?.ok_or("aucun lien de désinscription pour cet expéditeur")?;
    match unsub_target(&header).ok_or("lien de désinscription non pris en charge")? {
        Unsub::Https(url) => Ok(types::UnsubscribeResult { done: false, open_url: Some(url) }),
        Unsub::Mailto { to, subject } => {
            let draft = types::Draft { to, cc: None, subject, body: "unsubscribe".into(), in_reply_to: None, attachments: vec![], remind_days: None, send_at: None };
            mail::send_now(&draft)?;
            Ok(types::UnsubscribeResult { done: true, open_url: None })
        }
    }
}

// ---------- Contacts ----------

/// Crée les contacts des expéditeurs reçus ; ne touche jamais un contact saisi à la main et ne
/// remplace un nom que s'il est vide.
pub fn upsert_contacts(conn: &Connection, own: &str, items: &[Incoming]) -> Result<(), String> {
    for m in items.iter().filter(|m| m.from_addr.contains('@') && !m.from_addr.eq_ignore_ascii_case(own)) {
        conn.execute(
            "INSERT INTO contact(name, email, manual) VALUES(?1, ?2, 0)
             ON CONFLICT(email) DO UPDATE SET name=CASE WHEN manual=0 AND COALESCE(name,'')='' THEN excluded.name ELSE name END",
            params![m.from_name, m.from_addr.trim().to_lowercase()]).map_err(e)?;
    }
    Ok(())
}

fn contact_of(r: &rusqlite::Row) -> rusqlite::Result<types::Contact> {
    Ok(types::Contact { id: r.get(0)?, name: r.get::<_, Option<String>>(1)?.unwrap_or_default(), email: r.get(2)?, notes: r.get(3)? })
}

pub fn contacts_list(conn: &Connection, query: Option<&str>) -> Result<Vec<types::Contact>, String> {
    let q = query.map(|q| q.trim().to_lowercase()).filter(|q| !q.is_empty());
    let pattern = q.map(|q| format!("%{}%", like_escape(&q))).unwrap_or_else(|| "%".into());
    let mut st = conn.prepare(
        "SELECT id, name, email, notes FROM contact WHERE LOWER(COALESCE(name,'')) LIKE ?1 ESCAPE '\\' OR LOWER(email) LIKE ?1 ESCAPE '\\'
         ORDER BY LOWER(CASE WHEN COALESCE(name,'')='' THEN email ELSE name END), email").map_err(e)?;
    let rows = st.query_map(params![pattern], contact_of).map_err(e)?;
    rows.collect::<Result<_, _>>().map_err(e)
}

fn contact_by_email(conn: &Connection, email: &str) -> Result<Option<types::Contact>, String> {
    conn.query_row("SELECT id, name, email, notes FROM contact WHERE email=?1", params![email], contact_of).optional().map_err(e)
}

pub fn contact_save(conn: &Connection, contact: types::Contact) -> Result<types::Contact, String> {
    let email = contact.email.trim().to_lowercase();
    if !email.contains('@') || email.contains(char::is_whitespace) { return Err("adresse e-mail invalide".into()); }
    let notes = contact.notes.filter(|n| !n.trim().is_empty());
    conn.execute(
        "INSERT INTO contact(name, email, notes, manual) VALUES(?1,?2,?3,1)
         ON CONFLICT(email) DO UPDATE SET name=excluded.name, notes=excluded.notes, manual=1",
        params![contact.name.trim(), email, notes]).map_err(e)?;
    contact_by_email(conn, &email)?.ok_or_else(|| "contact introuvable".into())
}

/// Règle qui vise cet expéditeur ou son domaine (condition de premier niveau).
fn rule_for(conn: &Connection, email: &str) -> Result<Option<types::Rule>, String> {
    let domain = email.rsplit_once('@').map(|(_, d)| d.to_string()).unwrap_or_default();
    for r in rules_list(conn)? {
        let Ok(c) = serde_json::from_value::<rules::Cond>(r.cond.clone()) else { continue };
        let hit = match c {
            rules::Cond::SenderIs { value } => value.trim().eq_ignore_ascii_case(email),
            rules::Cond::SenderDomain { value } => value.trim().trim_start_matches('@').eq_ignore_ascii_case(&domain),
            _ => false,
        };
        if hit { return Ok(Some(r)); }
    }
    Ok(None)
}

pub fn contact_get(conn: &Connection, acc: &str, email: &str) -> Result<types::ContactDetail, String> {
    let email = email.trim().to_lowercase();
    let contact = match contact_by_email(conn, &email)? {
        Some(c) => c,
        None => {
            let name: Option<String> = conn.query_row("SELECT from_name FROM message WHERE account=?1 AND LOWER(from_addr)=?2 AND COALESCE(from_name,'')<>'' ORDER BY date DESC LIMIT 1",
                params![acc, email], |r| r.get(0)).optional().map_err(e)?;
            types::Contact { id: None, name: name.unwrap_or_default(), email: email.clone(), notes: None }
        }
    };
    let mut st = conn.prepare(
        "SELECT thread_id, COALESCE(subject,''), MAX(date) FROM message WHERE account=?1 AND LOWER(from_addr)=?2 AND thread_id IS NOT NULL
         GROUP BY thread_id ORDER BY MAX(date) DESC LIMIT 20").map_err(e)?;
    let recent_threads = st.query_map(params![acc, email], |r| Ok(types::ThreadRef { thread_id: r.get(0)?, subject: r.get(1)?, date: r.get(2)? }))
        .map_err(e)?.collect::<Result<_, _>>().map_err(e)?;
    Ok(types::ContactDetail { rule: rule_for(conn, &email)?, contact, recent_threads })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use chrono::Utc;
    use serde_json::json;

    const T0: i64 = 1_800_000_000;
    const ACC: &str = "me@example.org";

    fn mem() -> Connection { db::open_in_memory().unwrap() }

    fn rule(name: &str, cond: serde_json::Value, action: serde_json::Value) -> types::Rule {
        types::Rule { id: None, name: name.into(), enabled: true, position: 0, cond, action }
    }

    fn pdf_invoice() -> types::Rule {
        rule("Factures", json!({"type":"all","of":[{"type":"has_attachment_ext","ext":"pdf"},{"type":"subject_contains","value":"facture"}]}),
            json!({"type":"move_to","folder":"Factures"}))
    }

    #[allow(clippy::too_many_arguments)]
    fn msg(c: &Connection, folder: &str, uid: u32, mid: &str, irt: &str, refs: &str, from: &str, subject: &str, date: i64, seen: bool, attach: bool, unsub: &str) {
        c.execute("INSERT INTO message(account,folder,uid,message_id,in_reply_to,refs,from_name,from_addr,subject,date,seen,has_attach,list_unsubscribe,thread_id)
                   VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
            params![ACC, folder, uid, mid, irt, refs, from.split('@').next().unwrap(), from, subject, date, seen, attach, unsub, format!("t-{mid}")]).unwrap();
    }

    fn simple(c: &Connection, folder: &str, uid: u32, from: &str, subject: &str, date: i64) {
        msg(c, folder, uid, &format!("m{uid}@x"), "", "", from, subject, date, false, false, "");
    }

    // ----- règles -----

    #[test]
    fn rule_crud_roundtrip() {
        let c = mem();
        let saved = rule_save(&c, pdf_invoice()).unwrap();
        assert_eq!((saved.id, saved.position, saved.enabled), (Some(1), 1, true));
        let second = rule_save(&c, rule("", json!({"type":"has_list_unsubscribe"}), json!({"type":"archive"}))).unwrap();
        assert_eq!(second.position, 2);
        assert_eq!(second.name, "Si le message contient un lien de désinscription, alors archiver");
        let mut edited = saved.clone();
        edited.name = "Factures PDF".into();
        edited.position = 5;
        assert_eq!(rule_save(&c, edited).unwrap().name, "Factures PDF");
        rule_toggle(&c, 1, false).unwrap();
        let all = rules_list(&c).unwrap();
        assert_eq!(all.iter().map(|r| (r.id.unwrap(), r.enabled)).collect::<Vec<_>>(), vec![(2, true), (1, false)]);
        assert_eq!(active_rules(&c).unwrap().len(), 1);
        rule_delete(&c, 2).unwrap();
        assert_eq!(rules_list(&c).unwrap().len(), 1);
        assert!(rule_toggle(&c, 99, true).is_err());
    }

    #[test]
    fn rule_save_validates() {
        let c = mem();
        let bad_cond = rule("x", json!({"type":"nope"}), json!({"type":"archive"}));
        assert!(rule_save(&c, bad_cond).unwrap_err().starts_with("condition invalide"));
        let http = rule("x", json!({"type":"has_list_unsubscribe"}), json!({"type":"webhook","url":"http://example.org/hook"}));
        assert!(rule_save(&c, http).unwrap_err().contains("https"));
        let local = rule("x", json!({"type":"has_list_unsubscribe"}), json!({"type":"webhook","url":"https://127.0.0.1/hook"}));
        assert!(rule_save(&c, local).is_err());
        let empty = rule("x", json!({"type":"has_list_unsubscribe"}), json!({"type":"move_to","folder":" "}));
        assert!(rule_save(&c, empty).is_err());
        let mut ghost = pdf_invoice();
        ghost.id = Some(42);
        assert_eq!(rule_save(&c, ghost).unwrap_err(), "règle introuvable");
        assert!(rules_list(&c).unwrap().is_empty());
    }

    #[test]
    fn rule_delete_clears_message_tag() {
        let c = mem();
        let r = rule_save(&c, pdf_invoice()).unwrap();
        simple(&c, "Factures", 1, "a@x.org", "facture", T0);
        c.execute("UPDATE message SET rule_id=?1", params![r.id]).unwrap();
        rule_delete(&c, r.id.unwrap()).unwrap();
        let tag: Option<i64> = c.query_row("SELECT rule_id FROM message", [], |r| r.get(0)).unwrap();
        assert_eq!(tag, None);
    }

    #[test]
    fn sentence_matches_rules_module() {
        let s = rule_sentence(&json!({"type":"sender_domain","value":"banque.fr"}), &json!({"type":"notify"})).unwrap();
        assert_eq!(s, "Si l'expéditeur est du domaine banque.fr, alors m'envoyer une notification");
        assert!(rule_sentence(&json!({"type":"x"}), &json!({"type":"notify"})).is_err());
    }

    fn incoming_msg(uid: u32, from: &str, subject: &str, attach: bool) -> Incoming {
        Incoming { uid, folder: INBOX.into(), message_id: format!("m{uid}@x"), thread_id: Some(format!("t{uid}")), from_name: String::new(),
            from_addr: from.into(), subject: subject.into(), date: T0, has_attach: attach, list_unsubscribe: false }
    }

    #[test]
    fn plan_for_pdf_invoice_example() {
        let c = mem();
        rule_save(&c, pdf_invoice()).unwrap();
        rule_save(&c, rule("Alerte", json!({"type":"sender_is","value":"alerte@banque.fr"}), json!({"type":"notify"}))).unwrap();
        rule_save(&c, rule("Hook", json!({"type":"sender_is","value":"alerte@banque.fr"}), json!({"type":"webhook","url":"https://example.org/h"}))).unwrap();
        rule_save(&c, rule("Suivi", json!({"type":"sender_is","value":"alerte@banque.fr"}), json!({"type":"remind","days":3}))).unwrap();
        let active = active_rules(&c).unwrap();
        let inv = incoming_msg(1, "compta@fournisseur.fr", "Facture 2026-14", true);
        let p = plan(&active, &facts(&inv, vec!["pdf".into()]), T0);
        assert_eq!(p, Plan { filing: Some((Filing::Folder("Factures".into()), 1)), ..Plan::default() });
        assert_eq!(plan(&active, &facts(&inv, vec![]), T0), Plan::default(), "sans pièce jointe .pdf la règle ne s'applique pas");
        let alert = incoming_msg(2, "alerte@banque.fr", "Prélèvement", false);
        let p = plan(&active, &facts(&alert, vec![]), T0);
        assert_eq!((p.notify, p.webhooks, p.remind_days, p.filing), (true, vec!["https://example.org/h".to_string()], Some(3), None));
    }

    #[test]
    fn first_filing_rule_wins_and_is_attributed() {
        let c = mem();
        rule_save(&c, rule("A", json!({"type":"sender_domain","value":"x.org"}), json!({"type":"move_to","folder":"Pro"}))).unwrap();
        rule_save(&c, rule("B", json!({"type":"sender_domain","value":"x.org"}), json!({"type":"archive"}))).unwrap();
        let active = active_rules(&c).unwrap();
        let p = plan(&active, &facts(&incoming_msg(1, "a@x.org", "s", false), vec![]), T0);
        assert_eq!(p.filing, Some((Filing::Folder("Pro".into()), 1)));
    }

    #[test]
    fn sorted_folder_tags_matching_rule() {
        let c = mem();
        rule_save(&c, pdf_invoice()).unwrap();
        rule_save(&c, rule("Archive", json!({"type":"has_list_unsubscribe"}), json!({"type":"archive"}))).unwrap();
        let active = active_rules(&c).unwrap();
        let f = facts(&incoming_msg(1, "a@x.org", "Facture", true), vec!["pdf".into()]);
        assert_eq!(sorting_rule(&active, "Factures", &f, T0), Some(1));
        assert_eq!(sorting_rule(&active, "Autre", &f, T0), None);
        let mut news = incoming_msg(2, "n@x.org", "Hebdo", false);
        news.list_unsubscribe = true;
        assert_eq!(sorting_rule(&active, "Archives", &facts(&news, vec![]), T0), Some(2));
        assert_eq!(sorting_rule(&active, "Factures", &facts(&news, vec![]), T0), None);
    }

    #[test]
    fn notification_text_single_and_batch() {
        let mut one = incoming_msg(1, "a@x.org", "Bonjour", false);
        assert_eq!(notification_text(&[&one]), ("a@x.org".into(), "Bonjour".into()));
        one.from_name = "Amélie".into();
        let two = incoming_msg(2, "b@x.org", "Salut", false);
        assert_eq!(notification_text(&[&one, &two]), ("Iris".into(), "2 nouveaux messages".into()));
        assert_eq!(notification_text(&[&one]).0, "Amélie");
    }

    #[test]
    fn webhooks_refuse_non_https_without_network() {
        assert!(post_webhook("http://example.org/h", &json!({})).unwrap_err().contains("https"));
        assert!(post_webhook("https://localhost/h", &json!({})).is_err());
        assert!(post_webhook("https://10.0.0.1/h", &json!({})).is_err());
    }

    #[test]
    fn incoming_reads_cache_rows() {
        let c = mem();
        msg(&c, INBOX, 7, "m7@x", "", "", "a@x.org", "Sujet", T0, false, true, "<https://u>");
        let v = incoming(&c, ACC, INBOX, &[7, 8]).unwrap();
        assert_eq!(v.len(), 1);
        assert_eq!((v[0].uid, v[0].subject.as_str(), v[0].has_attach, v[0].list_unsubscribe), (7, "Sujet", true, true));
    }

    // ----- chrons -----

    fn utc(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> DateTime<Utc> { Utc.with_ymd_and_hms(y, mo, d, h, mi, 0).unwrap() }

    #[test]
    fn seeds_four_defaults_once_monthly_paused() {
        let c = mem();
        let now = utc(2026, 10, 30, 10, 0);
        seed_defaults(&c, now.timestamp()).unwrap();
        seed_defaults(&c, now.timestamp()).unwrap();
        let list = chrons_list(&c, &now).unwrap();
        assert_eq!(list.len(), 4);
        let kinds: Vec<_> = list.iter().map(|x| (x.kind, x.enabled)).collect();
        assert_eq!(kinds, vec![(ChronKind::MorningDigest, true), (ChronKind::Reminders, true), (ChronKind::NewsletterCleanup, true), (ChronKind::MonthlyInvoices, false)]);
        assert_eq!(list[0].description, "Du lundi au vendredi à 08:00");
        assert_eq!(list[0].next_run, Some(utc(2026, 11, 2, 8, 0).timestamp()), "vendredi 10:00 -> lundi 08:00");
        assert_eq!(list[3].next_run, None);
    }

    fn chron(name: &str, cron: &str, kind: ChronKind) -> types::Chron {
        types::Chron { id: None, name: name.into(), cron: cron.into(), kind, enabled: true, last_run: None, next_run: None, description: String::new() }
    }

    #[test]
    fn chron_save_validates_and_updates() {
        let c = mem();
        let now = utc(2026, 10, 30, 10, 0);
        assert!(chron_save(&c, chron("x", "pas un cron", ChronKind::Reminders), &now).is_err());
        let saved = chron_save(&c, chron("", "0 9 * * *", ChronKind::Reminders), &now).unwrap();
        assert_eq!((saved.name.as_str(), saved.last_run, saved.description.as_str()), ("Tous les jours à 09:00", Some(now.timestamp()), "Tous les jours à 09:00"));
        let mut edit = saved.clone();
        edit.cron = "0 20 * * 0".into();
        edit.enabled = false;
        let later = utc(2026, 10, 31, 10, 0);
        let upd = chron_save(&c, edit, &later).unwrap();
        assert_eq!((upd.cron.as_str(), upd.enabled, upd.last_run, upd.next_run), ("0 20 * * 0", false, Some(later.timestamp()), None));
        chron_toggle(&c, saved.id.unwrap(), true, later.timestamp()).unwrap();
        chron_delete(&c, saved.id.unwrap()).unwrap();
        assert!(chrons_list(&c, &now).unwrap().is_empty());
        assert!(chron_toggle(&c, 99, true, 0).is_err());
    }

    #[test]
    fn due_chrons_run_once_and_skip_stale_misses() {
        let c = mem();
        seed_defaults(&c, utc(2026, 10, 29, 7, 0).timestamp()).unwrap(); // jeudi 07:00
        // vendredi 09:05 : le résumé (vendredi 08:00) et les relances (vendredi 09:00) sont échus
        let now = utc(2026, 10, 30, 9, 5);
        let mut kinds = claim_due_chrons(&c, &now).unwrap();
        kinds.sort_by_key(|k| kind_str(*k));
        assert_eq!(kinds, vec![ChronKind::MorningDigest, ChronKind::Reminders]);
        assert!(claim_due_chrons(&c, &now).unwrap().is_empty(), "déjà marqués");
        assert_eq!(chrons_list(&c, &now).unwrap()[0].last_run, Some(now.timestamp()));
        // application fermée jusqu'au dimanche 20 décembre 12:00 : seule l'échéance de moins de 24 h (relances 09:00) est jouée
        let long_after = utc(2026, 12, 20, 12, 0);
        assert_eq!(claim_due_chrons(&c, &long_after).unwrap(), vec![ChronKind::Reminders]);
        assert!(claim_due_chrons(&c, &long_after).unwrap().is_empty());
    }

    #[test]
    fn disabled_chron_is_never_due() {
        let c = mem();
        seed_defaults(&c, utc(2026, 9, 1, 0, 0).timestamp()).unwrap();
        let kinds = claim_due_chrons(&c, &utc(2026, 10, 30, 7, 30)).unwrap();
        assert!(!kinds.contains(&ChronKind::MonthlyInvoices));
    }

    #[test]
    fn cron_describe_ok_and_error() {
        let ok = cron_describe("0 8 * * 1-5");
        assert!(ok.ok && ok.error.is_none());
        assert_eq!((ok.description.as_deref(), ok.next_runs.len()), (Some("Du lundi au vendredi à 08:00"), 3));
        assert!(DateTime::parse_from_rfc3339(&ok.next_runs[0]).is_ok());
        let bad = cron_describe("99 * * * *");
        assert!(!bad.ok && bad.description.is_none() && bad.next_runs.is_empty() && bad.error.is_some());
    }

    #[test]
    fn chron_tasks_pick_the_right_messages() {
        let c = mem();
        assert_eq!(digest_text(0), None);
        assert_eq!(digest_text(1).unwrap(), "1 message non lu dans votre boîte de réception");
        assert_eq!(digest_text(5).unwrap(), "5 messages non lus dans votre boîte de réception");
        simple(&c, INBOX, 1, "a@x.org", "non lu", T0);
        msg(&c, INBOX, 2, "m2", "", "", "n@j.org", "vieux lu", T0 - 10 * DAY, true, false, "<https://u>");
        msg(&c, INBOX, 3, "m3", "", "", "n@j.org", "récent lu", T0 - DAY, true, false, "<https://u>");
        msg(&c, INBOX, 4, "m4", "", "", "n@j.org", "vieux non lu", T0 - 10 * DAY, false, false, "<https://u>");
        msg(&c, "Archive", 5, "m5", "", "", "n@j.org", "ailleurs", T0 - 10 * DAY, true, false, "<https://u>");
        assert_eq!(unread_count(&c, ACC, INBOX).unwrap(), 2);
        assert_eq!(cleanup_candidates(&c, ACC, T0).unwrap(), vec![2]);
    }

    #[test]
    fn invoice_candidates_need_documents_sender_attachment_recent() {
        let c = mem();
        for (i, d) in [60, 30, 1].iter().enumerate() {
            msg(&c, INBOX, i as u32 + 1, &format!("f{i}"), "", "", "compta@edf.fr", "Facture mensuelle", T0 - d * DAY, true, true, "");
        }
        msg(&c, INBOX, 10, "x", "", "", "ami@x.org", "photo", T0 - DAY, false, true, "");
        assert_eq!(invoice_candidates(&c, ACC, T0).unwrap(), vec![2, 3]);
    }

    // ----- envois planifiés et relances -----

    fn draft(to: &str) -> types::Draft {
        types::Draft { to: to.into(), cc: None, subject: "Sujet".into(), body: "Corps".into(), in_reply_to: None, attachments: vec![], remind_days: None, send_at: None }
    }

    #[test]
    fn due_sends_are_claimed_once_and_settled() {
        let c = mem();
        let due = mail::schedule(&c, &draft("a@x.org"), T0 - 10).unwrap();
        let later = mail::schedule(&c, &draft("b@x.org"), T0 + 3600).unwrap();
        let cancelled = mail::schedule(&c, &draft("c@x.org"), T0 - 5).unwrap();
        mail::scheduled_cancel(&c, cancelled).unwrap();
        let claimed = claim_due_sends(&c, T0).unwrap();
        assert_eq!(claimed.iter().map(|i| i.id).collect::<Vec<_>>(), vec![due]);
        assert!(claim_due_sends(&c, T0).unwrap().is_empty(), "un envoi réclamé ne repart pas");
        settle_send(&c, due, &Ok("id@x".into())).unwrap();
        let status = |id: i64| -> String { c.query_row("SELECT status FROM scheduled_send WHERE id=?1", params![id], |r| r.get(0)).unwrap() };
        assert_eq!((status(due).as_str(), status(later).as_str()), ("sent", "pending"));
        settle_send(&c, later, &Err("smtp".into())).unwrap();
        assert_eq!(status(later), "failed");
    }

    #[test]
    fn reminders_detect_replies_and_origin() {
        let c = mem();
        msg(&c, "Sent", 1, "orig1@x", "", "", ACC, "Devis", T0 - 5 * DAY, true, false, "");
        msg(&c, "Sent", 2, "orig2@x", "", "", ACC, "Contrat", T0 - 5 * DAY, true, false, "");
        msg(&c, INBOX, 3, "rep2@x", "orig2@x", "orig2@x", "client@x.org", "Re: Contrat", T0 - DAY, false, false, "");
        mail::add_reminder(&c, "orig1@x", T0 - 60).unwrap();
        mail::add_reminder(&c, "orig2@x", T0 - 30).unwrap();
        mail::add_reminder(&c, "future@x", T0 + 3600).unwrap();
        mail::add_reminder(&c, "inconnu@x", T0 - 10).unwrap();
        let jobs = claim_due_reminders(&c, ACC, T0).unwrap();
        assert_eq!(jobs, vec![
            ReminderJob { id: 1, replied: false, origin: Some(("Sent".into(), 1)), subject: "Devis".into() },
            ReminderJob { id: 2, replied: true, origin: Some(("Sent".into(), 2)), subject: "Contrat".into() },
            ReminderJob { id: 4, replied: false, origin: None, subject: String::new() },
        ]);
        assert!(claim_due_reminders(&c, ACC, T0).unwrap().is_empty(), "relances traitées une seule fois");
        let st: Vec<String> = c.prepare("SELECT status FROM reminder ORDER BY id").unwrap().query_map([], |r| r.get(0)).unwrap().map(Result::unwrap).collect();
        assert_eq!(st, vec!["done", "replied", "pending", "done"]);
    }

    #[test]
    fn has_reply_matches_references_but_not_wildcards() {
        let c = mem();
        msg(&c, INBOX, 1, "r@x", "autre@x", "a@x b@x orig@x", "p@x.org", "Re", T0, false, false, "");
        assert!(has_reply(&c, ACC, "orig@x").unwrap());
        assert!(!has_reply(&c, ACC, "or_g@x").unwrap());
        assert!(!has_reply(&c, ACC, "%").unwrap());
        assert!(!has_reply(&c, ACC, "r@x").unwrap());
    }

    // ----- récurrents, nouveaux, décisions -----

    #[test]
    fn recurring_and_new_senders() {
        let c = mem();
        for i in 0..4u32 {
            msg(&c, INBOX, i + 1, &format!("n{i}"), "", "", "news@journal.org", &format!("Édition {i}"), T0 - i64::from(3 - i) * 7 * DAY, i < 2, false, "<https://journal.org/u>");
        }
        for i in 0..3u32 {
            msg(&c, "Factures", i + 10, &format!("f{i}"), "", "", "compta@edf.fr", "Facture d'électricité", T0 - 2 * DAY - i64::from(2 - i) * 30 * DAY, true, true, "");
        }
        simple(&c, INBOX, 20, "nouveau@x.org", "Bonjour", T0);
        simple(&c, INBOX, 21, "ancien@x.org", "Salut", T0 - DAY);
        simple(&c, "Sent", 22, "autre@x.org", "envoyé", T0);
        let all = recurring_list(&c, ACC, None).unwrap();
        assert_eq!(all.iter().map(|r| r.addr.as_str()).collect::<Vec<_>>(), vec!["news@journal.org", "compta@edf.fr"]);
        let news = &all[0];
        assert_eq!((news.kind, news.cadence.as_str(), news.last_subject.as_str(), news.unread, news.can_unsubscribe, news.filed_in.as_deref()),
            (RecurringKind::Newsletters, "Hebdo", "Édition 3", 2, true, None));
        let docs = recurring_list(&c, ACC, Some(RecurringKind::Documents)).unwrap();
        assert_eq!((docs.len(), docs[0].filed_in.as_deref(), docs[0].cadence.as_str()), (1, Some("Factures"), "Mensuel"));
        let new = new_senders_list(&c, ACC).unwrap();
        assert_eq!(new.iter().map(|n| (n.addr.as_str(), n.count)).collect::<Vec<_>>(), vec![("nouveau@x.org", 1), ("ancien@x.org", 1)]);
    }

    #[test]
    fn decisions_shape_lists_and_rules() {
        let c = mem();
        simple(&c, INBOX, 1, "spam@x.org", "Promo", T0);
        simple(&c, INBOX, 2, "reel@x.org", "Hello", T0 - 1);
        sender_decide(&c, "Spam@X.org", SenderDecision::Block, "Junk").unwrap();
        sender_decide(&c, "spam@x.org", SenderDecision::Block, "Junk").unwrap();
        let r = rules_list(&c).unwrap();
        assert_eq!(r.len(), 1, "une seule règle de blocage");
        assert_eq!((r[0].name.as_str(), &r[0].cond, &r[0].action), ("Bloquer spam@x.org", &json!({"type":"sender_is","value":"spam@x.org"}), &json!({"type":"move_to","folder":"Junk"})));
        assert_eq!(inbox_uids_from(&c, ACC, "SPAM@x.org").unwrap(), vec![1]);
        assert_eq!(new_senders_list(&c, ACC).unwrap().iter().map(|n| n.addr.clone()).collect::<Vec<_>>(), vec!["reel@x.org"]);
        sender_decide(&c, "reel@x.org", SenderDecision::Recurring, "").unwrap();
        assert!(new_senders_list(&c, ACC).unwrap().is_empty());
        let rec = recurring_list(&c, ACC, None).unwrap();
        assert_eq!((rec.len(), rec[0].addr.as_str(), rec[0].kind), (1, "reel@x.org", RecurringKind::Notifications));
        sender_decide(&c, "spam@x.org", SenderDecision::Inbox, "").unwrap();
        assert!(rules_list(&c).unwrap().is_empty(), "revenir en Réception retire la règle");
        assert!(sender_decide(&c, "pas-une-adresse", SenderDecision::Block, "Junk").is_err());
    }

    #[test]
    fn unsubscribe_target_prefers_mailto_and_rejects_http() {
        assert_eq!(unsub_target("<https://j.org/u/abc>, <mailto:unsub@j.org?subject=Stop%20me>"),
            Some(Unsub::Mailto { to: "unsub@j.org".into(), subject: "Stop me".into() }));
        assert_eq!(unsub_target("<mailto:u@j.org>"), Some(Unsub::Mailto { to: "u@j.org".into(), subject: "unsubscribe".into() }));
        assert_eq!(unsub_target("<https://j.org/u/abc>"), Some(Unsub::Https("https://j.org/u/abc".into())));
        assert_eq!(unsub_target("<http://j.org/u>"), None);
        assert_eq!(unsub_target("<mailto:a b@j.org>, <https://j.org/x>"), Some(Unsub::Https("https://j.org/x".into())));
        assert_eq!(unsub_target("<mailto:u@j.org?subject=a%0D%0ABcc:x@y.z>"), Some(Unsub::Mailto { to: "u@j.org".into(), subject: "unsubscribe".into() }));
        assert_eq!(unsub_target(""), None);
    }

    #[test]
    fn latest_unsubscribe_header_is_newest() {
        let c = mem();
        msg(&c, INBOX, 1, "a", "", "", "n@j.org", "1", T0 - 10, true, false, "<https://old>");
        msg(&c, INBOX, 2, "b", "", "", "N@J.org", "2", T0, true, false, "<https://new>");
        assert_eq!(latest_unsubscribe_header(&c, ACC, "n@j.org").unwrap().as_deref(), Some("<https://new>"));
        assert_eq!(latest_unsubscribe_header(&c, ACC, "x@j.org").unwrap(), None);
    }

    // ----- contacts -----

    #[test]
    fn contacts_upsert_keeps_manual_entries() {
        let c = mem();
        let mut a = incoming_msg(1, "Amelie@X.org", "s", false);
        a.from_name = "Amélie".into();
        upsert_contacts(&c, ACC, &[a.clone(), incoming_msg(2, ACC, "moi", false), incoming_msg(3, "pas-une-adresse", "s", false)]).unwrap();
        assert_eq!(contacts_list(&c, None).unwrap().len(), 1);
        let saved = contact_save(&c, types::Contact { id: None, name: "Amélie D.".into(), email: " AMELIE@x.org ".into(), notes: Some("cliente".into()) }).unwrap();
        assert_eq!((saved.name.as_str(), saved.email.as_str(), saved.notes.as_deref()), ("Amélie D.", "amelie@x.org", Some("cliente")));
        a.from_name = "Autre nom".into();
        upsert_contacts(&c, ACC, &[a]).unwrap();
        let list = contacts_list(&c, None).unwrap();
        assert_eq!((list.len(), list[0].name.as_str(), list[0].id), (1, "Amélie D.", saved.id));
        assert!(contact_save(&c, types::Contact { id: None, name: "x".into(), email: "nope".into(), notes: None }).is_err());
    }

    #[test]
    fn contacts_search_and_sort() {
        let c = mem();
        for (n, m) in [("Zoé", "zoe@x.org"), ("", "alice@x.org"), ("Bob", "bob@y.org")] {
            contact_save(&c, types::Contact { id: None, name: n.into(), email: m.into(), notes: None }).unwrap();
        }
        let names = |q: Option<&str>| contacts_list(&c, q).unwrap().into_iter().map(|x| x.email).collect::<Vec<_>>();
        assert_eq!(names(None), vec!["alice@x.org", "bob@y.org", "zoe@x.org"]);
        assert_eq!(names(Some("x.org")), vec!["alice@x.org", "zoe@x.org"]);
        assert_eq!(names(Some("BOB")), vec!["bob@y.org"]);
        assert!(names(Some("%")).is_empty());
    }

    #[test]
    fn contact_detail_has_threads_and_rule() {
        let c = mem();
        msg(&c, INBOX, 1, "a", "", "", "amelie@x.org", "Projet", T0 - 100, true, false, "");
        msg(&c, INBOX, 2, "b", "", "", "amelie@x.org", "Devis", T0, true, false, "");
        msg(&c, INBOX, 3, "c", "", "", "autre@x.org", "Hors sujet", T0, true, false, "");
        rule_save(&c, rule("Pro", json!({"type":"sender_domain","value":"X.org"}), json!({"type":"move_to","folder":"Pro"}))).unwrap();
        let d = contact_get(&c, ACC, "Amelie@x.org").unwrap();
        assert_eq!((d.contact.id, d.contact.email.as_str(), d.contact.name.as_str()), (None, "amelie@x.org", "amelie"));
        assert_eq!(d.recent_threads.iter().map(|t| t.subject.as_str()).collect::<Vec<_>>(), vec!["Devis", "Projet"]);
        assert_eq!(d.rule.unwrap().name, "Pro");
        let saved = contact_save(&c, types::Contact { id: None, name: "Amélie".into(), email: "amelie@x.org".into(), notes: None }).unwrap();
        assert_eq!(contact_get(&c, ACC, "amelie@x.org").unwrap().contact.id, saved.id);
        assert!(contact_get(&c, ACC, "inconnu@y.org").unwrap().rule.is_none());
    }
}
