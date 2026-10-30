use serde::{Deserialize, Serialize};
use std::{collections::HashMap, env, fs, path::PathBuf, sync::Mutex};

const SERVICE: &str = "dev.dksuite.iris";
const OLD_SERVICE: &str = "dev.angeloclauin.courrier";

/// Sécurité du transport. `None` n'est accepté que pour un hôte local ou privé (voir `mail`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Security { Ssl, Starttls, None }

impl Security {
    /// Valeur historique quand `*_security` manque : SMTP 465 = TLS implicite, sinon STARTTLS ; IMAP = TLS.
    fn legacy_smtp(port: u16) -> Security { if port == 465 { Security::Ssl } else { Security::Starttls } }
}

/// Réglages non secrets, stockés dans <config>/iris/settings.json. Le mot de passe va dans le trousseau système.
#[derive(Clone, Serialize, Deserialize)]
pub struct Settings {
    pub email: String,
    pub imap_host: String,
    pub imap_port: u16,
    pub smtp_host: String,
    pub smtp_port: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub imap_security: Option<Security>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub smtp_security: Option<Security>,
}

impl Settings {
    pub fn imap_security(&self) -> Security { self.imap_security.unwrap_or(Security::Ssl) }
    pub fn smtp_security(&self) -> Security { self.smtp_security.unwrap_or_else(|| Security::legacy_smtp(self.smtp_port)) }
}

#[derive(Clone)]
pub struct Config {
    pub user: String,
    pub pass: String,
    pub imap_host: String,
    pub imap_port: u16,
    pub smtp_host: String,
    pub smtp_port: u16,
    pub imap_security: Security,
    pub smtp_security: Security,
}

impl Config {
    pub fn from_settings(s: &Settings, pass: &str) -> Config {
        Config {
            user: s.email.clone(), pass: pass.to_string(), imap_host: s.imap_host.clone(), imap_port: s.imap_port,
            smtp_host: s.smtp_host.clone(), smtp_port: s.smtp_port,
            imap_security: s.imap_security(), smtp_security: s.smtp_security(),
        }
    }
}

static CACHE: Mutex<Option<Config>> = Mutex::new(None);

