use rusqlite::Connection;
use std::{fs, path::Path};

/// Migrations numérotées : l'index + 1 = `PRAGMA user_version` après application.
const MIGRATIONS: &[&str] = &[V1, V2];
pub const LATEST: i64 = MIGRATIONS.len() as i64;

const V1: &str = "
CREATE TABLE message(account TEXT, folder TEXT, uid INTEGER, message_id TEXT, in_reply_to TEXT, refs TEXT,
  from_name TEXT, from_addr TEXT, subject TEXT, date INTEGER, seen INTEGER, has_attach INTEGER,
  list_unsubscribe TEXT, thread_id TEXT, rule_id INTEGER, PRIMARY KEY(account,folder,uid));
CREATE INDEX msg_thread ON message(thread_id);
CREATE INDEX msg_from ON message(from_addr);
CREATE TABLE rule(id INTEGER PRIMARY KEY, name TEXT, enabled INTEGER DEFAULT 1, cond_json TEXT NOT NULL, action_json TEXT NOT NULL, position INTEGER);
CREATE TABLE chron(id INTEGER PRIMARY KEY, name TEXT, cron TEXT NOT NULL, kind TEXT NOT NULL, enabled INTEGER DEFAULT 1, last_run INTEGER, next_run INTEGER);
CREATE TABLE scheduled_send(id INTEGER PRIMARY KEY, send_at INTEGER NOT NULL, payload_json TEXT NOT NULL, status TEXT DEFAULT 'pending');
CREATE TABLE reminder(id INTEGER PRIMARY KEY, message_id TEXT, due_at INTEGER, status TEXT DEFAULT 'pending');
CREATE TABLE contact(id INTEGER PRIMARY KEY, name TEXT, email TEXT UNIQUE NOT NULL, notes TEXT, manual INTEGER DEFAULT 0);
CREATE TABLE sender_decision(addr TEXT PRIMARY KEY, decision TEXT NOT NULL);
CREATE TABLE document(id INTEGER PRIMARY KEY, name TEXT, path TEXT, sha256 TEXT, status TEXT, signers_json TEXT, level TEXT DEFAULT 'simple', origin_message_id TEXT, created INTEGER);
CREATE TABLE proof_event(id INTEGER PRIMARY KEY, document_id INTEGER, at INTEGER, kind TEXT, detail TEXT);
";

/// Tri des listes de messages (dossier + date) et lecture par expéditeur.
const V2: &str = "
CREATE INDEX IF NOT EXISTS msg_folder_date ON message(account, folder, date DESC, uid DESC);
CREATE INDEX IF NOT EXISTS msg_message_id ON message(message_id);
";

fn migrate(conn: &mut Connection) -> rusqlite::Result<()> {
    let current: i64 = conn.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    for (i, sql) in MIGRATIONS.iter().enumerate().skip(current as usize) {
        let tx = conn.transaction()?;
        tx.execute_batch(sql)?;
        tx.pragma_update(None, "user_version", i as i64 + 1)?;
        tx.commit()?;
    }
    Ok(())
}

/// Ouvre (et migre) la base `<config>/iris/iris.db`.
pub fn open(path: &Path) -> Result<Connection, String> {
    if let Some(dir) = path.parent() { fs::create_dir_all(dir).map_err(|e| e.to_string())?; }
    let mut conn = Connection::open(path).map_err(|e| e.to_string())?;
    // WAL + synchronous=NORMAL : une écriture ne force plus un fsync de tout le fichier ; les lectures ne bloquent plus l'écriture.
    let _ = conn.pragma_update(None, "journal_mode", "WAL");
    let _ = conn.pragma_update(None, "synchronous", "NORMAL");
    let _ = conn.busy_timeout(std::time::Duration::from_secs(5));
    migrate(&mut conn).map_err(|e| e.to_string())?;
    Ok(conn)
}

pub fn open_in_memory() -> Result<Connection, String> {
    let mut conn = Connection::open_in_memory().map_err(|e| e.to_string())?;
    migrate(&mut conn).map_err(|e| e.to_string())?;
    Ok(conn)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tables(c: &Connection) -> Vec<String> {
        let mut st = c.prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name").unwrap();
        st.query_map([], |r| r.get(0)).unwrap().map(Result::unwrap).collect()
    }

    #[test]
    fn open_in_memory_applies_all_migrations() {
        let c = open_in_memory().unwrap();
        let v: i64 = c.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(v, LATEST);
        for t in ["message", "rule", "chron", "scheduled_send", "reminder", "contact", "sender_decision", "document", "proof_event"] {
            assert!(tables(&c).contains(&t.to_string()), "table {t} manquante");
        }
    }

    #[test]
    fn migrate_is_idempotent() {
        let mut c = open_in_memory().unwrap();
        migrate(&mut c).unwrap();
        let v: i64 = c.query_row("PRAGMA user_version", [], |r| r.get(0)).unwrap();
        assert_eq!(v, LATEST);
    }
}
