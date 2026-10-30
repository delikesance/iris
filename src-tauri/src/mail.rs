use crate::config::{self, Config, Security};
use crate::types::*;
use crate::{db, signing, threading};
use lettre::message::{header::ContentType, Mailbox, MultiPart, SinglePart};
use lettre::{transport::smtp::authentication::Credentials, Message as OutMessage, SmtpTransport, Transport};
use mail_parser::{HeaderValue, Message, MessageParser, MimeHeaders};
use rusqlite::{params, Connection};
use std::{collections::{HashMap, HashSet}, io::{Read, Write}, net::{IpAddr, TcpStream}, path::{Path, PathBuf}, sync::{Mutex, OnceLock}};

/// Flux IMAP : TLS (implicite ou STARTTLS) ou texte clair (hôtes locaux/privés uniquement).
enum Stream { Tls(native_tls::TlsStream<TcpStream>), Plain(TcpStream) }

impl Read for Stream {
    fn read(&mut self, b: &mut [u8]) -> std::io::Result<usize> {
        match self { Stream::Tls(s) => s.read(b), Stream::Plain(s) => s.read(b) }
    }
}
impl Write for Stream {
    fn write(&mut self, b: &[u8]) -> std::io::Result<usize> {
        match self { Stream::Tls(s) => s.write(b), Stream::Plain(s) => s.write(b) }
    }
    fn flush(&mut self) -> std::io::Result<()> {
        match self { Stream::Tls(s) => s.flush(), Stream::Plain(s) => s.flush() }
    }
}

type Session = imap::Session<Stream>;

fn e<T: std::fmt::Display>(x: T) -> String { x.to_string() }

/// Le texte clair n'est toléré que pour localhost ou une adresse privée / locale.
pub fn plaintext_allowed(host: &str) -> bool {
    if host.eq_ignore_ascii_case("localhost") { return true; }
    match host.parse::<IpAddr>() {
        Ok(IpAddr::V4(a)) => a.is_loopback() || a.is_private() || a.is_link_local(),
        Ok(IpAddr::V6(a)) => a.is_loopback() || (a.segments()[0] & 0xfe00) == 0xfc00 || (a.segments()[0] & 0xffc0) == 0xfe80,
        Err(_) => false,
    }
}

fn require_plaintext_ok(host: &str, security: Security) -> Result<(), String> {
    if security == Security::None && !plaintext_allowed(host) {
        return Err(format!("connexion non chiffrée refusée pour {host} (réservée aux hôtes locaux)"));
    }
    Ok(())
}

/// Lit une ligne IMAP octet par octet (aucune donnée lue au-delà : le flux passe ensuite en TLS).
fn read_line(s: &mut TcpStream) -> Result<String, String> {
    let (mut line, mut b) = (Vec::new(), [0u8; 1]);
    while !line.ends_with(b"\r\n") {
        if s.read(&mut b).map_err(e)? == 0 { return Err("connexion fermée par le serveur".into()); }
        line.push(b[0]);
    }
    Ok(String::from_utf8_lossy(&line).into_owned())
}

/// Négocie STARTTLS à la main puis renvoie le flux TCP prêt pour la poignée de main TLS.
fn starttls_negotiate(mut tcp: TcpStream) -> Result<TcpStream, String> {
    read_line(&mut tcp)?;
    tcp.write_all(b"iris0 STARTTLS\r\n").map_err(e)?;
    loop {
        let l = read_line(&mut tcp)?;
        if let Some(rest) = l.strip_prefix("iris0 ") {
            return if rest.starts_with("OK") { Ok(tcp) } else { Err(format!("STARTTLS refusé : {}", rest.trim())) };
        }
    }
}

fn open_stream(c: &Config) -> Result<(Stream, bool), String> {
    require_plaintext_ok(&c.imap_host, c.imap_security)?;
    let tcp = TcpStream::connect((c.imap_host.as_str(), c.imap_port)).map_err(e)?;
    let tls = || native_tls::TlsConnector::builder().build().map_err(e);
    match c.imap_security {
        Security::Ssl => Ok((Stream::Tls(tls()?.connect(&c.imap_host, tcp).map_err(e)?), true)),
        Security::Starttls => {
            let tcp = starttls_negotiate(tcp)?;
            Ok((Stream::Tls(tls()?.connect(&c.imap_host, tcp).map_err(e)?), false))
        }
        Security::None => Ok((Stream::Plain(tcp), true)),
    }
}

fn connect(c: &Config) -> Result<Session, String> {
    let (stream, greeting_pending) = open_stream(c)?;
    let mut client = imap::Client::new(stream);
    if greeting_pending { client.read_greeting().map_err(e)?; }
    client.login(&c.user, &c.pass).map_err(|x| x.0.to_string())
}


static SESSION: std::sync::Mutex<Option<Session>> = std::sync::Mutex::new(None);

/// Session IMAP persistante : évite un handshake TLS + login à chaque action. Reconnecte si la connexion est tombée.
fn with<T>(f: impl Fn(&mut Session) -> Result<T, String>) -> Result<T, String> {
    let mut g = SESSION.lock().unwrap_or_else(|p| p.into_inner());
    let mut last = String::new();
    for _ in 0..2 {
        if g.is_none() { *g = Some(connect(&config::load()?)?); }
        match f(g.as_mut().unwrap()) {
            Ok(v) => return Ok(v),
            Err(e) => { last = e; *g = None; }
        }
    }
    Err(last)
}

pub fn reset() { *SESSION.lock().unwrap_or_else(|p| p.into_inner()) = None; }

// ---------- Base de données partagée ----------

static DB: Mutex<Option<Connection>> = Mutex::new(None);

/// Même dossier que `config::dir()` (`<config>/iris`), qui est privé à config.rs.
fn db_path() -> PathBuf {
    let base = std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from)
        .or_else(|| std::env::var_os("APPDATA").map(PathBuf::from))
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."));
    base.join("iris").join("iris.db")
}

/// Connexion SQLite unique du processus (`<config>/iris/iris.db`), ouverte et migrée au premier appel.
pub fn with_db<T>(f: impl FnOnce(&Connection) -> Result<T, String>) -> Result<T, String> {
    let mut g = DB.lock().unwrap_or_else(|p| p.into_inner());
    if g.is_none() { *g = Some(db::open(&db_path())?); }
    f(g.as_ref().unwrap())
}

fn account() -> Result<String, String> { Ok(config::load()?.user.to_lowercase()) }

pub fn now() -> i64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0)
}

/// Dossier virtuel des envois planifiés (table `scheduled_send`).
pub const SCHEDULED_FOLDER: &str = "Planifiés";

// ---------- Analyse des messages (fonctions pures) ----------

/// En-têtes utiles d'un message, tels que mis en cache.
#[derive(Debug, Clone, PartialEq)]
pub struct Header {
    pub message_id: String,
    pub in_reply_to: String,
    pub references: Vec<String>,
    pub from_name: String,
    pub from_addr: String,
    pub subject: String,
    pub date: i64,
    pub list_unsubscribe: Option<String>,
    pub has_attach: bool,
}

fn clean_id(s: &str) -> String { s.trim().trim_start_matches('<').trim_end_matches('>').trim().to_string() }