fn base_dir() -> PathBuf {
    env::var_os("XDG_CONFIG_HOME").map(PathBuf::from)
        .or_else(|| env::var_os("APPDATA").map(PathBuf::from))
        .or_else(|| env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        .unwrap_or_else(|| PathBuf::from("."))
}
fn dir() -> PathBuf { base_dir().join("iris") }
fn settings_path() -> PathBuf { dir().join("settings.json") }

fn entry(email: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(SERVICE, email).map_err(|e| e.to_string())
}

pub fn read_settings() -> Option<Settings> {
    serde_json::from_str(&fs::read_to_string(settings_path()).ok()?).ok()
}

pub fn save(s: &Settings, pass: &str) -> Result<(), String> {
    entry(&s.email)?.set_password(pass).map_err(|e| format!("trousseau système : {e}"))?;
    fs::create_dir_all(dir()).map_err(|e| e.to_string())?;
    fs::write(settings_path(), serde_json::to_string_pretty(s).unwrap()).map_err(|e| e.to_string())?;
    crate::mail::reset();
    *CACHE.lock().unwrap() = Some(Config::from_settings(s, pass));
    Ok(())
}

pub fn clear() {
    if let Some(s) = read_settings() { if let Ok(e) = entry(&s.email) { let _ = e.delete_credential(); } }
    let _ = fs::remove_file(settings_path());
    *CACHE.lock().unwrap() = None;
    crate::mail::reset();
}

/// Mode dev : IRIS_ENV=/chemin/.env pour contourner l'écran de connexion.
fn from_env_file() -> Option<Config> {
    let txt = fs::read_to_string(env::var("IRIS_ENV").ok()?).ok()?;
    let m: HashMap<_, _> = txt.lines().filter_map(|l| l.split_once('=')).map(|(k, v)| (k.trim().to_string(), v.trim().to_string())).collect();
    let g = |k: &str, d: &str| m.get(k).cloned().unwrap_or_else(|| d.to_string());
    Some(Config {
        user: g("MAIL_USER", ""), pass: g("MAIL_PASS", ""), imap_host: g("IMAP_HOST", "imap.fastmail.com"),
        imap_port: g("IMAP_PORT", "993").parse().unwrap_or(993), smtp_host: g("SMTP_HOST", "smtp.fastmail.com"),
        smtp_port: g("SMTP_PORT", "465").parse().unwrap_or(465),
        imap_security: Security::Ssl, smtp_security: Security::Ssl,
    })
}

pub fn load() -> Result<Config, String> {
    if let Some(c) = CACHE.lock().unwrap().clone() { return Ok(c); }
    if let Some(c) = from_env_file() { return Ok(c); }
    let s = read_settings().ok_or("non connecté")?;
    let pass = entry(&s.email)?.get_password().map_err(|e| format!("mot de passe introuvable dans le trousseau : {e}"))?;
    let c = Config::from_settings(&s, &pass);
    *CACHE.lock().unwrap() = Some(c.clone());
    Ok(c)
}

/// Reprise des réglages de Courrier : copie settings.json vers le dossier `iris` et
/// ré-enregistre le mot de passe sous le nouveau service du trousseau. Sans effet si déjà migré.
pub fn migrate_legacy() {
    let old = base_dir().join("courrier");
    let Some(s) = migrate_settings(&old, &dir()) else { return };
    let moved = keyring::Entry::new(OLD_SERVICE, &s.email).ok().and_then(|o| {
        let pass = o.get_password().ok()?;
        entry(&s.email).ok()?.set_password(&pass).ok()?;
        let _ = o.delete_credential();
        Some(())
    });
    if moved.is_none() { let _ = fs::remove_file(dir().join("settings.json")); }
}

/// Copie `old/settings.json` vers `new/` s'il n'existe pas encore ; renvoie les réglages copiés.
fn migrate_settings(old: &std::path::Path, new: &std::path::Path) -> Option<Settings> {
    let target = new.join("settings.json");
    if target.exists() { return None; }
    let raw = fs::read_to_string(old.join("settings.json")).ok()?;
    let s: Settings = serde_json::from_str(&raw).ok()?;
    fs::create_dir_all(new).ok()?;
    fs::write(&target, raw).ok()?;
    Some(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp(name: &str) -> PathBuf {
        let d = env::temp_dir().join(format!("iris-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }
    const OLD: &str = r#"{"email":"a@b.fr","imap_host":"i","imap_port":993,"smtp_host":"s","smtp_port":465}"#;

    #[test]
    fn migrate_copies_settings_once() {
        let (old, new) = (tmp("old"), tmp("new").join("iris"));
        fs::write(old.join("settings.json"), OLD).unwrap();
        assert_eq!(migrate_settings(&old, &new).unwrap().email, "a@b.fr");
        assert!(new.join("settings.json").exists());
        assert!(migrate_settings(&old, &new).is_none(), "déjà migré");
    }

    #[test]
    fn migrate_without_old_settings_does_nothing() {
        let (old, new) = (tmp("none-old"), tmp("none-new").join("iris"));
        assert!(migrate_settings(&old, &new).is_none());
        assert!(!new.exists());
    }

    #[test]
    fn old_settings_without_security_keep_working() {
        let s: Settings = serde_json::from_str(OLD).unwrap();
        assert_eq!((s.imap_security(), s.smtp_security()), (Security::Ssl, Security::Ssl));
        let s587: Settings = serde_json::from_str(&OLD.replace("465", "587")).unwrap();
        assert_eq!(s587.smtp_security(), Security::Starttls);
    }

    #[test]
    fn security_round_trips_as_snake_case() {
        let mut s: Settings = serde_json::from_str(OLD).unwrap();
        s.imap_security = Some(Security::Starttls);
        s.smtp_security = Some(Security::None);
        let json = serde_json::to_string(&s).unwrap();
        assert!(json.contains(r#""imap_security":"starttls""#) && json.contains(r#""smtp_security":"none""#));
        let back: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!((back.imap_security(), back.smtp_security()), (Security::Starttls, Security::None));
    }
}
