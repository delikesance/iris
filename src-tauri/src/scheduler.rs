//! Planificateur : fonctions pures sur des expressions cron à 5 champs
//! (minute heure jour-du-mois mois jour-de-semaine). Aucun thread ici ;
//! la boucle d'exécution vit dans l'automatisation.
#![allow(dead_code)]

use chrono::{DateTime, Duration, TimeZone};
use cron::Schedule;
use std::str::FromStr;

/// Une tâche planifiée vue par le planificateur.
#[derive(Debug, Clone, PartialEq)]
pub struct Chron {
    pub id: i64,
    pub expr: String,
    pub enabled: bool,
    /// Dernière exécution (secondes Unix UTC). `None` = jamais exécutée.
    pub last_run: Option<i64>,
}

/// Politique de rattrapage : une exécution manquée depuis moins de 24 h est jouée une fois.
pub const CATCH_UP_WINDOW_SECS: i64 = 24 * 3600;

const DAYS: [&str; 7] = ["SUN", "MON", "TUE", "WED", "THU", "FRI", "SAT"];

fn dow_number(tok: &str) -> Result<u32, String> {
    let up = tok.to_ascii_uppercase();
    if let Some(i) = DAYS.iter().position(|d| *d == up) {
        return Ok(i as u32);
    }
    match up.parse::<u32>() {
        Ok(n) if n <= 7 => Ok(n % 7),
        _ => Err(format!("jour de semaine invalide : {tok}")),
    }
}

/// Convertit le champ jour-de-semaine (0/7 = dimanche) vers la numérotation
/// de la crate `cron` (1 = dimanche … 7 = samedi).
fn convert_dow(field: &str) -> Result<String, String> {
    if field == "*" || field == "?" {
        return Ok(field.to_string());
    }
    let mut out: Vec<String> = Vec::new();
    for item in field.split(',') {
        let (range, step) = match item.split_once('/') {
            Some((r, s)) => (r, Some(s.parse::<u32>().map_err(|_| "pas invalide".to_string())?)),
            None => (item, None),
        };
        if range == "*" {
            out.push(item.to_string());
            continue;
        }
        let days: Vec<u32> = if let Some((a, b)) = range.split_once('-') {
            let (a, b) = (dow_number(a)?, dow_number(b)?);
            // 5-7 (ven-dim) : 7 compte comme dimanche en fin de plage
            let b = if b == 0 && a != 0 { 7 } else { b };
            if a > b {
                return Err("plage de jours invalide".into());
            }
            (a..=b).step_by(step.unwrap_or(1).max(1) as usize).map(|d| d % 7).collect()
        } else {
            vec![dow_number(range)?]
        };
        out.extend(days.iter().map(|d| (d + 1).to_string()));
    }
    Ok(out.join(","))
}

fn parse(expr: &str) -> Result<Schedule, String> {
    let f: Vec<&str> = expr.split_whitespace().collect();
    if f.len() != 5 {
        return Err(format!(
            "expression cron invalide : 5 champs attendus, {} reçus",
            f.len()
        ));
    }
    let dow = convert_dow(f[4])?;
    let full = format!("0 {} {} {} {} {}", f[0], f[1], f[2], f[3], dow);
    Schedule::from_str(&full).map_err(|e| format!("expression cron invalide : {e}"))
}

/// Valide une expression cron à 5 champs.
pub fn validate(expr: &str) -> Result<(), String> {
    parse(expr).map(|_| ())
}

/// Prochaine exécution strictement après `after` (dans son fuseau).
pub fn next_run<Tz: TimeZone>(expr: &str, after: &DateTime<Tz>) -> Result<DateTime<Tz>, String> {
    parse(expr)?
        .after(after)
        .next()
        .ok_or_else(|| "aucune prochaine exécution".to_string())
}

fn num(s: &str) -> Option<u32> {
    s.parse::<u32>().ok()
}

/// Description en français, ou l'expression brute si le motif n'est pas reconnu.
pub fn describe(expr: &str) -> String {
    let f: Vec<&str> = expr.split_whitespace().collect();
    if f.len() != 5 || validate(expr).is_err() {
        return "Expression invalide".to_string();
    }
    let (min, hour, dom, mon, dow) = (f[0], f[1], f[2], f[3], f[4]);
    let (Some(m), Some(h)) = (num(min), num(hour)) else {
        return expr.to_string();
    };
    let at = format!("à {h:02}:{m:02}");
    if mon != "*" {
        return expr.to_string();
    }
    match (dom, dow) {
        ("*", "*") => format!("Tous les jours {at}"),
        ("*", "1-5") => format!("Du lundi au vendredi {at}"),
        ("*", d) => match dow_number(d) {
            Ok(n) => {
                let name = [
                    "dimanches", "lundis", "mardis", "mercredis", "jeudis", "vendredis", "samedis",
                ][n as usize];
                format!("Tous les {name} {at}")
            }
            Err(_) => expr.to_string(),
        },
        (d, "*") => match num(d) {
            Some(1) => format!("Le 1er de chaque mois {at}"),
            Some(n) => format!("Le {n} de chaque mois {at}"),
            None => expr.to_string(),
        },
        _ => expr.to_string(),
    }
}

/// Ids des tâches activées dont une exécution est échue à `now`.
/// Une tâche jamais exécutée n'est pas due.
pub fn due<Tz: TimeZone>(chrons: &[Chron], now: &DateTime<Tz>) -> Vec<i64> {
    chrons
        .iter()
        .filter(|c| c.enabled)
        .filter(|c| {
            let Some(last) = c.last_run.and_then(|t| DateTime::from_timestamp(t, 0)) else {
                return false;
            };
            let last = last.with_timezone(&now.timezone());
            matches!(next_run(&c.expr, &last), Ok(n) if n <= *now)
        })
        .map(|c| c.id)
        .collect()
}