fn ids_of(v: &HeaderValue) -> Vec<String> {
    let raw: Vec<&str> = match v {
        HeaderValue::Text(t) => t.split_whitespace().collect(),
        HeaderValue::TextList(l) => l.iter().flat_map(|t| t.split_whitespace()).collect(),
        _ => vec![],
    };
    raw.into_iter().map(clean_id).filter(|s| !s.is_empty()).collect()
}

fn addr_label(a: Option<&mail_parser::Address>) -> (String, String) {
    match a.and_then(|a| a.first()) {
        Some(x) => {
            let addr = x.address().unwrap_or("").to_string();
            let name = x.name().map(|s| s.to_string()).filter(|s| !s.is_empty()).unwrap_or_else(|| addr.clone());
            (name, addr)
        }
        None => (String::new(), String::new()),
    }
}

fn addr_list(a: Option<&mail_parser::Address>) -> String {
    a.map(|a| a.iter().filter_map(|x| x.address()).collect::<Vec<_>>().join(", ")).unwrap_or_default()
}

/// Indice de pièce jointe lu dans le Content-Type racine (BODYSTRUCTURE n'est pas exploitable
/// sans dépendre directement de imap-proto) : multipart/mixed ou partie unique non textuelle.
fn attach_hint(m: &Message) -> bool {
    let ct = m.root_part().headers().iter().find(|h| h.name().eq_ignore_ascii_case("content-type")).and_then(|h| h.value().as_content_type());
    match ct {
        Some(ct) => match ct.ctype() {
            "multipart" => ct.subtype() == Some("mixed"),
            "text" => false,
            _ => true,
        },
        None => false,
    }
}

/// Extrait les en-têtes d'un message brut (ou de ses seuls en-têtes).
pub fn parse_summary(raw: &[u8]) -> Result<Header, String> {
    let m = MessageParser::default().parse(raw).ok_or("message illisible")?;
    let (from_name, from_addr) = addr_label(m.from());
    Ok(Header {
        message_id: m.message_id().map(clean_id).unwrap_or_default(),
        in_reply_to: ids_of(m.in_reply_to()).into_iter().next().unwrap_or_default(),
        references: ids_of(m.references()),
        from_name, from_addr,
        subject: m.subject().unwrap_or("(sans objet)").to_string(),
        date: m.date().map(|d| d.to_timestamp()).unwrap_or(0),
        list_unsubscribe: m.header_raw("List-Unsubscribe").map(|s| s.trim().to_string()).filter(|s| !s.is_empty()),
        has_attach: attach_hint(&m),
    })
}

fn mime_of(p: &mail_parser::MessagePart) -> String {
    p.content_type().map(|c| format!("{}/{}", c.ctype(), c.subtype().unwrap_or("octet-stream"))).unwrap_or_else(|| "application/octet-stream".into())
}

fn attachments_of(m: &Message) -> Vec<Attachment> {
    m.attachments().enumerate().map(|(i, p)| {
        let filename = p.attachment_name().map(str::to_string).unwrap_or_else(|| format!("piece-jointe-{}", i + 1));
        let mime = mime_of(p);
        Attachment { index: i as u32, signable: signing::is_signable(&mime, &filename), filename, mime, size: p.len() as u64, superseded_by: None }
    }).collect()
}

/// Octets de la pièce jointe `index` (même ordre que `Full.attachments`).
pub fn attachment_bytes(raw: &[u8], index: u32) -> Result<(String, Vec<u8>), String> {
    let m = MessageParser::default().parse(raw).ok_or("message illisible")?;
    let p = m.attachments().nth(index as usize).ok_or("pièce jointe introuvable")?;
    Ok((p.attachment_name().unwrap_or("piece-jointe").to_string(), p.contents().to_vec()))
}

/// Message complet depuis le brut ; les champs de règle/fil sont remplis par `enrich`.
pub fn full_from_raw(raw: &[u8], folder: &str, uid: u32) -> Result<Full, String> {
    let m = MessageParser::default().parse(raw).ok_or("message illisible")?;
    let (from_name, from_addr) = addr_label(m.from());
    Ok(Full {
        uid, folder: folder.to_string(), from: from_name.clone(), from_name, from_addr,
        to: addr_list(m.to()), cc: addr_list(m.cc()),
        subject: m.subject().unwrap_or("(sans objet)").to_string(),
        date: m.date().map(|d| d.to_timestamp()).unwrap_or(0),
        message_id: m.message_id().map(clean_id).unwrap_or_default(),
        thread_id: None,
        text: m.body_text(0).map(|t| t.to_string()).unwrap_or_default(),
        html: m.body_html(0).map(|t| t.to_string()),
        attachments: attachments_of(&m),
        rule_id: None, rule_name: None, sorted_into: None,
    })
}

pub fn thread_message_from_raw(raw: &[u8], folder: &str, uid: u32) -> Result<ThreadMessage, String> {
    let f = full_from_raw(raw, folder, uid)?;
    Ok(ThreadMessage {
        uid, folder: f.folder, from_name: f.from_name, from_addr: f.from_addr, to: f.to, subject: f.subject,
        date: f.date, message_id: f.message_id, text: f.text, html: f.html, attachments: f.attachments, superseded: false,
    })
}

/// Marque les pièces jointes remplacées par une version plus récente du fil (messages en ordre chronologique).
pub fn mark_superseded(msgs: &mut [ThreadMessage]) {
    for i in 0..msgs.len() {
        let later: Vec<String> = msgs[i + 1..].iter().flat_map(|m| m.attachments.iter().map(|a| a.filename.clone())).collect();
        for a in msgs[i].attachments.iter_mut() {
            if let Some(newer) = later.iter().rev().find(|n| threading::supersedes(&a.filename, n)) {
                a.superseded_by = Some(newer.clone());
                a.signable = false;
            }
        }
        msgs[i].superseded = msgs[i].attachments.iter().any(|a| a.superseded_by.is_some());
    }
}

// ---------- Cache des en-têtes ----------

/// Ligne prête à être écrite dans `message`.
pub struct Cached { pub uid: u32, pub seen: bool, pub header: Header }

/// Écrit la fenêtre synchronisée d'un dossier : met à jour, supprime les UID disparus, renvoie les nouveaux UID.
/// Conserve `thread_id` et `rule_id` des lignes existantes.
pub fn store_headers(conn: &Connection, account: &str, folder: &str, rows: &[Cached]) -> Result<Vec<u32>, String> {
    let known: HashSet<u32> = {
        let mut st = conn.prepare("SELECT uid FROM message WHERE account=?1 AND folder=?2").map_err(e)?;
        let it = st.query_map(params![account, folder], |r| r.get::<_, u32>(0)).map_err(e)?;
        it.collect::<Result<_, _>>().map_err(e)?
    };
    let min = rows.iter().map(|r| r.uid).min().unwrap_or(0);
    let present: HashSet<u32> = rows.iter().map(|r| r.uid).collect();
    let mut up = conn.prepare(
        "INSERT INTO message(account,folder,uid,message_id,in_reply_to,refs,from_name,from_addr,subject,date,seen,has_attach,list_unsubscribe)
         VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)
         ON CONFLICT(account,folder,uid) DO UPDATE SET message_id=excluded.message_id, in_reply_to=excluded.in_reply_to, refs=excluded.refs,
           from_name=excluded.from_name, from_addr=excluded.from_addr, subject=excluded.subject, date=excluded.date, seen=excluded.seen,
           has_attach=excluded.has_attach, list_unsubscribe=excluded.list_unsubscribe").map_err(e)?;
    let mut new = Vec::new();
    for r in rows {
        let h = &r.header;
        up.execute(params![account, folder, r.uid, h.message_id, h.in_reply_to, h.references.join(" "), h.from_name, h.from_addr,
            h.subject, h.date, r.seen, h.has_attach, h.list_unsubscribe]).map_err(e)?;
        if !known.contains(&r.uid) { new.push(r.uid); }
    }
    for uid in known.iter().filter(|u| **u >= min && !present.contains(u)) { forget(conn, account, folder, &[*uid])?; }
    Ok(new)
}

