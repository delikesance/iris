//! Règles de tri : modèle, évaluation pure et validation des webhooks.

use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

const DAY: i64 = 86_400;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Cond {
    SenderIs { value: String },
    SenderDomain { value: String },
    SubjectContains { value: String },
    HasAttachmentExt { ext: String },
    HasListUnsubscribe,
    NoReplyAfterDays { days: u32 },
    /// Toutes les conditions doivent être vraies ; une liste vide ne correspond à rien.
    All { of: Vec<Cond> },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Action {
    MoveTo { folder: String },
    Archive,
    Remind { days: u32 },
    Notify,
    Webhook { url: String },
}

/// Règle active ; la liste passée à `evaluate` est déjà filtrée et triée par position.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Rule {
    pub cond: Cond,
    pub action: Action,
}

/// Faits d'un message utilisés par les conditions. `date` en secondes Unix UTC.
#[derive(Clone, Debug, Default)]
pub struct MsgFacts {
    pub from_addr: String,
    pub subject: String,
    pub has_attach_exts: Vec<String>,
    pub has_list_unsubscribe: bool,
    pub date: i64,
    pub replied: bool,
}

fn norm_ext(e: &str) -> String {
    e.trim().trim_start_matches('.').to_lowercase()
}

pub fn matches(c: &Cond, m: &MsgFacts, now: i64) -> bool {
    match c {
        Cond::SenderIs { value } => m.from_addr.trim().eq_ignore_ascii_case(value.trim()),
        Cond::SenderDomain { value } => m
            .from_addr
            .rsplit_once('@')
            .is_some_and(|(_, d)| d.trim().eq_ignore_ascii_case(value.trim().trim_start_matches('@'))),
        Cond::SubjectContains { value } => {
            !value.is_empty() && m.subject.to_lowercase().contains(&value.to_lowercase())
        }
        Cond::HasAttachmentExt { ext } => {
            let want = norm_ext(ext);
            m.has_attach_exts.iter().any(|e| norm_ext(e) == want)
        }
        Cond::HasListUnsubscribe => m.has_list_unsubscribe,
        Cond::NoReplyAfterDays { days } => {
            !m.replied && now.saturating_sub(m.date) >= i64::from(*days) * DAY
        }
        Cond::All { of } => !of.is_empty() && of.iter().all(|c| matches(c, m, now)),
    }
}

fn cond_text(c: &Cond) -> String {
    match c {
        Cond::SenderIs { value } => format!("l'expéditeur est {value}"),
        Cond::SenderDomain { value } => format!("l'expéditeur est du domaine {value}"),
        Cond::SubjectContains { value } => format!("l'objet contient « {value} »"),
        Cond::HasAttachmentExt { ext } => format!("une pièce jointe .{} est présente", norm_ext(ext)),
        Cond::HasListUnsubscribe => "le message contient un lien de désinscription".into(),
        Cond::NoReplyAfterDays { days } => {
            format!("aucune réponse après {days} jour{}", if *days > 1 { "s" } else { "" })
        }
        Cond::All { of } => of.iter().map(cond_text).collect::<Vec<_>>().join(" et "),
    }
}

fn action_text(a: &Action) -> String {
    match a {
        Action::MoveTo { folder } => format!("classer dans {folder}"),
        Action::Archive => "archiver".into(),
        Action::Remind { days } => {
            format!("me relancer dans {days} jour{}", if *days > 1 { "s" } else { "" })
        }
        Action::Notify => "m'envoyer une notification".into(),
        Action::Webhook { url } => format!("appeler le webhook {url}"),
    }
}

pub fn sentence(c: &Cond, a: &Action) -> String {
    format!("Si {}, alors {}", cond_text(c), action_text(a))
}

/// Actions à appliquer : la première `MoveTo`/`Archive` gagne, les autres s'accumulent.
pub fn evaluate(rules: &[Rule], m: &MsgFacts, now: i64) -> Vec<Action> {
    let mut out = Vec::new();
    let mut filed = false;
    for r in rules.iter().filter(|r| matches(&r.cond, m, now)) {
        let exclusive = matches!(r.action, Action::MoveTo { .. } | Action::Archive);
        if exclusive && filed {
            continue;
        }
        filed |= exclusive;
        out.push(r.action.clone());
    }
    out
}

fn private_v4(ip: Ipv4Addr) -> bool {
    let o = ip.octets();
    ip.is_private()
        || ip.is_loopback()
        || ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_broadcast()
        || (o[0] == 100 && (64..128).contains(&o[1]))
}

