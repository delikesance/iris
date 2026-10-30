//! Classification des expéditeurs récurrents (newsletters, notifications, documents),
//! détection de cadence et de nouveaux expéditeurs. Fonctions pures, sans I/O.
#![allow(dead_code)]

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Newsletters,
    Notifications,
    Documents,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Cadence {
    Quotidien,
    Hebdo,
    Mensuel,
    #[serde(rename = "Irrégulier")]
    Irregulier,
}

impl Cadence {
    pub fn label(&self) -> &'static str {
        match self {
            Cadence::Quotidien => "Quotidien",
            Cadence::Hebdo => "Hebdo",
            Cadence::Mensuel => "Mensuel",
            Cadence::Irregulier => "Irrégulier",
        }
    }
}

/// Résumé des messages reçus d'un même expéditeur.
#[derive(Debug, Clone, Default)]
pub struct SenderHistory {
    pub addr: String,
    pub subjects: Vec<String>,
    /// Dates (secondes Unix) des messages reçus.
    pub dates: Vec<i64>,
    pub has_list_unsubscribe: bool,
    /// Extensions de pièces jointes (minuscules, sans point), tous messages confondus.
    pub attachment_exts: Vec<String>,
}

const DAY: f64 = 86_400.0;
const MIN_MESSAGES: usize = 3;
const DOC_WORDS: [&str; 7] = ["facture", "relevé", "releve", "contrat", "invoice", "statement", "avis d'échéance"];

fn is_automated_addr(addr: &str) -> bool {
    let local = addr.split('@').next().unwrap_or("").to_lowercase();
    let local = local.replace(['-', '_', '.'], "");
    ["noreply", "donotreply", "notifications", "notification"]
        .iter()
        .any(|p| local == *p || local.starts_with(p))
}

fn is_documents(h: &SenderHistory) -> bool {
    let has_pdf = h.attachment_exts.iter().any(|e| e.eq_ignore_ascii_case("pdf"));
    has_pdf
        && h.subjects.iter().any(|s| {
            let s = s.to_lowercase();
            DOC_WORDS.iter().any(|w| s.contains(w))
        })
}

/// Type de l'expéditeur, ou `None` s'il n'est pas récurrent.
/// Priorité : Documents > Newsletters (List-Unsubscribe) > Notifications (adresse automatique).
/// Il faut au moins 3 messages.
pub fn classify(h: &SenderHistory) -> Option<Kind> {
    if h.dates.len().max(h.subjects.len()) < MIN_MESSAGES {
        return None;
    }
    if is_documents(h) {
        Some(Kind::Documents)
    } else if h.has_list_unsubscribe {
        Some(Kind::Newsletters)
    } else if is_automated_addr(&h.addr) {
        Some(Kind::Notifications)
    } else {
        None
    }
}

/// Cadence d'après l'écart médian entre dates consécutives (secondes Unix).
/// Moins de 3 dates, ou écarts trop dispersés (±40 % de la médiane) : Irrégulier.
pub fn cadence(dates: &[i64]) -> Cadence {
    if dates.len() < 3 {
        return Cadence::Irregulier;
    }
    let mut d = dates.to_vec();
    d.sort_unstable();
    let mut gaps: Vec<f64> = d.windows(2).map(|w| (w[1] - w[0]) as f64 / DAY).collect();
    gaps.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let n = gaps.len();
    let median = if n % 2 == 1 { gaps[n / 2] } else { (gaps[n / 2 - 1] + gaps[n / 2]) / 2.0 };
    if median <= 0.0 {
        return Cadence::Irregulier;
    }
    let steady = gaps.iter().filter(|g| (**g - median).abs() <= 0.4 * median).count();
    if (steady as f64) < 0.7 * n as f64 {
        return Cadence::Irregulier;
    }
    match median {
        m if (0.5..=1.5).contains(&m) => Cadence::Quotidien,
        m if (5.0..=9.0).contains(&m) => Cadence::Hebdo,
        m if (26.0..=35.0).contains(&m) => Cadence::Mensuel,
        _ => Cadence::Irregulier,
    }
}

/// Nouvel expéditeur : jamais décidé par l'utilisateur, ni contact connu, ni destinataire
/// d'un de ses envois, et pas encore ancien (≤ 2 messages reçus).
pub fn is_new_sender(
    addr: &str,
    received_count: usize,
    known: &HashSet<String>,
    decided: &HashSet<String>,
) -> bool {
    let a = addr.trim().to_lowercase();
    !a.is_empty() && received_count <= 2 && !known.contains(&a) && !decided.contains(&a)
}

#[cfg(test)]
mod tests {
    use super::*;