fn forget(conn: &Connection, account: &str, folder: &str, uids: &[u32]) -> Result<(), String> {
    for uid in uids {
        conn.execute("DELETE FROM message WHERE account=?1 AND folder=?2 AND uid=?3", params![account, folder, uid]).map_err(e)?;
    }
    Ok(())
}

/// Recalcule les fils sur tout le cache du compte (les identifiants synthétiques dépendent de la liste complète).
pub fn assign_cached_threads(conn: &Connection, account: &str) -> Result<(), String> {
    let mut st = conn.prepare("SELECT rowid, message_id, in_reply_to, refs, subject, date, from_addr, thread_id FROM message WHERE account=?1 ORDER BY date, rowid").map_err(e)?;
    let rows: Vec<(i64, Option<String>, threading::Msg)> = st.query_map(params![account], |r| {
        let s = |i: usize| r.get::<_, Option<String>>(i).map(Option::unwrap_or_default);
        let from = s(6)?;
        Ok((r.get(0)?, r.get::<_, Option<String>>(7)?, threading::Msg {
            message_id: s(1)?, in_reply_to: s(2)?, references: s(3)?.split_whitespace().map(str::to_string).collect(),
            subject: s(4)?, date: r.get::<_, Option<i64>>(5)?.unwrap_or(0), participants: vec![from], thread_id: String::new(),
        }))
    }).map_err(e)?.collect::<Result<_, _>>().map_err(e)?;
    let (ids, olds): (Vec<_>, Vec<_>) = rows.iter().map(|(id, old, _)| (*id, old.clone())).unzip();
    let mut msgs: Vec<threading::Msg> = rows.into_iter().map(|(_, _, m)| m).collect();
    threading::assign_threads(&mut msgs);
    for ((id, old), m) in ids.into_iter().zip(olds).zip(&msgs) {
        if old.as_deref() != Some(m.thread_id.as_str()) {
            conn.execute("UPDATE message SET thread_id=?1 WHERE rowid=?2", params![m.thread_id, id]).map_err(e)?;
        }
    }
    Ok(())
}

fn like_escape(s: &str) -> String { s.replace('\\', "\\\\").replace('%', "\\%").replace('_', "\\_") }

const ROW_SELECT: &str = "SELECT m.uid, m.folder, m.from_name, m.from_addr, m.subject, m.date, m.seen, m.has_attach, m.thread_id, m.rule_id, r.name
    FROM message m LEFT JOIN rule r ON r.id = m.rule_id";

fn row_of(r: &rusqlite::Row) -> rusqlite::Result<MessageRow> {
    let s = |i: usize| r.get::<_, Option<String>>(i).map(Option::unwrap_or_default);
    let folder: String = r.get(1)?;
    let rule_id: Option<i64> = r.get(9)?;
    Ok(MessageRow {
        uid: r.get(0)?, from_name: s(2)?, from_addr: s(3)?, subject: s(4)?, date: r.get::<_, Option<i64>>(5)?.unwrap_or(0),
        seen: r.get::<_, Option<bool>>(6)?.unwrap_or(false), has_attach: r.get::<_, Option<bool>>(7)?.unwrap_or(false),
        thread_id: r.get(8)?, rule_id, rule_name: r.get(10)?,
        sorted_into: rule_id.map(|_| folder.clone()), folder,
    })
}

/// Messages en cache d'un dossier, du plus récent au plus ancien.
pub fn cached_messages(conn: &Connection, account: &str, folder: &str, query: Option<&str>, f: &Filters) -> Result<Vec<MessageRow>, String> {
    use rusqlite::types::Value;
    let mut sql = format!("{ROW_SELECT} WHERE m.account=?1 AND m.folder=?2");
    let mut args: Vec<Value> = vec![account.to_string().into(), folder.to_string().into()];
    // Chaque clause n'a qu'un paramètre, référencé par son numéro (`?` -> `?N`).
    fn add(sql: &mut String, args: &mut Vec<Value>, clause: &str, v: Value) {
        args.push(v);
        sql.push_str(&clause.replace('?', &format!("?{}", args.len())));
    }
    if let Some(q) = query.map(str::trim).filter(|q| !q.is_empty()) {
        add(&mut sql, &mut args, " AND (m.subject LIKE ? ESCAPE '\\' OR m.from_name LIKE ? ESCAPE '\\' OR m.from_addr LIKE ? ESCAPE '\\')", format!("%{}%", like_escape(q)).into());
    }
    if f.unread == Some(true) { sql.push_str(" AND m.seen=0"); }
    if f.has_attach == Some(true) { sql.push_str(" AND m.has_attach=1"); }
    if let Some(from) = f.from.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        add(&mut sql, &mut args, " AND (m.from_addr LIKE ? ESCAPE '\\' OR m.from_name LIKE ? ESCAPE '\\')", format!("%{}%", like_escape(from)).into());
    }
    if let Some(t) = f.since { add(&mut sql, &mut args, " AND m.date >= ?", t.into()); }
    if let Some(t) = f.until { add(&mut sql, &mut args, " AND m.date <= ?", t.into()); }
    sql.push_str(" ORDER BY m.date DESC, m.uid DESC LIMIT 1000");
    let mut st = conn.prepare(&sql).map_err(e)?;
    let out = st.query_map(rusqlite::params_from_iter(args), row_of).map_err(e)?.collect::<Result<_, _>>().map_err(e);
    out
}

fn cached_row(conn: &Connection, account: &str, folder: &str, uid: u32) -> Option<MessageRow> {
    let sql = format!("{ROW_SELECT} WHERE m.account=?1 AND m.folder=?2 AND m.uid=?3");
    conn.query_row(&sql, params![account, folder, uid], row_of).ok()
}

fn thread_refs(conn: &Connection, account: &str, thread_id: &str) -> Result<Vec<(String, u32)>, String> {
    let mut st = conn.prepare("SELECT folder, uid FROM message WHERE account=?1 AND thread_id=?2 ORDER BY date, uid").map_err(e)?;
    let out = st.query_map(params![account, thread_id], |r| Ok((r.get(0)?, r.get(1)?))).map_err(e)?.collect::<Result<_, _>>().map_err(e);
    out
}

fn set_seen_cache(conn: &Connection, account: &str, folder: &str, uid: u32, seen: bool) -> Result<(), String> {
    conn.execute("UPDATE message SET seen=?4 WHERE account=?1 AND folder=?2 AND uid=?3", params![account, folder, uid, seen]).map(|_| ()).map_err(e)
}