fn private_v6(ip: Ipv6Addr) -> bool {
    if let Some(v4) = ip.to_ipv4_mapped() {
        return private_v4(v4);
    }
    let s = ip.segments()[0];
    ip.is_loopback() || ip.is_unspecified() || (s & 0xfe00) == 0xfc00 || (s & 0xffc0) == 0xfe80
}

fn host_of(rest: &str) -> Result<&str, String> {
    let auth = rest.split(['/', '?', '#']).next().unwrap_or("");
    if auth.contains('@') {
        return Err("identifiants dans l'URL interdits".into());
    }
    if let Some(v6) = auth.strip_prefix('[') {
        return v6.split_once(']').map(|(h, _)| h).ok_or_else(|| "URL invalide".into());
    }
    Ok(auth.split(':').next().unwrap_or(""))
}

/// https uniquement ; refuse localhost et les IP littérales privées/locales (pas de résolution DNS).
pub fn validate_webhook(url: &str) -> Result<(), String> {
    let url = url.trim();
    let scheme_ok = url.get(..8).is_some_and(|s| s.eq_ignore_ascii_case("https://"));
    if !scheme_ok {
        return Err("le webhook doit utiliser https".into());
    }
    let host = host_of(&url[8..])?.trim_end_matches('.').to_lowercase();
    if host.is_empty() {
        return Err("URL invalide : hôte manquant".into());
    }
    if host == "localhost" || host.ends_with(".localhost") {
        return Err("hôte local interdit".into());
    }
    let numeric_only = host.chars().all(|c| c.is_ascii_digit() || c == '.');
    match host.parse::<IpAddr>() {
        Ok(IpAddr::V4(ip)) if private_v4(ip) => Err("adresse privée interdite".into()),
        Ok(IpAddr::V6(ip)) if private_v6(ip) => Err("adresse privée interdite".into()),
        Ok(_) => Ok(()),
        Err(_) if numeric_only => Err("adresse IP invalide".into()),
        Err(_) => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: i64 = 1_800_000_000;

    fn facts() -> MsgFacts {
        MsgFacts {
            from_addr: "Compta@Atelier-Norge.fr".into(),
            subject: "Votre Facture de mars".into(),
            has_attach_exts: vec!["PDF".into()],
            has_list_unsubscribe: false,
            date: NOW - DAY,
            replied: false,
        }
    }

    fn invoices() -> (Cond, Action) {
        (
            Cond::All {
                of: vec![
                    Cond::HasAttachmentExt { ext: "pdf".into() },
                    Cond::SubjectContains { value: "facture".into() },
                ],
            },
            Action::MoveTo { folder: "Factures".into() },
        )
    }

    fn newsletters() -> (Cond, Action) {
        (Cond::HasListUnsubscribe, Action::MoveTo { folder: "Newsletters".into() })
    }

    fn follow_up() -> (Cond, Action) {
        (Cond::NoReplyAfterDays { days: 3 }, Action::MoveTo { folder: "À relancer".into() })
    }

    #[test]
    fn pdf_example_invoices() {
        let (c, a) = invoices();
        assert!(matches(&c, &facts(), NOW));
        let no_pdf = MsgFacts { has_attach_exts: vec!["docx".into()], ..facts() };
        assert!(!matches(&c, &no_pdf, NOW));
        let other = MsgFacts { subject: "Bonjour".into(), ..facts() };
        assert!(!matches(&c, &other, NOW));
        assert_eq!(
            sentence(&c, &a),
            "Si une pièce jointe .pdf est présente et l'objet contient « facture », alors classer dans Factures"
        );
    }

    #[test]
    fn pdf_example_newsletters() {
        let (c, a) = newsletters();
        assert!(!matches(&c, &facts(), NOW));
        let m = MsgFacts { has_list_unsubscribe: true, ..facts() };
        assert!(matches(&c, &m, NOW));
        assert_eq!(
            sentence(&c, &a),
            "Si le message contient un lien de désinscription, alors classer dans Newsletters"
        );
    }

    #[test]
    fn pdf_example_follow_up() {
        let (c, a) = follow_up();
        let old = MsgFacts { date: NOW - 3 * DAY, ..facts() };
        assert!(matches(&c, &old, NOW));
        assert!(!matches(&c, &MsgFacts { date: NOW - 3 * DAY + 1, ..facts() }, NOW));
        assert!(!matches(&c, &MsgFacts { replied: true, ..old }, NOW));
        assert_eq!(
            sentence(&c, &a),
            "Si aucune réponse après 3 jours, alors classer dans À relancer"
        );
    }

    #[test]
    fn sender_conditions() {
        let is = Cond::SenderIs { value: "compta@atelier-norge.fr".into() };
        let dom = Cond::SenderDomain { value: "atelier-norge.fr".into() };
        let other = Cond::SenderDomain { value: "norge.fr".into() };
        assert!(matches(&is, &facts(), NOW));
        assert!(matches(&dom, &facts(), NOW));
        assert!(!matches(&other, &facts(), NOW));
        assert_eq!(
            sentence(&dom, &Action::MoveTo { folder: "Clients".into() }),
            "Si l'expéditeur est du domaine atelier-norge.fr, alors classer dans Clients"
        );
    }

    #[test]
    fn empty_all_and_empty_subject_never_match() {
        assert!(!matches(&Cond::All { of: vec![] }, &facts(), NOW));
        assert!(!matches(&Cond::SubjectContains { value: String::new() }, &facts(), NOW));
    }

    #[test]
    fn attachment_ext_ignores_dot_and_case() {
        let c = Cond::HasAttachmentExt { ext: ".Pdf".into() };
        assert!(matches(&c, &facts(), NOW));
    }

    #[test]
    fn sentences_for_other_actions() {
        let c = Cond::SenderIs { value: "a@b.fr".into() };
        assert_eq!(sentence(&c, &Action::Archive), "Si l'expéditeur est a@b.fr, alors archiver");
        assert_eq!(
            sentence(&c, &Action::Remind { days: 1 }),
            "Si l'expéditeur est a@b.fr, alors me relancer dans 1 jour"
        );
        assert_eq!(
            sentence(&c, &Action::Notify),
            "Si l'expéditeur est a@b.fr, alors m'envoyer une notification"
        );
        assert_eq!(
            sentence(&c, &Action::Webhook { url: "https://x.fr/h".into() }),
            "Si l'expéditeur est a@b.fr, alors appeler le webhook https://x.fr/h"
        );
    }

    #[test]
    fn evaluate_first_filing_wins_and_others_accumulate() {
        let rules: Vec<Rule> = [
            (Cond::SenderDomain { value: "atelier-norge.fr".into() }, Action::Notify),
            invoices(),
            (Cond::SenderIs { value: "compta@atelier-norge.fr".into() }, Action::Archive),
            (Cond::SubjectContains { value: "facture".into() }, Action::Webhook { url: "https://x.fr/h".into() }),
            (Cond::SubjectContains { value: "facture".into() }, Action::Remind { days: 2 }),
        ]
        .into_iter()
        .map(|(cond, action)| Rule { cond, action })
        .collect();
        assert_eq!(
            evaluate(&rules, &facts(), NOW),
            vec![
                Action::Notify,
                Action::MoveTo { folder: "Factures".into() },
                Action::Webhook { url: "https://x.fr/h".into() },
                Action::Remind { days: 2 },
            ]
        );
    }

    #[test]
    fn evaluate_no_match_is_empty() {
        let (cond, action) = newsletters();
        assert!(evaluate(&[Rule { cond, action }], &facts(), NOW).is_empty());
    }

    #[test]
    fn serde_tagged_roundtrip() {
        let (c, _) = invoices();
        let json = serde_json::to_string(&c).unwrap();
        assert!(json.contains(r#""type":"all""#) && json.contains(r#""type":"has_attachment_ext""#));
        assert_eq!(serde_json::from_str::<Cond>(&json).unwrap(), c);
        let a: Action = serde_json::from_str(r#"{"type":"archive"}"#).unwrap();
        assert_eq!(a, Action::Archive);
    }

    #[test]
    fn webhook_accepts_public_https() {
        for u in ["https://hooks.example.com/x", "https://example.com:8443/a?b=1", "HTTPS://8.8.8.8/h", "https://[2606:4700::1111]/h"] {
            assert!(validate_webhook(u).is_ok(), "{u}");
        }
    }

    #[test]
    fn webhook_rejects_bad_urls() {
        for u in [
            "http://example.com/x",
            "ftp://example.com",
            "example.com",
            "https://",
            "https:///path",
            "https://user:pw@example.com/",
            "https://localhost/x",
            "https://api.localhost:8080/",
            "https://127.0.0.1/x",
            "https://10.0.0.5/x",
            "https://172.16.3.4/x",
            "https://192.168.1.1:8443/x",
            "https://169.254.169.254/latest",
            "https://100.64.0.1/",
            "https://0.0.0.0/",
            "https://2130706433/",
            "https://[::1]/x",
            "https://[fc00::1]/x",
            "https://[fd12:3456::1]/x",
            "https://[fe80::1]/x",
            "https://[::ffff:127.0.0.1]/x",
        ] {
            assert!(validate_webhook(u).is_err(), "{u}");
        }
    }
}
