//! Regroupement des messages en conversations.
//!
//! Union-find sur Message-ID / In-Reply-To / References, puis repli sur le
//! sujet normalisé (30 jours, au moins un participant en commun).

#![allow(dead_code)]

use regex::Regex;
use std::collections::HashMap;
use std::sync::OnceLock;

const SUBJECT_WINDOW_SECS: i64 = 30 * 24 * 3600;

/// Message minimal nécessaire au regroupement.
#[derive(Debug, Clone, Default)]
pub struct Msg {
    /// Message-ID sans chevrons (peut être vide).
    pub message_id: String,
    pub in_reply_to: String,
    pub references: Vec<String>,
    pub subject: String,
    /// Date en secondes Unix.
    pub date: i64,
    /// Adresses (expéditeur + destinataires), en minuscules.
    pub participants: Vec<String>,
    /// Renseigné par `assign_threads`.
    pub thread_id: String,
}

fn clean_id(s: &str) -> String {
    s.trim().trim_matches(|c| c == '<' || c == '>').trim().to_lowercase()
}

struct Dsu(Vec<usize>);

impl Dsu {
    fn find(&mut self, x: usize) -> usize {
        let mut r = x;
        while self.0[r] != r {
            r = self.0[r];
        }
        let mut c = x;
        while self.0[c] != r {
            let n = self.0[c];
            self.0[c] = r;
            c = n;
        }
        r
    }
    fn union(&mut self, a: usize, b: usize) {
        let (ra, rb) = (self.find(a), self.find(b));
        if ra != rb {
            self.0[ra.max(rb)] = ra.min(rb);
        }
    }
}

/// Retire les préfixes Re:/Fwd:/TR:… et normalise casse et espaces.
pub fn normalize_subject(subject: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"(?i)^\s*(re|fwd?|tr|réf|ref|aw|sv|wg)\s*(\[\d+\])?\s*:\s*").unwrap()
    });
    let mut s = subject.trim().to_string();
    loop {
        let next = re.replace(&s, "").to_string();
        if next == s {
            break;
        }
        s = next;
    }
    s.split_whitespace().collect::<Vec<_>>().join(" ").to_lowercase()
}

/// Affecte `thread_id` à chaque message. L'identifiant est celui du message
/// le plus ancien du groupe (son Message-ID, sinon `t<index>`).
pub fn assign_threads(msgs: &mut [Msg]) {
    let n = msgs.len();
    // Nœuds 0..n = messages ; au-delà = identifiants référencés mais absents.
    let mut ids: HashMap<String, usize> = HashMap::new();
    let mut total = n;
    for (i, m) in msgs.iter().enumerate() {
        let id = clean_id(&m.message_id);
        if !id.is_empty() {
            ids.entry(id).or_insert(i);
        }
    }
    let mut node = |id: &str| -> usize {
        *ids.entry(id.to_string()).or_insert_with(|| {
            total += 1;
            total - 1
        })
    };
    let mut links: Vec<(usize, usize)> = Vec::new();
    for (i, m) in msgs.iter().enumerate() {
        let chain: Vec<String> = m
            .references
            .iter()
            .map(|r| clean_id(r))
            .filter(|r| !r.is_empty())
            .collect();
        for r in &chain {
            links.push((i, node(r)));
        }
        for w in chain.windows(2) {
            links.push((node(&w[0]), node(&w[1])));
        }
        let irt = clean_id(&m.in_reply_to);
        if !irt.is_empty() {
            links.push((i, node(&irt)));
        }
    }
    let mut dsu = Dsu((0..total).collect());
    for (a, b) in links {
        dsu.union(a, b);
    }

    // Repli : sujet normalisé, 30 jours, participant commun.
    let mut by_subject: HashMap<String, Vec<usize>> = HashMap::new();
    for (i, m) in msgs.iter().enumerate() {
        let s = normalize_subject(&m.subject);
        if !s.is_empty() {
            by_subject.entry(s).or_default().push(i);
        }
    }
    for group in by_subject.values() {
        for (x, &a) in group.iter().enumerate() {
            for &b in &group[x + 1..] {
                let close = (msgs[a].date - msgs[b].date).abs() <= SUBJECT_WINDOW_SECS;
                let shared = msgs[a]
                    .participants
                    .iter()
                    .any(|p| msgs[b].participants.iter().any(|q| p.eq_ignore_ascii_case(q)));
                if close && shared {
                    dsu.union(a, b);
                }
            }
        }
    }

    // Le plus ancien message de chaque composante donne l'identifiant.
    let mut oldest: HashMap<usize, usize> = HashMap::new();
    for i in 0..n {
        let r = dsu.find(i);
        let e = oldest.entry(r).or_insert(i);
        if (msgs[i].date, i) < (msgs[*e].date, *e) {
            *e = i;
        }
    }
    let tids: Vec<String> = (0..n)
        .map(|i| {
            let o = oldest[&dsu.find(i)];
            let id = clean_id(&msgs[o].message_id);
            if id.is_empty() {
                format!("t{o}")
            } else {
                id
            }
        })
        .collect();
    for (m, t) in msgs.iter_mut().zip(tids) {
        m.thread_id = t;
    }
}