fn unread_cached(conn: &Connection, account: &str, folder: &str) -> u32 {
    conn.query_row("SELECT COUNT(*) FROM message WHERE account=?1 AND folder=?2 AND seen=0", params![account, folder], |r| r.get(0)).unwrap_or(0)
}

// ---------- Dossiers ----------

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role { Inbox, Drafts, Sent, Archive, Trash, Junk }

/// Rôle d'un dossier IMAP : attribut SPECIAL-USE en priorité, sinon nom usuel.
pub fn role_of(name: &str, attrs: &[String]) -> Option<Role> {
    let by_attr = attrs.iter().find_map(|a| match a.to_lowercase().as_str() {
        "\\drafts" => Some(Role::Drafts), "\\sent" => Some(Role::Sent), "\\archive" => Some(Role::Archive),
        "\\trash" => Some(Role::Trash), "\\junk" => Some(Role::Junk), _ => None,
    });
    by_attr.or_else(|| match name.to_lowercase().as_str() {
        "inbox" => Some(Role::Inbox),
        "drafts" | "draft" | "brouillons" => Some(Role::Drafts),
        "sent" | "sent items" | "sent messages" | "envoyés" | "éléments envoyés" => Some(Role::Sent),
        "archive" | "archives" => Some(Role::Archive),
        "trash" | "deleted items" | "deleted messages" | "corbeille" => Some(Role::Trash),
        "junk" | "spam" | "junk e-mail" | "indésirables" => Some(Role::Junk),
        _ => None,
    })
}

fn label_of(role: Role) -> &'static str {
    match role {
        Role::Inbox => "Réception", Role::Drafts => "Brouillons", Role::Sent => "Envoyés",
        Role::Archive => "Archives", Role::Trash => "Corbeille", Role::Junk => "Indésirables",
    }
}

/// Ordre : Réception, Planifiés (virtuel), Brouillons, Envoyés, Archives, Corbeille, Indésirables, puis dossiers de tri.
pub fn build_folders(entries: &[(String, Option<Role>)], unread: &HashMap<String, u32>) -> Vec<Folder> {
    let rank = |r: Option<Role>| match r {
        Some(Role::Inbox) => 0, Some(Role::Drafts) => 2, Some(Role::Sent) => 3, Some(Role::Archive) => 4,
        Some(Role::Trash) => 5, Some(Role::Junk) => 6, None => 7,
    };
    let mut v: Vec<Folder> = entries.iter().map(|(name, role)| Folder {
        name: name.clone(), unread: unread.get(name).copied().unwrap_or(0),
        label: role.map(label_of).map(str::to_string).unwrap_or_else(|| name.clone()),
        kind: if role.is_some() { FolderKind::System } else { FolderKind::Sorted },
    }).collect();
    let rank_of = |f: &Folder| rank(entries.iter().find(|(n, _)| *n == f.name).and_then(|(_, r)| *r));
    v.push(Folder { name: SCHEDULED_FOLDER.into(), label: SCHEDULED_FOLDER.into(), kind: FolderKind::System, unread: 0 });
    v.sort_by_key(|f| if f.name == SCHEDULED_FOLDER { (1, String::new()) } else { (rank_of(f), f.label.to_lowercase()) });
    v
}

/// Extrait le nombre de la réponse `* STATUS "x" (UNSEEN 3)`.
pub fn parse_status_unseen(resp: &[u8]) -> Option<u32> {
    let s = String::from_utf8_lossy(resp);
    let rest = &s[s.find("UNSEEN")? + "UNSEEN".len()..];
    rest.trim_start().split(|c: char| !c.is_ascii_digit()).next()?.parse().ok()
}