    const D: i64 = 86_400;
    const T0: i64 = 1_700_000_000;

    fn series(step_days: i64, n: usize) -> Vec<i64> {
        (0..n as i64).map(|i| T0 + i * step_days * D).collect()
    }

    fn hist(addr: &str) -> SenderHistory {
        SenderHistory {
            addr: addr.into(),
            subjects: vec!["a".into(), "b".into(), "c".into()],
            dates: series(7, 3),
            ..Default::default()
        }
    }

    #[test]
    fn cadence_daily_weekly_monthly() {
        assert_eq!(cadence(&series(1, 10)), Cadence::Quotidien);
        assert_eq!(cadence(&series(7, 6)), Cadence::Hebdo);
        assert_eq!(cadence(&series(30, 5)), Cadence::Mensuel);
    }

    #[test]
    fn cadence_tolerates_jitter_and_unsorted() {
        let mut v: Vec<i64> = [0, 7, 14, 22, 28, 35].iter().map(|d| T0 + d * D + 3600).collect();
        v.reverse();
        assert_eq!(cadence(&v), Cadence::Hebdo);
        let monthly: Vec<i64> = [0, 31, 59, 90, 120].iter().map(|d| T0 + d * D).collect();
        assert_eq!(cadence(&monthly), Cadence::Mensuel);
    }

    #[test]
    fn cadence_irregular() {
        assert_eq!(cadence(&[]), Cadence::Irregulier);
        assert_eq!(cadence(&series(7, 2)), Cadence::Irregulier);
        let v: Vec<i64> = [0, 1, 9, 10, 40, 41, 90].iter().map(|d| T0 + d * D).collect();
        assert_eq!(cadence(&v), Cadence::Irregulier);
        assert_eq!(cadence(&series(15, 5)), Cadence::Irregulier);
        assert_eq!(cadence(&[T0, T0, T0]), Cadence::Irregulier);
    }

    #[test]
    fn cadence_labels() {
        assert_eq!(Cadence::Irregulier.label(), "Irrégulier");
        assert_eq!(Cadence::Hebdo.label(), "Hebdo");
    }

    #[test]
    fn classify_newsletter() {
        let mut h = hist("news@brand.com");
        h.has_list_unsubscribe = true;
        assert_eq!(classify(&h), Some(Kind::Newsletters));
    }

    #[test]
    fn classify_notifications() {
        for a in ["no-reply@github.com", "noreply@x.io", "Notifications@linkedin.com", "do_not_reply@bank.fr"] {
            assert_eq!(classify(&hist(a)), Some(Kind::Notifications), "{a}");
        }
    }

    #[test]
    fn classify_documents_needs_pdf_and_subject() {
        let mut h = hist("compta@edf.fr");
        h.subjects = vec!["Votre facture de mars".into(), "x".into(), "y".into()];
        assert_eq!(classify(&h), None); // pas de PDF
        h.attachment_exts = vec!["pdf".into()];
        assert_eq!(classify(&h), Some(Kind::Documents));
        h.subjects = vec!["salut".into(), "x".into(), "y".into()];
        assert_eq!(classify(&h), None); // PDF mais sujets sans mot-clé
    }

    #[test]
    fn classify_documents_beats_newsletter() {
        let mut h = hist("no-reply@bank.fr");
        h.has_list_unsubscribe = true;
        h.subjects = vec!["Relevé de compte".into(), "Relevé".into(), "Relevé".into()];
        h.attachment_exts = vec!["PDF".into()];
        assert_eq!(classify(&h), Some(Kind::Documents));
    }

    #[test]
    fn classify_needs_three_messages_and_ignores_humans() {
        let mut h = hist("no-reply@x.io");
        h.dates.truncate(2);
        h.subjects.truncate(2);
        assert_eq!(classify(&h), None);
        assert_eq!(classify(&hist("alice@example.com")), None);
        assert_eq!(classify(&hist("noreplyfan@example.com")), Some(Kind::Notifications)); // préfixe
    }

    fn set(v: &[&str]) -> HashSet<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn new_sender_rules() {
        let known = set(["bob@x.fr"].as_ref());
        let decided = set(["spam@x.fr"].as_ref());
        assert!(is_new_sender("Fresh@Shop.com", 1, &known, &decided));
        assert!(!is_new_sender("bob@x.fr", 1, &known, &decided));
        assert!(!is_new_sender("BOB@x.fr", 1, &known, &decided));
        assert!(!is_new_sender("spam@x.fr", 1, &known, &decided));
        assert!(!is_new_sender("fresh@shop.com", 5, &known, &decided));
        assert!(!is_new_sender("  ", 1, &known, &decided));
    }
}