/// Rattrapage au démarrage : `true` si au moins une exécution a été manquée
/// depuis `last_run` et qu'elle date de moins de 24 h (jouée alors une seule
/// fois). `last_run = None` ne rattrape rien.
pub fn catch_up<Tz: TimeZone>(expr: &str, last_run: Option<i64>, now: &DateTime<Tz>) -> bool {
    let Some(last) = last_run.and_then(|t| DateTime::from_timestamp(t, 0)) else {
        return false;
    };
    let last = last.with_timezone(&now.timezone());
    let Ok(sched) = parse(expr) else { return false };
    let window_start = now.clone() - Duration::seconds(CATCH_UP_WINDOW_SECS);
    let from = if window_start > last { window_start } else { last.clone() };
    sched.after(&from).take_while(|t| t <= now).any(|t| t > last)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{Datelike, Timelike, Utc, Weekday};

    fn utc(y: i32, mo: u32, d: u32, h: u32, mi: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(y, mo, d, h, mi, 0).unwrap()
    }

    #[test]
    fn weekdays_after_friday_goes_to_monday() {
        // 2026-10-30 est un vendredi
        let after = utc(2026, 10, 30, 8, 1);
        assert_eq!(after.weekday(), Weekday::Fri);
        let n = next_run("0 8 * * 1-5", &after).unwrap();
        assert_eq!(n, utc(2026, 11, 2, 8, 0));
        assert_eq!(n.weekday(), Weekday::Mon);
    }

    #[test]
    fn daily_nine() {
        let n = next_run("0 9 * * *", &utc(2026, 10, 30, 9, 0)).unwrap();
        assert_eq!(n, utc(2026, 10, 31, 9, 0));
        let n = next_run("0 9 * * *", &utc(2026, 10, 30, 8, 59)).unwrap();
        assert_eq!(n, utc(2026, 10, 30, 9, 0));
    }

    #[test]
    fn sunday_zero_and_seven() {
        let n = next_run("0 20 * * 0", &utc(2026, 10, 30, 12, 0)).unwrap();
        assert_eq!(n.weekday(), Weekday::Sun);
        assert_eq!((n.day(), n.hour()), (1, 20));
        assert_eq!(next_run("0 20 * * 7", &utc(2026, 10, 30, 12, 0)).unwrap(), n);
    }

    #[test]
    fn monthly_first() {
        let n = next_run("0 7 1 * *", &utc(2026, 10, 30, 12, 0)).unwrap();
        assert_eq!(n, utc(2026, 11, 1, 7, 0));
    }

    #[test]
    fn invalid_expressions() {
        assert!(validate("pas du cron").is_err());
        assert!(validate("0 8 * *").is_err());
        assert!(validate("61 8 * * *").is_err());
        assert!(validate("0 25 * * *").is_err());
        assert!(next_run("nope", &utc(2026, 1, 1, 0, 0)).is_err());
        assert!(validate("0 8 * * 1-5").is_ok());
        assert!(validate("*/15 * * * *").is_ok());
    }

    #[test]
    fn describe_french() {
        assert_eq!(describe("0 8 * * 1-5"), "Du lundi au vendredi à 08:00");
        assert_eq!(describe("0 9 * * *"), "Tous les jours à 09:00");
        assert_eq!(describe("0 20 * * 0"), "Tous les dimanches à 20:00");
        assert_eq!(describe("0 7 1 * *"), "Le 1er de chaque mois à 07:00");
        assert_eq!(describe("garbage"), "Expression invalide");
    }

    fn chron(id: i64, expr: &str, enabled: bool, last: Option<DateTime<Utc>>) -> Chron {
        Chron { id, expr: expr.into(), enabled, last_run: last.map(|d| d.timestamp()) }
    }

    #[test]
    fn due_selects_elapsed_enabled_only() {
        let now = utc(2026, 10, 30, 9, 0);
        let cs = vec![
            chron(1, "0 9 * * *", true, Some(utc(2026, 10, 29, 9, 0))),   // échu
            chron(2, "0 9 * * *", true, Some(utc(2026, 10, 30, 9, 0))),   // déjà fait
            chron(3, "0 9 * * *", false, Some(utc(2026, 10, 29, 9, 0))),  // en pause
            chron(4, "0 9 * * *", true, None),                            // jamais lancé
            chron(5, "0 20 * * 0", true, Some(utc(2026, 10, 25, 20, 0))), // pas encore
        ];
        assert_eq!(due(&cs, &now), vec![1]);
    }

    #[test]
    fn catch_up_policy() {
        let now = utc(2026, 10, 30, 10, 0);
        // manqué à 09:00 aujourd'hui (dernier run hier) -> rattrapé
        assert!(catch_up("0 9 * * *", Some(utc(2026, 10, 29, 9, 0).timestamp()), &now));
        // rien de manqué
        assert!(!catch_up("0 9 * * *", Some(utc(2026, 10, 30, 9, 0).timestamp()), &now));
        // seule échéance manquée il y a plus de 24 h (hebdo dimanche) -> ignorée
        assert!(!catch_up("0 20 * * 0", Some(utc(2026, 10, 18, 20, 0).timestamp()), &now));
        // jamais lancé -> rien
        assert!(!catch_up("0 9 * * *", None, &now));
    }
}