fn quote(s: &str) -> String { format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")) }

fn list_entries(s: &mut Session) -> Result<Vec<(String, Option<Role>)>, String> {
    let names = s.list(None, Some("*")).map_err(e)?;
    Ok(names.iter().filter_map(|n| {
        let noselect = n.attributes().iter().any(|a| matches!(a, imap::types::NameAttribute::NoSelect));
        let attrs: Vec<String> = n.attributes().iter().filter_map(|a| match a {
            imap::types::NameAttribute::Custom(c) => Some(c.to_string()), _ => None,
        }).collect();
        (!noselect).then(|| (n.name().to_string(), role_of(n.name(), &attrs)))
    }).collect())
}

fn role_folder(s: &mut Session, role: Role) -> Option<String> {
    list_entries(s).ok()?.into_iter().find(|(_, r)| *r == Some(role)).map(|(n, _)| n)
}

pub fn folders() -> Result<Vec<Folder>, String> {
    let acc = account()?;
    let (entries, unread) = with(|s| {
        let entries = list_entries(s)?;
        let unread: HashMap<String, u32> = entries.iter().filter_map(|(n, _)| {
            let raw = s.run_command_and_read_response(format!("STATUS {} (UNSEEN)", quote(n))).ok()?;
            Some((n.clone(), parse_status_unseen(&raw)?))
        }).collect();
        Ok((entries, unread))
    })?;
    // Repli sur le cache pour un dossier dont STATUS a échoué.
    let unread = with_db(|c| Ok(entries.iter().map(|(n, _)| (n.clone(), unread.get(n).copied().unwrap_or_else(|| unread_cached(c, &acc, n)))).collect()))?;
    Ok(build_folders(&entries, &unread))
}

// ---------- Synchronisation et lecture ----------

type AfterSync = fn(folder: &str, new_uids: &[u32]);
static AFTER_SYNC: OnceLock<AfterSync> = OnceLock::new();

/// Enregistre le rappel appelé après chaque synchronisation (règles de tri, contacts : b2).
#[allow(dead_code)] // branché par b2 (règles après synchronisation)
pub fn set_after_sync(f: AfterSync) { let _ = AFTER_SYNC.set(f); }

const WINDOW: u32 = 200;

fn fetch_window(s: &mut Session, folder: &str) -> Result<Vec<Cached>, String> {
    let mb = s.select(folder).map_err(e)?;
    if mb.exists == 0 { return Ok(vec![]); }
    let start = mb.exists.saturating_sub(WINDOW - 1).max(1);
    let fetched = s.fetch(format!("{start}:*"), "(UID FLAGS BODY.PEEK[HEADER])").map_err(e)?;
    Ok(fetched.iter().filter_map(|f| {
        let header = parse_summary(f.header()?).ok()?;
        let seen = f.flags().iter().any(|fl| matches!(fl, imap::types::Flag::Seen));
        Some(Cached { uid: f.uid?, seen, header })
    }).collect())
}

/// Rafraîchit le cache d'un dossier depuis IMAP ; renvoie les UID nouveaux (puis appelle le rappel b2).
pub fn sync_folder(folder: &str) -> Result<Vec<u32>, String> {
    let acc = account()?;
    let rows = with(|s| fetch_window(s, folder))?;
    let new = with_db(|c| {
        let new = store_headers(c, &acc, folder, &rows)?;
        assign_cached_threads(c, &acc)?;
        Ok(new)
    })?;
    if let Some(f) = AFTER_SYNC.get() { f(folder, &new); }
    Ok(new)
}

fn scheduled_rows(conn: &Connection, acc: &str, query: Option<&str>) -> Result<Vec<MessageRow>, String> {
    let q = query.map(|q| q.trim().to_lowercase()).filter(|q| !q.is_empty());
    Ok(scheduled_pending(conn)?.into_iter()
        .filter(|i| q.as_ref().map_or(true, |q| i.subject.to_lowercase().contains(q) || i.to.to_lowercase().contains(q)))
        .map(|i| MessageRow {
            uid: i.id as u32, folder: SCHEDULED_FOLDER.into(), from_name: "Vous".into(), from_addr: acc.to_string(),
            subject: i.subject, date: i.send_at, seen: true, has_attach: !i.draft.attachments.is_empty(),
            thread_id: None, rule_id: None, rule_name: None, sorted_into: None,
        }).collect())
}

pub fn messages(folder: &str, query: Option<&str>, filters: Option<&Filters>) -> Result<Vec<MessageRow>, String> {
    let acc = account()?;
    with_db(|c| if folder == SCHEDULED_FOLDER { scheduled_rows(c, &acc, query) }
        else { cached_messages(c, &acc, folder, query, &filters.cloned().unwrap_or_default()) })
}

/// Récupère les messages bruts ; `None` si un message n'existe plus sur le serveur.
fn fetch_raws(refs: &[(String, u32)]) -> Result<Vec<Option<Vec<u8>>>, String> {
    with(|s| {
        let mut current = String::new();
        refs.iter().map(|(folder, uid)| {
            if *folder != current { s.select(folder).map_err(e)?; current = folder.clone(); }
            let f = s.uid_fetch(uid.to_string(), "BODY.PEEK[]").map_err(e)?;
            Ok(f.iter().next().and_then(|f| f.body()).map(<[u8]>::to_vec))
        }).collect()
    })
}

fn fetch_raw(folder: &str, uid: u32) -> Result<Vec<u8>, String> {
    fetch_raws(&[(folder.to_string(), uid)])?.pop().flatten().ok_or_else(|| "message introuvable".to_string())
}

pub fn message(folder: &str, uid: u32) -> Result<Full, String> {
    let acc = account()?;
    let mut full = full_from_raw(&fetch_raw(folder, uid)?, folder, uid)?;
    mark_seen(folder, uid, true)?;
    with_db(|c| {
        if let Some(r) = cached_row(c, &acc, folder, uid) {
            full.thread_id = r.thread_id; full.rule_id = r.rule_id; full.rule_name = r.rule_name; full.sorted_into = r.sorted_into;
        }
        Ok(())
    })?;
    Ok(full)
}

pub fn thread(thread_id: &str) -> Result<Vec<ThreadMessage>, String> {
    let acc = account()?;
    let refs = with_db(|c| thread_refs(c, &acc, thread_id))?;
    let raws = fetch_raws(&refs)?;
    let mut msgs: Vec<ThreadMessage> = refs.iter().zip(raws)
        .filter_map(|((folder, uid), raw)| thread_message_from_raw(&raw?, folder, *uid).ok()).collect();
    msgs.sort_by_key(|m| m.date);
    mark_superseded(&mut msgs);
    Ok(msgs)
}

pub fn save_attachment(folder: &str, uid: u32, index: u32, dest: &str) -> Result<(), String> {
    let (_, bytes) = attachment_bytes(&fetch_raw(folder, uid)?, index)?;
    std::fs::write(dest, bytes).map_err(|x| format!("écriture de {dest} impossible : {x}"))
}

// ---------- Actions sur les messages ----------

/// `1,2,3` : liste d'UID pour les commandes IMAP.
pub fn uid_set(uids: &[u32]) -> String { uids.iter().map(u32::to_string).collect::<Vec<_>>().join(",") }

pub fn mark_seen(folder: &str, uid: u32, seen: bool) -> Result<(), String> {
    let acc = account()?;
    with(|s| {
        s.select(folder).map_err(e)?;
        let op = if seen { "+FLAGS.SILENT (\\Seen)" } else { "-FLAGS.SILENT (\\Seen)" };
        s.uid_store(uid.to_string(), op).map(|_| ()).map_err(e)
    })?;
    with_db(|c| set_seen_cache(c, &acc, folder, uid, seen))
}

/// UID MOVE ; crée la cible si besoin ; repli COPY + \Deleted + EXPUNGE si MOVE n'est pas supporté.
fn move_uids(s: &mut Session, uids: &str, target: &str) -> Result<(), String> {
    if s.uid_mv(uids, target).is_ok() { return Ok(()); }
    let _ = s.create(target);
    if s.uid_mv(uids, target).is_ok() { return Ok(()); }
    s.uid_copy(uids, target).map_err(e)?;
    expunge_uids(s, uids)
}

fn expunge_uids(s: &mut Session, uids: &str) -> Result<(), String> {
    s.uid_store(uids, "+FLAGS.SILENT (\\Deleted)").map_err(e)?;
    if s.uid_expunge(uids).is_err() { s.expunge().map_err(e)?; }
    Ok(())
}

pub fn bulk_move(folder: &str, uids: &[u32], target: &str) -> Result<(), String> {
    if uids.is_empty() || folder == target { return Ok(()); }
    let acc = account()?;
    let set = uid_set(uids);
    with(|s| { s.select(folder).map_err(e)?; move_uids(s, &set, target) })?;
    with_db(|c| forget(c, &acc, folder, uids))
}

/// Déplace vers la corbeille ; supprime définitivement si le dossier est déjà la corbeille.
pub fn bulk_delete(folder: &str, uids: &[u32]) -> Result<(), String> {
    if uids.is_empty() { return Ok(()); }
    let acc = account()?;
    let set = uid_set(uids);
    with(|s| {
        let trash = role_folder(s, Role::Trash).unwrap_or_else(|| "Trash".into());
        s.select(folder).map_err(e)?;
        if folder == trash { expunge_uids(s, &set) } else { move_uids(s, &set, &trash) }
    })?;
    with_db(|c| forget(c, &acc, folder, uids))
}

// ---------- Envoi ----------

fn parse_addrs(list: &str) -> Result<Vec<Mailbox>, String> {
    list.split([',', ';']).map(str::trim).filter(|s| !s.is_empty())
        .map(|a| a.parse().map_err(|x| format!("adresse invalide « {a} » : {x}"))).collect()
}

/// Type MIME d'après l'extension (repli : application/octet-stream).
pub fn mime_for(filename: &str) -> &'static str {
    match filename.rsplit('.').next().map(str::to_lowercase).as_deref() {
        Some("pdf") => "application/pdf",
        Some("doc") => "application/msword",
        Some("docx") => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        Some("odt") => "application/vnd.oasis.opendocument.text",
        Some("txt") => "text/plain",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("zip") => "application/zip",
        _ => "application/octet-stream",
    }
}

fn attachment_part(path: &str) -> Result<SinglePart, String> {
    let p = Path::new(path);
    let name = p.file_name().and_then(|n| n.to_str()).ok_or_else(|| format!("chemin invalide : {path}"))?;
    let bytes = std::fs::read(p).map_err(|x| format!("pièce jointe illisible ({path}) : {x}"))?;
    let ct = ContentType::parse(mime_for(name)).map_err(e)?;
    Ok(lettre::message::Attachment::new(name.to_string()).body(bytes, ct))
}