fn split_version(name: &str) -> (String, Option<u32>, String) {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| {
        Regex::new(r"(?i)^(.*?)(?:[\s_\-.]+(?:v|ver|version|rev)?\s*|(?:v|ver|rev))(\d{1,4})$").unwrap()
    });
    let (stem, ext) = match name.rfind('.') {
        Some(p) if p > 0 => (&name[..p], name[p + 1..].to_lowercase()),
        _ => (name, String::new()),
    };
    let stem = stem.trim();
    match re.captures(stem) {
        Some(c) if !c[1].trim().is_empty() => (c[1].trim().to_lowercase(), c[2].parse().ok(), ext),
        _ => (stem.to_lowercase(), None, ext),
    }
}

/// `true` si `new_name` est une version plus récente de `old_name`
/// (contrat_v2.pdf → contrat_v3.pdf).
pub fn supersedes(old_name: &str, new_name: &str) -> bool {
    let (ob, ov, oe) = split_version(old_name);
    let (nb, nv, ne) = split_version(new_name);
    if ob.is_empty() || ob != nb || oe != ne {
        return false;
    }
    match (ov, nv) {
        (Some(o), Some(n)) => n > o,
        (None, Some(n)) => n > 1,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: i64 = 86_400;

    fn msg(id: &str, irt: &str, refs: &[&str], subj: &str, day: i64, who: &[&str]) -> Msg {
        Msg {
            message_id: id.into(),
            in_reply_to: irt.into(),
            references: refs.iter().map(|s| s.to_string()).collect(),
            subject: subj.into(),
            date: day * DAY,
            participants: who.iter().map(|s| s.to_string()).collect(),
            thread_id: String::new(),
        }
    }

    #[test]
    fn normalize_strips_prefixes() {
        assert_eq!(normalize_subject("Re: Re: Fwd:  Devis  Mars"), "devis mars");
        assert_eq!(normalize_subject("RE : TR : Contrat"), "contrat");
        assert_eq!(normalize_subject("Re[2]: Hello"), "hello");
        assert_eq!(normalize_subject("Revue du budget"), "revue du budget");
        assert_eq!(normalize_subject("   "), "");
    }

    #[test]
    fn re_re_chain_is_one_thread() {
        let mut v = vec![
            msg("a@x", "", &[], "Devis", 1, &["a@x.fr", "b@y.fr"]),
            msg("b@x", "<a@x>", &["<a@x>"], "Re: Devis", 2, &["b@y.fr", "a@x.fr"]),
            msg("c@x", "<b@x>", &["<a@x>", "<b@x>"], "Re: Re: Devis", 3, &["a@x.fr", "b@y.fr"]),
        ];
        assign_threads(&mut v);
        assert!(v.iter().all(|m| m.thread_id == "a@x"));
    }

    #[test]
    fn references_only_links_through_missing_parent() {
        let mut v = vec![
            msg("r1", "", &["<root>"], "Alpha", 1, &["a@x.fr"]),
            msg("r2", "", &["<root>", "<r1>"], "Beta", 5, &["c@z.fr"]),
            msg("other", "", &[], "Gamma", 2, &["d@w.fr"]),
        ];
        assign_threads(&mut v);
        assert_eq!(v[0].thread_id, v[1].thread_id);
        assert_ne!(v[0].thread_id, v[2].thread_id);
    }

    #[test]
    fn same_subject_different_people_not_merged() {
        let mut v = vec![
            msg("1", "", &[], "Facture", 1, &["a@x.fr", "me@i.fr"]),
            msg("2", "", &[], "Facture", 2, &["z@y.fr", "other@i.fr"]),
        ];
        assign_threads(&mut v);
        assert_ne!(v[0].thread_id, v[1].thread_id);
    }

    #[test]
    fn subject_fallback_merges_with_shared_participant_within_30_days() {
        let mut v = vec![
            msg("1", "", &[], "Réunion", 1, &["a@x.fr", "me@i.fr"]),
            msg("2", "", &[], "RE: réunion", 10, &["A@x.fr", "me@i.fr"]),
            msg("3", "", &[], "Réunion", 100, &["a@x.fr", "me@i.fr"]),
        ];
        assign_threads(&mut v);
        assert_eq!(v[0].thread_id, v[1].thread_id);
        assert_ne!(v[0].thread_id, v[2].thread_id);
    }

    #[test]
    fn missing_message_id_gets_synthetic_thread_id() {
        let mut v = vec![msg("", "", &[], "Seul", 1, &["a@x.fr"])];
        assign_threads(&mut v);
        assert_eq!(v[0].thread_id, "t0");
    }

    #[test]
    fn supersedes_cases() {
        assert!(supersedes("contrat_v2.pdf", "contrat_v3.pdf"));
        assert!(supersedes("Contrat V2.PDF", "contrat_v10.pdf"));
        assert!(supersedes("contrat.pdf", "contrat_v2.pdf"));
        assert!(supersedes("devis-1.docx", "devis-2.docx"));
        assert!(!supersedes("contrat_v3.pdf", "contrat_v2.pdf"));
        assert!(!supersedes("contrat_v2.pdf", "contrat_v2.pdf"));
        assert!(!supersedes("contrat_v2.pdf", "contrat_v3.docx"));
        assert!(!supersedes("contrat_v2.pdf", "facture_v3.pdf"));
        assert!(!supersedes("contrat.pdf", "facture.pdf"));
    }
}