/// Construit le message : text/plain seul, ou multipart/mixed avec pièces jointes.
pub fn build_message(from: &str, draft: &Draft, message_id: &str) -> Result<OutMessage, String> {
    let mut b = OutMessage::builder().from(from.parse().map_err(e)?).subject(draft.subject.clone())
        .message_id(Some(format!("<{message_id}>"))).date_now();
    for to in parse_addrs(&draft.to)? { b = b.to(to); }
    for cc in parse_addrs(draft.cc.as_deref().unwrap_or(""))? { b = b.cc(cc); }
    if let Some(id) = draft.in_reply_to.as_deref().map(clean_id).filter(|s| !s.is_empty()) {
        b = b.in_reply_to(format!("<{id}>")).references(format!("<{id}>"));
    }
    let text = SinglePart::plain(draft.body.clone());
    if draft.attachments.is_empty() { return b.singlepart(text).map_err(e); }
    let mut mp = MultiPart::mixed().singlepart(text);
    for path in &draft.attachments { mp = mp.singlepart(attachment_part(path)?); }
    b.multipart(mp).map_err(e)
}

fn new_message_id(domain: &str) -> String {
    let n = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    format!("{:x}.{:x}@{}", n, std::process::id(), domain)
}

/// Relance : à l'échéance, le planificateur vérifie qu'aucune réponse n'a cité `message_id`.
pub fn add_reminder(conn: &Connection, message_id: &str, due_at: i64) -> Result<i64, String> {
    conn.execute("INSERT INTO reminder(message_id, due_at) VALUES(?1, ?2)", params![message_id, due_at]).map_err(e)?;
    Ok(conn.last_insert_rowid())
}

/// Envoie immédiatement (SMTP), copie dans « Envoyés », crée la relance demandée. Renvoie le Message-ID.
/// Point d'entrée de l'envoi pour le planificateur (b2).
pub fn send_now(draft: &Draft) -> Result<String, String> {
    let c = config::load()?;
    let id = new_message_id(c.user.rsplit('@').next().unwrap_or("localhost"));
    let msg = build_message(&c.user, draft, &id)?;
    smtp_transport(&c)?.send(&msg).map_err(e)?;
    // Fastmail n'archive pas les envois SMTP : copie dans « Envoyés » en tâche de fond.
    let raw = msg.formatted();
    std::thread::spawn(move || {
        let _ = with(|s| {
            let sent = role_folder(s, Role::Sent).unwrap_or_else(|| "Sent".into());
            s.append_with_flags(sent, &raw, &[imap::types::Flag::Seen]).map_err(e)
        });
    });
    if let Some(days) = draft.remind_days.filter(|d| *d > 0) {
        with_db(|c| add_reminder(c, &id, now() + i64::from(days) * 86_400).map(|_| ()))?;
    }
    Ok(id)
}

pub fn send(draft: &Draft) -> Result<SendResult, String> {
    match draft.send_at.filter(|t| *t > now()) {
        Some(t) => Ok(SendResult { status: SendStatus::Scheduled, id: Some(with_db(|c| schedule(c, draft, t))?) }),
        None => send_now(draft).map(|_| SendResult { status: SendStatus::Sent, id: None }),
    }
}

// ---------- Envois planifiés ----------

pub fn schedule(conn: &Connection, draft: &Draft, send_at: i64) -> Result<i64, String> {
    let payload = serde_json::to_string(draft).map_err(e)?;
    conn.execute("INSERT INTO scheduled_send(send_at, payload_json) VALUES(?1, ?2)", params![send_at, payload]).map_err(e)?;
    Ok(conn.last_insert_rowid())
}

/// Envois planifiés en attente dont l'heure est <= `until`.
fn scheduled_until(conn: &Connection, until: i64) -> Result<Vec<ScheduledItem>, String> {
    let mut st = conn.prepare("SELECT id, send_at, payload_json FROM scheduled_send WHERE status='pending' AND send_at <= ?1 ORDER BY send_at, id").map_err(e)?;
    let rows: Vec<(i64, i64, String)> = st.query_map(params![until], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
        .map_err(e)?.collect::<Result<_, _>>().map_err(e)?;
    rows.into_iter().map(|(id, send_at, payload)| {
        let draft: Draft = serde_json::from_str(&payload).map_err(e)?;
        Ok(ScheduledItem { id, send_at, to: draft.to.clone(), subject: draft.subject.clone(), draft })
    }).collect()
}

pub fn scheduled_pending(conn: &Connection) -> Result<Vec<ScheduledItem>, String> { scheduled_until(conn, i64::MAX) }

/// Envois planifiés échus (`send_at <= now`), à traiter par le planificateur (b2).
#[allow(dead_code)] // utilisé par le planificateur (b2)
pub fn scheduled_due(conn: &Connection, now: i64) -> Result<Vec<ScheduledItem>, String> { scheduled_until(conn, now) }

/// `status` : `sent` ou `failed` (b2) ; `cancelled` via `scheduled_cancel`.
#[allow(dead_code)] // utilisé par le planificateur (b2)
pub fn mark_scheduled(conn: &Connection, id: i64, status: &str) -> Result<(), String> {
    conn.execute("UPDATE scheduled_send SET status=?2 WHERE id=?1", params![id, status]).map(|_| ()).map_err(e)
}

pub fn scheduled_cancel(conn: &Connection, id: i64) -> Result<(), String> {
    let n = conn.execute("UPDATE scheduled_send SET status='cancelled' WHERE id=?1 AND status='pending'", params![id]).map_err(e)?;
    if n == 0 { return Err("envoi planifié introuvable ou déjà parti".into()); }
    Ok(())
}

fn smtp_transport(c: &Config) -> Result<SmtpTransport, String> {
    require_plaintext_ok(&c.smtp_host, c.smtp_security)?;
    let b = match c.smtp_security {
        Security::Ssl => SmtpTransport::relay(&c.smtp_host).map_err(e)?,
        Security::Starttls => SmtpTransport::starttls_relay(&c.smtp_host).map_err(e)?,
        Security::None => SmtpTransport::builder_dangerous(&c.smtp_host),
    };
    Ok(b.port(c.smtp_port).credentials(Credentials::new(c.user.clone(), c.pass.clone())).build())
}

/// Vérifie IMAP puis SMTP avec ces réglages, sans rien enregistrer.
pub fn test_login(c: &Config) -> Result<(), String> {
    connect(c).map_err(|x| format!("IMAP : {x}"))?.logout().ok();
    match smtp_transport(c)?.test_connection() {
        Ok(true) => Ok(()),
        Ok(false) => Err("SMTP : connexion refusée".into()),
        Err(x) => Err(format!("SMTP : {x}")),
    }
}


#[cfg(test)]
mod mail_core_tests {
    use super::*;

    fn fixture(name: &str) -> Vec<u8> {
        std::fs::read(format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))).unwrap()
    }
    fn fixture_path(name: &str) -> String { format!("{}/tests/fixtures/{name}", env!("CARGO_MANIFEST_DIR")) }
    fn draft(to: &str) -> Draft {
        Draft { to: to.into(), cc: None, subject: "Sujet".into(), body: "Corps".into(), in_reply_to: None, attachments: vec![], remind_days: None, send_at: None }
    }
    fn cached(uid: u32, name: &str, seen: bool) -> Cached { Cached { uid, seen, header: parse_summary(&fixture(name)).unwrap() } }

    #[test]
    fn parse_summary_simple() {
        let h = parse_summary(&fixture("simple.eml")).unwrap();
        assert_eq!((h.from_name.as_str(), h.from_addr.as_str()), ("Amélie Dupont", "amelie@example.org"));
        assert_eq!(h.subject, "Rendez-vous demain");
        assert_eq!(h.message_id, "simple-1@example.org");
        assert_eq!(h.date, 1_793_109_900);
        assert!(h.references.is_empty() && h.in_reply_to.is_empty() && h.list_unsubscribe.is_none() && !h.has_attach);
    }

    #[test]
    fn parse_summary_list_unsubscribe() {
        let h = parse_summary(&fixture("newsletter.eml")).unwrap();
        assert_eq!(h.list_unsubscribe.as_deref(), Some("<https://journal.example/u/abc>, <mailto:unsub@journal.example>"));
        assert!(!h.has_attach);
    }

    #[test]
    fn parse_summary_references() {
        let h = parse_summary(&fixture("reply.eml")).unwrap();
        assert_eq!(h.in_reply_to, "simple-1@example.org");
        assert_eq!(h.references, vec!["root-0@example.org", "simple-1@example.org"]);
    }

    #[test]
    fn parse_summary_accepts_header_only_input_and_flags_attachments() {
        let raw = fixture("attachment.eml");
        let head_end = raw.windows(4).position(|w| w == b"\n\n--").unwrap();
        assert!(parse_summary(&raw[..head_end + 2]).unwrap().has_attach);
    }

    #[test]
    fn attachments_are_listed_and_extracted() {
        let raw = fixture("attachment.eml");
        let full = full_from_raw(&raw, "INBOX", 7).unwrap();
        assert_eq!(full.text.trim(), "Voici le contrat.");
        assert_eq!(full.cc, "compta@example.org");
        let a = &full.attachments[0];
        assert_eq!((a.index, a.filename.as_str(), a.mime.as_str(), a.signable), (0, "contrat_v2.pdf", "application/pdf", true));
        let pdf = fixture("contrat_v2.pdf");
        assert_eq!(a.size, pdf.len() as u64);
        let (name, bytes) = attachment_bytes(&raw, 0).unwrap();
        assert_eq!((name.as_str(), bytes), ("contrat_v2.pdf", pdf));
        assert!(attachment_bytes(&raw, 1).is_err());
    }

    #[test]
    fn full_message_has_html_and_no_attachment() {
        let f = full_from_raw(&fixture("newsletter.eml"), "INBOX", 1).unwrap();
        assert!(f.html.unwrap().contains("<b>HTML</b>"));
        assert!(f.attachments.is_empty());
    }

    #[test]
    fn newer_version_supersedes_older_attachment() {
        let mut msgs = vec![
            thread_message_from_raw(&fixture("attachment.eml"), "INBOX", 1).unwrap(),
            thread_message_from_raw(&fixture("attachment_v3.eml"), "INBOX", 2).unwrap(),
        ];
        mark_superseded(&mut msgs);
        let old = &msgs[0].attachments[0];
        assert_eq!(old.superseded_by.as_deref(), Some("contrat_v3.pdf"));
        assert!(!old.signable && msgs[0].superseded);
        assert!(msgs[1].attachments[0].signable && !msgs[1].superseded);
    }

    #[test]
    fn plain_message_is_single_part() {
        let m = build_message("moi@example.com", &draft("a@example.org"), "id1@example.com").unwrap();
        let s = String::from_utf8(m.formatted()).unwrap();
        assert!(!s.contains("multipart/mixed"));
        assert!(s.contains("Message-ID: <id1@example.com>") && s.contains("text/plain"));
    }

    #[test]
    fn message_with_attachment_is_multipart_mixed_and_round_trips() {
        let mut d = Draft { cc: Some("c@example.org; d@example.org".into()), in_reply_to: Some("<parent@example.org>".into()), ..draft("a@example.org, b@example.org") };
        d.attachments = vec![fixture_path("contrat_v2.pdf")];
        let raw = build_message("moi@example.com", &d, "id2@example.com").unwrap().formatted();
        let s = String::from_utf8_lossy(&raw);
        assert!(s.contains("multipart/mixed") && s.contains("contrat_v2.pdf"));
        let h = parse_summary(&raw).unwrap();
        assert_eq!(h.in_reply_to, "parent@example.org");
        let (name, bytes) = attachment_bytes(&raw, 0).unwrap();
        assert_eq!((name.as_str(), bytes), ("contrat_v2.pdf", fixture("contrat_v2.pdf")));
        assert_eq!(full_from_raw(&raw, "Sent", 1).unwrap().cc, "c@example.org, d@example.org");
    }

    #[test]
    fn bad_address_or_missing_attachment_is_an_error() {
        assert!(build_message("moi@example.com", &draft("pas une adresse"), "x@y").is_err());
        let d = Draft { attachments: vec!["/inexistant/fichier.pdf".into()], ..draft("a@example.org") };
        assert!(build_message("moi@example.com", &d, "x@y").is_err());
    }

    #[test]
    fn mime_for_known_extensions() {
        assert_eq!(mime_for("A.PDF"), "application/pdf");
        assert_eq!(mime_for("x.bin"), "application/octet-stream");
    }

    #[test]
    fn cache_stores_threads_and_filters() {
        let c = db::open_in_memory().unwrap();
        let rows = [cached(1, "simple.eml", true), cached(2, "reply.eml", false), cached(3, "newsletter.eml", false), cached(4, "attachment.eml", false)];
        assert_eq!(store_headers(&c, "me", "INBOX", &rows).unwrap(), vec![1, 2, 3, 4]);
        assign_cached_threads(&c, "me").unwrap();
        let all = cached_messages(&c, "me", "INBOX", None, &Filters::default()).unwrap();
        assert_eq!(all.iter().map(|m| m.uid).collect::<Vec<_>>(), vec![4, 2, 1, 3]);
        let t = |uid| all.iter().find(|m| m.uid == uid).unwrap().thread_id.clone();
        assert!(t(1).is_some() && t(1) == t(2) && t(1) != t(3));

        let ids = |q: Option<&str>, f: Filters| cached_messages(&c, "me", "INBOX", q, &f).unwrap().iter().map(|m| m.uid).collect::<Vec<_>>();
        assert_eq!(ids(Some("rendez"), Filters::default()), vec![2, 1]);
        assert_eq!(ids(Some("100%"), Filters::default()), Vec::<u32>::new());
        assert_eq!(ids(None, Filters { unread: Some(true), ..Default::default() }), vec![4, 2, 3]);
        assert_eq!(ids(None, Filters { has_attach: Some(true), ..Default::default() }), vec![4]);
        assert_eq!(ids(None, Filters { from: Some("journal".into()), ..Default::default() }), vec![3]);
        assert_eq!(ids(None, Filters { since: Some(1_793_109_900), until: Some(1_793_109_900), ..Default::default() }), vec![1]);
        assert!(cached_messages(&c, "autre", "INBOX", None, &Filters::default()).unwrap().is_empty());
    }

    #[test]
    fn resync_keeps_rule_and_thread_and_drops_vanished() {
        let c = db::open_in_memory().unwrap();
        store_headers(&c, "me", "INBOX", &[cached(1, "simple.eml", false), cached(2, "reply.eml", false), cached(3, "newsletter.eml", false)]).unwrap();
        assign_cached_threads(&c, "me").unwrap();
        c.execute("INSERT INTO rule(id, name, cond_json, action_json) VALUES(5, 'Journaux', '{}', '{}')", []).unwrap();
        c.execute("UPDATE message SET rule_id=5 WHERE uid=3", []).unwrap();
        // uid 2 a disparu du serveur, uid 1 est devenu lu, uid 9 est nouveau
        let new = store_headers(&c, "me", "INBOX", &[cached(1, "simple.eml", true), cached(3, "newsletter.eml", true), cached(9, "attachment.eml", false)]).unwrap();
        assert_eq!(new, vec![9]);
        let all = cached_messages(&c, "me", "INBOX", None, &Filters::default()).unwrap();
        assert_eq!(all.iter().map(|m| m.uid).collect::<Vec<_>>(), vec![9, 1, 3]);
        let n = all.iter().find(|m| m.uid == 3).unwrap();
        assert_eq!((n.rule_id, n.rule_name.as_deref(), n.sorted_into.as_deref(), n.seen), (Some(5), Some("Journaux"), Some("INBOX"), true));
        assert!(all.iter().find(|m| m.uid == 1).unwrap().thread_id.is_some());
        set_seen_cache(&c, "me", "INBOX", 9, true).unwrap();
        assert_eq!(unread_cached(&c, "me", "INBOX"), 0);
        assert_eq!(thread_refs(&c, "me", &all[1].thread_id.clone().unwrap()).unwrap(), vec![("INBOX".to_string(), 1)]);
    }

    #[test]
    fn folders_are_ordered_and_labelled_in_french() {
        let attrs = |a: &str| vec![a.to_string()];
        assert_eq!(role_of("Envoyes-perso", &attrs("\\Sent")), Some(Role::Sent));
        assert_eq!(role_of("Trash", &[]), Some(Role::Trash));
        assert_eq!(role_of("Factures", &[]), None);
        let entries: Vec<(String, Option<Role>)> = ["Factures", "Trash", "Archive", "Sent", "Drafts", "Clients", "INBOX"]
            .iter().map(|n| (n.to_string(), role_of(n, &[]))).collect();
        let unread = HashMap::from([("INBOX".to_string(), 3u32), ("Clients".to_string(), 1)]);
        let f = build_folders(&entries, &unread);
        let names: Vec<&str> = f.iter().map(|x| x.name.as_str()).collect();
        assert_eq!(names, ["INBOX", SCHEDULED_FOLDER, "Drafts", "Sent", "Archive", "Trash", "Clients", "Factures"]);
        assert_eq!(f[0].label, "Réception");
        assert_eq!(f[0].unread, 3);
        assert_eq!(f[3].label, "Envoyés");
        assert_eq!((f[6].kind, f[6].unread, f[0].kind), (FolderKind::Sorted, 1, FolderKind::System));
    }

    #[test]
    fn status_and_uid_set_helpers() {
        assert_eq!(parse_status_unseen(b"* STATUS \"INBOX\" (UNSEEN 12)\r\nA1 OK done\r\n"), Some(12));
        assert_eq!(parse_status_unseen(b"A1 NO nope\r\n"), None);
        assert_eq!(uid_set(&[4, 8, 15]), "4,8,15");
        assert_eq!(quote("a\"b"), "\"a\\\"b\"");
    }

    #[test]
    fn scheduled_sends_are_stored_listed_cancelled_and_due() {
        let c = db::open_in_memory().unwrap();
        let a = schedule(&c, &draft("a@example.org"), 2_000).unwrap();
        let b = schedule(&c, &Draft { subject: "Deux".into(), ..draft("b@example.org") }, 1_000).unwrap();
        let list = scheduled_pending(&c).unwrap();
        assert_eq!(list.iter().map(|i| i.id).collect::<Vec<_>>(), vec![b, a]);
        assert_eq!((list[0].to.as_str(), list[0].subject.as_str(), list[0].send_at), ("b@example.org", "Deux", 1_000));
        assert_eq!(scheduled_due(&c, 1_500).unwrap().len(), 1);
        let rows = scheduled_rows(&c, "me@example.com", Some("deux")).unwrap();
        assert_eq!((rows.len(), rows[0].folder.as_str(), rows[0].date), (1, SCHEDULED_FOLDER, 1_000));
        scheduled_cancel(&c, b).unwrap();
        assert!(scheduled_cancel(&c, b).is_err());
        mark_scheduled(&c, a, "sent").unwrap();
        assert!(scheduled_pending(&c).unwrap().is_empty());
    }

    #[test]
    fn reminder_row_is_created() {
        let c = db::open_in_memory().unwrap();
        add_reminder(&c, "id@example.com", 5_000).unwrap();
        let (m, due, status): (String, i64, String) = c.query_row("SELECT message_id, due_at, status FROM reminder", [], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?))).unwrap();
        assert_eq!((m.as_str(), due, status.as_str()), ("id@example.com", 5_000, "pending"));
    }
}

#[cfg(test)]
mod login_tests {
    use super::*;
    use crate::config;
    #[test]
    #[ignore = "test en direct : nécessite ../.env et un serveur réel"]
    fn login_roundtrip() {
        let txt = std::fs::read_to_string("../.env").unwrap();
        let g = |k: &str| txt.lines().find_map(|l| l.strip_prefix(&format!("{k}="))).unwrap().to_string();
        let mut c = Config { user: g("MAIL_USER"), pass: g("MAIL_PASS"), imap_host: "imap.fastmail.com".into(), imap_port: 993, smtp_host: "smtp.fastmail.com".into(), smtp_port: 465, imap_security: Security::Ssl, smtp_security: Security::Ssl };
        test_login(&c).unwrap(); println!("login OK (465)");
        c.smtp_port = 587; c.smtp_security = Security::Starttls; test_login(&c).unwrap(); println!("login OK (587 starttls)");
        let bad = Config { pass: "mauvais".into(), ..c.clone() };
        println!("mauvais mdp -> {:?}", test_login(&bad).err());
        let s = config::Settings { email: c.user.clone(), imap_host: c.imap_host.clone(), imap_port: 993, smtp_host: c.smtp_host.clone(), smtp_port: 465, imap_security: None, smtp_security: None };
        match config::save(&s, &c.pass) { Ok(()) => { println!("save OK, load={}", config::load().is_ok()); config::clear(); println!("cleared, settings={}", config::read_settings().is_none()); }
            Err(e) => println!("save KO: {e}") }
    }
}

#[cfg(test)]
mod security_tests {
    use super::*;

    #[test]
    fn plaintext_only_for_local_hosts() {
        for h in ["localhost", "127.0.0.1", "10.0.0.5", "192.168.1.2", "172.16.0.1", "::1", "fd00::1"] {
            assert!(plaintext_allowed(h), "{h}");
        }
        for h in ["imap.fastmail.com", "8.8.8.8", "2001:db8::1"] { assert!(!plaintext_allowed(h), "{h}"); }
    }

    #[test]
    fn none_security_refused_for_public_host() {
        assert!(require_plaintext_ok("imap.fastmail.com", Security::None).is_err());
        assert!(require_plaintext_ok("imap.fastmail.com", Security::Ssl).is_ok());
    }
}
