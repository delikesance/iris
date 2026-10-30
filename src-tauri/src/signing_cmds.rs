//! Logique des commandes de signature : documents, signataires, journal de preuve.
//!
//! Tout le cœur travaille sur une connexion SQLite et un dossier `<config>/iris/documents`
//! fournis par l'appelant, ce qui le rend testable hors ligne (base en mémoire, dossier temporaire).
//! Les fichiers sont stockés sous `<id>_<nom>` ; la signature manuscrite dans `signature.png`.

use crate::signing::{self, Sealed};
use crate::threading::supersedes;
use crate::types::{Document, DocumentTab, Draft, Proof, ProofEvent, SignatureLevel, Signer};
use rusqlite::{params, Connection, OptionalExtension};
use std::fs;
use std::path::{Path, PathBuf};

const SIGNATURE_FILE: &str = "signature.png";
const PNG_MAGIC: &[u8] = b"\x89PNG\r\n\x1a\n";

fn e(x: impl std::fmt::Display) -> String { x.to_string() }

// ---------- base64 (sans dépendance) ----------

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

pub fn b64_encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for i in 0..4 {
            if i <= c.len() { out.push(B64[(n >> (18 - 6 * i) & 63) as usize] as char); } else { out.push('='); }
        }
    }
    out
}

/// Décode du base64 standard ; accepte un préfixe `data:…;base64,` et ignore les espaces.
pub fn b64_decode(s: &str) -> Result<Vec<u8>, String> {
    let s = s.split_once("base64,").map_or(s, |(_, rest)| rest);
    let mut out = Vec::with_capacity(s.len() / 4 * 3);
    let (mut acc, mut bits) = (0u32, 0);
    for ch in s.bytes().filter(|b| !b.is_ascii_whitespace() && *b != b'=') {
        let v = B64.iter().position(|&b| b == ch).ok_or("base64 invalide")? as u32;
        acc = acc << 6 | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits & 0xff) as u8);
        }
    }
    Ok(out)
}

// ---------- conversions ----------

fn tab_str(t: DocumentTab) -> &'static str {
    match t { DocumentTab::Todo => "todo", DocumentTab::Waiting => "waiting", DocumentTab::Signed => "signed" }
}

fn parse_tab(s: &str) -> DocumentTab {
    match s { "waiting" => DocumentTab::Waiting, "signed" => DocumentTab::Signed, _ => DocumentTab::Todo }
}

fn level_str(l: SignatureLevel) -> &'static str {
    match l { SignatureLevel::Simple => "simple", SignatureLevel::Advanced => "advanced", SignatureLevel::Qualified => "qualified" }
}

fn parse_level(s: &str) -> SignatureLevel {
    match s { "advanced" => SignatureLevel::Advanced, "qualified" => SignatureLevel::Qualified, _ => SignatureLevel::Simple }
}

/// Nom de fichier sûr : dernier composant du chemin, sans caractères de contrôle ni séparateurs.
fn safe_name(name: &str) -> String {
    let base = name.rsplit(['/', '\\']).next().unwrap_or("");
    let clean: String = base.chars().map(|c| if c.is_control() || c == ':' { '_' } else { c }).collect();
    let clean = clean.trim().trim_start_matches('.').to_string();
    if clean.is_empty() { "document.pdf".into() } else { clean }
}

// ---------- accès base ----------

const COLS: &str = "id, name, path, sha256, status, signers_json, level, origin_message_id, created";

fn read_row(r: &rusqlite::Row) -> rusqlite::Result<Document> {
    let signers: String = r.get::<_, Option<String>>(5)?.unwrap_or_default();
    Ok(Document {
        id: r.get(0)?,
        name: r.get::<_, Option<String>>(1)?.unwrap_or_default(),
        path: r.get::<_, Option<String>>(2)?.unwrap_or_default(),
        sha256: r.get::<_, Option<String>>(3)?.unwrap_or_default(),
        status: parse_tab(&r.get::<_, Option<String>>(4)?.unwrap_or_default()),
        signers: serde_json::from_str(&signers).unwrap_or_default(),
        level: parse_level(&r.get::<_, Option<String>>(6)?.unwrap_or_default()),
        origin_message_id: r.get(7)?,
        created: r.get::<_, Option<i64>>(8)?.unwrap_or(0),
        signable: true,
    })
}

fn all_names(conn: &Connection) -> Result<Vec<String>, String> {
    let mut st = conn.prepare("SELECT name FROM document").map_err(e)?;
    let rows = st.query_map([], |r| r.get::<_, Option<String>>(0)).map_err(e)?;
    Ok(rows.filter_map(|r| r.ok().flatten()).collect())
}

/// Un document est non signable dès qu'un autre document porte une version plus récente du même nom.
fn with_signable(mut d: Document, names: &[String]) -> Document {
    d.signable = !names.iter().any(|n| supersedes(&d.name, n));
    d
}

pub fn get(conn: &Connection, id: i64) -> Result<Document, String> {
    let d = conn
        .query_row(&format!("SELECT {COLS} FROM document WHERE id=?1"), [id], read_row)
        .optional().map_err(e)?
        .ok_or("document introuvable")?;
    Ok(with_signable(d, &all_names(conn)?))
}

pub fn list(conn: &Connection, tab: DocumentTab) -> Result<Vec<Document>, String> {
    let names = all_names(conn)?;
    let mut st = conn
        .prepare(&format!("SELECT {COLS} FROM document WHERE status=?1 ORDER BY created DESC, id DESC"))
        .map_err(e)?;
    let rows = st.query_map([tab_str(tab)], read_row).map_err(e)?;
    rows.map(|r| r.map(|d| with_signable(d, &names)).map_err(e)).collect()
}

fn log_event(conn: &Connection, id: i64, at: i64, kind: &str, detail: &str) -> Result<(), String> {
    conn.execute("INSERT INTO proof_event(document_id, at, kind, detail) VALUES(?1,?2,?3,?4)", params![id, at, kind, detail])
        .map_err(e)?;
    Ok(())
}

fn events(conn: &Connection, id: i64) -> Result<Vec<ProofEvent>, String> {
    let mut st = conn
        .prepare("SELECT id, at, kind, detail FROM proof_event WHERE document_id=?1 ORDER BY id")
        .map_err(e)?;
    let rows = st.query_map([id], |r| {
        Ok(ProofEvent {
            id: r.get(0)?,
            at: r.get::<_, Option<i64>>(1)?.unwrap_or(0),
            kind: r.get::<_, Option<String>>(2)?.unwrap_or_default(),
            detail: r.get::<_, Option<String>>(3)?.unwrap_or_default(),
        })
    }).map_err(e)?;
    rows.map(|r| r.map_err(e)).collect()
}

pub fn proof(conn: &Connection, id: i64) -> Result<Proof, String> {
    Ok(Proof { document: get(conn, id)?, events: events(conn, id)? })
}

fn save_signers(conn: &Connection, id: i64, signers: &[Signer]) -> Result<(), String> {
    let json = serde_json::to_string(signers).map_err(e)?;
    conn.execute("UPDATE document SET signers_json=?1 WHERE id=?2", params![json, id]).map_err(e)?;
    Ok(())
}

// ---------- import ----------

fn existing_for_origin(conn: &Connection, origin: &str, name: &str) -> Result<Option<i64>, String> {
    conn.query_row("SELECT id FROM document WHERE origin_message_id=?1 AND name=?2", params![origin, name], |r| r.get(0))
        .optional().map_err(e)
}

fn is_pdf_bytes(b: &[u8]) -> bool { b.windows(5).take(1024).any(|w| w == b"%PDF-") }

/// Contenu PDF prêt à signer : le PDF tel quel, ou le résultat de la conversion LibreOffice.
fn pdf_content(dir: &Path, stored: &Path, bytes: &[u8]) -> Result<(PathBuf, Vec<u8>), String> {
    if is_pdf_bytes(bytes) {
        lopdf::Document::load_mem(bytes).map_err(|x| format!("PDF illisible : {x}"))?;
        return Ok((stored.to_path_buf(), bytes.to_vec()));
    }
    fs::create_dir_all(dir).map_err(e)?;
    fs::write(stored, bytes).map_err(e)?;
    let pdf = signing::convert_to_pdf(stored).inspect_err(|_| { let _ = fs::remove_file(stored); })?;
    let converted = fs::read(&pdf).map_err(e)?;
    if stored != pdf { let _ = fs::remove_file(stored); }
    Ok((pdf, converted))
}

/// Importe un document : refuse les types non signables, convertit Word/ODT en PDF, enregistre
/// la ligne `document` et l'événement « opened ». Idempotent pour (message d'origine, nom).
pub fn import_bytes(conn: &Connection, dir: &Path, name: &str, bytes: &[u8], origin: Option<&str>, now: i64) -> Result<Document, String> {
    let name = safe_name(name);
    if !signing::is_signable("", &name) && !is_pdf_bytes(bytes) {
        return Err(format!("« {name} » n'est pas signable : seuls les PDF et documents Word/ODT le sont"));
    }
    if let Some(id) = origin.map(|o| existing_for_origin(conn, o, &name)).transpose()?.flatten() {
        return get(conn, id);
    }
    conn.execute(
        "INSERT INTO document(name, path, sha256, status, signers_json, level, origin_message_id, created) VALUES(?1,'','','todo','[]','simple',?2,?3)",
        params![name, origin, now],
    ).map_err(e)?;
    let id = conn.last_insert_rowid();
    let stored = dir.join(format!("{id}_{name}"));
    match pdf_content(dir, &stored, bytes) {
        Ok((path, content)) => finish_import(conn, dir, id, &path, &content, now)?,
        Err(err) => {
            conn.execute("DELETE FROM document WHERE id=?1", [id]).map_err(e)?;
            return Err(err);
        }
    }
    get(conn, id)
}

fn finish_import(conn: &Connection, dir: &Path, id: i64, path: &Path, content: &[u8], now: i64) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(e)?;
    fs::write(path, content).map_err(e)?;
    let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let shown = name.strip_prefix(&format!("{id}_")).unwrap_or(&name).to_string();
    conn.execute(
        "UPDATE document SET name=?1, path=?2, sha256=?3 WHERE id=?4",
        params![shown, path.to_string_lossy(), signing::sha256_hex(content), id],
    ).map_err(e)?;
    log_event(conn, id, now, "opened", &format!("Document « {shown} » ajouté"))
}

pub fn import_file(conn: &Connection, dir: &Path, src: &Path, origin: Option<&str>, now: i64) -> Result<Document, String> {
    let bytes = fs::read(src).map_err(|x| format!("lecture de {} impossible : {x}", src.display()))?;
    let name = src.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    import_bytes(conn, dir, &name, &bytes, origin, now)
}

// ---------- signature ----------

fn check_signable(d: &Document) -> Result<(), String> {
    if d.level != SignatureLevel::Simple {
        return Err(format!(
            "niveau de signature « {} » indisponible : il nécessite un prestataire de signature, seule la signature simple est prise en charge",
            level_str(d.level)
        ));
    }
    if !d.signable {
        return Err(format!("« {} » est remplacé par une version plus récente et n'est plus signable", d.name));
    }
    if d.status != DocumentTab::Todo {
        return Err(format!("« {} » est déjà signé par vous", d.name));
    }
    Ok(())
}

fn proof_events(conn: &Connection, id: i64) -> Result<Vec<signing::ProofEvent>, String> {
    Ok(events(conn, id)?.iter().map(|x| signing::ProofEvent::new(x.at, &x.kind, &x.detail)).collect())
}

/// Appose la signature (`page` 0-based, `rect` en points PDF), scelle le PDF et journalise.
pub fn apply(conn: &Connection, dir: &Path, id: i64, page: u32, rect: [f32; 4], png_b64: &str, now: i64) -> Result<Document, String> {
    let doc = get(conn, id)?;
    check_signable(&doc)?;
    let png = b64_decode(png_b64)?;
    let pdf = fs::read(&doc.path).map_err(|x| format!("lecture du document impossible : {x}"))?;
    let placed = signing::place_signature(&pdf, page + 1, rect, &png)?;
    log_event(conn, id, now, "signed", &format!("Signature apposée page {}", page + 1))?;
    let Sealed { pdf: sealed, sha256 } = signing::seal(&placed, &proof_events(conn, id)?)?;
    let out = dir.join(format!("{id}_signe_{}", doc.name));
    fs::write(&out, &sealed).map_err(e)?;
    log_event(conn, id, now, "sealed", &format!("Document scellé, empreinte SHA-256 {sha256}"))?;
    let status = if doc.signers.is_empty() { DocumentTab::Signed } else { DocumentTab::Waiting };
    conn.execute(
        "UPDATE document SET path=?1, sha256=?2, status=?3 WHERE id=?4",
        params![out.to_string_lossy(), sha256, tab_str(status), id],
    ).map_err(e)?;
    get(conn, id)
}

/// Chemin du PDF scellé à joindre à la réponse ; journalise l'envoi.
pub fn export_for_reply(conn: &Connection, id: i64, now: i64) -> Result<String, String> {
    let doc = get(conn, id)?;
    if doc.status == DocumentTab::Todo {
        return Err(format!("« {} » n'est pas encore signé", doc.name));
    }
    if !Path::new(&doc.path).is_file() {
        return Err("fichier du document introuvable".into());
    }
    log_event(conn, id, now, "sent", "Document signé exporté pour réponse")?;
    Ok(doc.path)
}

// ---------- signataires et relances ----------

pub fn signer_add(conn: &Connection, id: i64, name: &str, email: &str) -> Result<Document, String> {
    let email = email.trim().to_lowercase();
    if !email.contains('@') || email.starts_with('@') || email.ends_with('@') {
        return Err(format!("adresse e-mail invalide : {email}"));
    }
    let mut signers = get(conn, id)?.signers;
    if signers.iter().any(|s| s.email.eq_ignore_ascii_case(&email)) {
        return Err(format!("{email} est déjà signataire"));
    }
    signers.push(Signer { name: name.trim().to_string(), email, signed: false });
    save_signers(conn, id, &signers)?;
    get(conn, id)
}

pub fn signer_remove(conn: &Connection, id: i64, email: &str) -> Result<Document, String> {
    let mut signers = get(conn, id)?.signers;
    let before = signers.len();
    signers.retain(|s| !s.email.eq_ignore_ascii_case(email.trim()));
    if signers.len() == before {
        return Err(format!("{email} n'est pas signataire de ce document"));
    }
    save_signers(conn, id, &signers)?;
    get(conn, id)
}

/// Brouillon de relance pour les signataires en attente, avec le document en pièce jointe.
/// Journalise « reminded ».
pub fn reminder_draft(conn: &Connection, id: i64, now: i64) -> Result<Draft, String> {
    let doc = get(conn, id)?;
    if doc.status != DocumentTab::Waiting {
        return Err("seul un document que vous avez signé et qui attend d'autres signatures peut être relancé".into());
    }
    let pending: Vec<&Signer> = doc.signers.iter().filter(|s| !s.signed).collect();
    if pending.is_empty() {
        return Err("aucun signataire en attente".into());
    }
    let to = pending.iter().map(|s| s.email.as_str()).collect::<Vec<_>>().join(", ");
    log_event(conn, id, now, "reminded", &format!("Relance envoyée à {to}"))?;
    Ok(Draft {
        to,
        cc: None,
        subject: format!("Relance : signature de « {} »", doc.name),
        body: format!("Bonjour,\n\nPourriez-vous signer le document « {} » joint à ce message ?\n\nMerci d'avance.", doc.name),
        in_reply_to: doc.origin_message_id,
        attachments: vec![doc.path],
        remind_days: None,
        send_at: None,
    })
}

// ---------- image de signature ----------

pub fn image_save(dir: &Path, png_b64: &str) -> Result<(), String> {
    let png = b64_decode(png_b64)?;
    if !png.starts_with(PNG_MAGIC) {
        return Err("l'image de signature doit être un PNG".into());
    }
    fs::create_dir_all(dir).map_err(e)?;
    fs::write(dir.join(SIGNATURE_FILE), png).map_err(e)
}

pub fn image_get(dir: &Path) -> Result<Option<String>, String> {
    match fs::read(dir.join(SIGNATURE_FILE)) {
        Ok(b) => Ok(Some(b64_encode(&b))),
        Err(x) if x.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(x) => Err(e(x)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lopdf::{dictionary, Object, Stream};

    fn test_pdf() -> Vec<u8> {
        let mut doc = lopdf::Document::with_version("1.5");
        let pages = doc.new_object_id();
        let content = doc.add_object(Stream::new(dictionary! {}, b"BT ET".to_vec()));
        let page = doc.add_object(dictionary! {
            "Type" => "Page", "Parent" => pages, "Contents" => content,
            "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
        });
        doc.objects.insert(pages, Object::Dictionary(dictionary! {
            "Type" => "Pages", "Kids" => vec![page.into()], "Count" => 1,
        }));
        let catalog = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages });
        doc.trailer.set("Root", catalog);
        let mut out = Vec::new();
        doc.save_to(&mut out).unwrap();
        out
    }

    /// PNG RGBA 16x16 valide, généré en mémoire (zlib en blocs « stored »).
    fn test_png() -> Vec<u8> {
        fn crc(data: &[u8]) -> u32 {
            let mut c = !0u32;
            for &b in data {
                c ^= b as u32;
                for _ in 0..8 { c = if c & 1 == 1 { 0xEDB88320 ^ (c >> 1) } else { c >> 1 }; }
            }
            !c
        }
        fn chunk(out: &mut Vec<u8>, kind: &[u8], data: &[u8]) {
            out.extend((data.len() as u32).to_be_bytes());
            let mut body = kind.to_vec();
            body.extend(data);
            out.extend(&body);
            out.extend(crc(&body).to_be_bytes());
        }
        let raw: Vec<u8> = (0..16).flat_map(|_| std::iter::once(0u8).chain((0..16).flat_map(|_| [0u8, 0, 0, 255]))).collect();
        let (mut a, mut b) = (1u32, 0u32);
        for &x in &raw { a = (a + x as u32) % 65521; b = (b + a) % 65521; }
        let mut z = vec![0x78, 0x01, 1];
        z.extend((raw.len() as u16).to_le_bytes());
        z.extend((!(raw.len() as u16)).to_le_bytes());
        z.extend(&raw);
        z.extend(((b << 16) | a).to_be_bytes());
        let mut png = PNG_MAGIC.to_vec();
        let mut ihdr = Vec::new();
        ihdr.extend(16u32.to_be_bytes());
        ihdr.extend(16u32.to_be_bytes());
        ihdr.extend([8, 6, 0, 0, 0]);
        chunk(&mut png, b"IHDR", &ihdr);
        chunk(&mut png, b"IDAT", &z);
        chunk(&mut png, b"IEND", &[]);
        png
    }

    fn setup() -> (Connection, tempdir::Dir) {
        (crate::db::open_in_memory().unwrap(), tempdir::Dir::new())
    }

    /// Dossier temporaire minimal, supprimé à la fin du test.
    mod tempdir {
        use std::path::{Path, PathBuf};
        use std::sync::atomic::{AtomicU32, Ordering};
        static N: AtomicU32 = AtomicU32::new(0);
        pub struct Dir(PathBuf);
        impl Dir {
            pub fn new() -> Self {
                let base = std::env::var_os("TMPDIR").map(PathBuf::from).unwrap_or_else(std::env::temp_dir);
                let p = base.join(format!("iris-sign-{}-{}", std::process::id(), N.fetch_add(1, Ordering::SeqCst)));
                std::fs::create_dir_all(&p).unwrap();
                Dir(p)
            }
            pub fn path(&self) -> &Path { &self.0 }
        }
        impl Drop for Dir { fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); } }
    }

    fn import(conn: &Connection, dir: &Path, name: &str) -> Document {
        import_bytes(conn, dir, name, &test_pdf(), None, 100).unwrap()
    }

    #[test]
    fn base64_roundtrip() {
        for data in [&b""[..], b"a", b"ab", b"abc", b"abcd", &[0, 255, 128, 7, 9]] {
            assert_eq!(b64_decode(&b64_encode(data)).unwrap(), data);
        }
        assert_eq!(b64_encode(b"Man"), "TWFu");
        assert_eq!(b64_decode("data:image/png;base64,TWE=").unwrap(), b"Ma");
        assert!(b64_decode("@@").is_err());
    }

    #[test]
    fn import_lists_and_logs_opened() {
        let (conn, dir) = setup();
        let d = import(&conn, dir.path(), "contrat.pdf");
        assert_eq!((d.status, d.level, d.signable), (DocumentTab::Todo, SignatureLevel::Simple, true));
        assert!(Path::new(&d.path).is_file());
        assert_eq!(d.sha256.len(), 64);
        assert_eq!(list(&conn, DocumentTab::Todo).unwrap().len(), 1);
        assert!(list(&conn, DocumentTab::Signed).unwrap().is_empty());
        assert_eq!(proof(&conn, d.id).unwrap().events[0].kind, "opened");
        assert!(get(&conn, 999).is_err());
    }

    #[test]
    fn import_rejects_unsignable_and_broken() {
        let (conn, dir) = setup();
        assert!(import_bytes(&conn, dir.path(), "photo.png", b"xx", None, 1).unwrap_err().contains("n'est pas signable"));
        assert!(import_bytes(&conn, dir.path(), "faux.pdf", b"%PDF-1.4 garbage", None, 1).unwrap_err().contains("illisible"));
        assert!(list(&conn, DocumentTab::Todo).unwrap().is_empty());
    }

    #[test]
    fn import_from_message_is_idempotent_and_sanitised() {
        let (conn, dir) = setup();
        let a = import_bytes(&conn, dir.path(), "../../etc/devis.pdf", &test_pdf(), Some("<m1@x>"), 1).unwrap();
        let b = import_bytes(&conn, dir.path(), "devis.pdf", &test_pdf(), Some("<m1@x>"), 2).unwrap();
        assert_eq!((a.id, a.name.as_str()), (b.id, "devis.pdf"));
        assert!(Path::new(&a.path).starts_with(dir.path()));
    }

    #[test]
    fn word_without_conversion_is_rejected_cleanly() {
        let (conn, dir) = setup();
        if signing::soffice_available() { return; }
        let err = import_bytes(&conn, dir.path(), "lettre.docx", b"PK fake", None, 1).unwrap_err();
        assert!(err.contains("soffice"), "{err}");
        assert!(list(&conn, DocumentTab::Todo).unwrap().is_empty());
    }

    #[test]
    fn sign_seals_and_moves_tab() {
        let (conn, dir) = setup();
        let d = import(&conn, dir.path(), "contrat.pdf");
        let before = d.sha256.clone();
        let s = apply(&conn, dir.path(), d.id, 0, [100.0, 100.0, 200.0, 150.0], &b64_encode(&test_png()), 200).unwrap();
        assert_eq!(s.status, DocumentTab::Signed);
        assert_ne!(s.sha256, before);
        let sealed = fs::read(&s.path).unwrap();
        let reloaded = lopdf::Document::load_mem(&sealed).unwrap();
        assert_eq!(reloaded.get_pages().len(), 2, "page d'origine + journal de preuve");
        let kinds: Vec<_> = proof(&conn, d.id).unwrap().events.into_iter().map(|x| x.kind).collect();
        assert_eq!(kinds, ["opened", "signed", "sealed"]);
        assert_eq!(list(&conn, DocumentTab::Signed).unwrap().len(), 1);
        assert!(apply(&conn, dir.path(), d.id, 0, [0.0, 0.0, 10.0, 10.0], &b64_encode(&test_png()), 300).unwrap_err().contains("déjà signé"));
    }

    #[test]
    fn sign_with_pending_signers_goes_waiting_and_can_be_reminded() {
        let (conn, dir) = setup();
        let d = import(&conn, dir.path(), "contrat.pdf");
        assert!(reminder_draft(&conn, d.id, 1).is_err());
        signer_add(&conn, d.id, "Alice", "Alice@Example.com").unwrap();
        assert!(signer_add(&conn, d.id, "Alice", "alice@example.com").unwrap_err().contains("déjà"));
        assert!(signer_add(&conn, d.id, "X", "pas-un-mail").is_err());
        let s = apply(&conn, dir.path(), d.id, 0, [10.0, 10.0, 90.0, 40.0], &b64_encode(&test_png()), 200).unwrap();
        assert_eq!(s.status, DocumentTab::Waiting);
        let draft = reminder_draft(&conn, d.id, 300).unwrap();
        assert_eq!((draft.to.as_str(), draft.attachments.len()), ("alice@example.com", 1));
        assert!(draft.subject.contains("contrat.pdf"));
        assert_eq!(proof(&conn, d.id).unwrap().events.last().unwrap().kind, "reminded");
        let s = signer_remove(&conn, d.id, "ALICE@example.com").unwrap();
        assert!(s.signers.is_empty());
        assert!(signer_remove(&conn, d.id, "alice@example.com").is_err());
    }

    #[test]
    fn non_simple_level_is_rejected() {
        let (conn, dir) = setup();
        let d = import(&conn, dir.path(), "contrat.pdf");
        for level in ["advanced", "qualified"] {
            conn.execute("UPDATE document SET level=?1 WHERE id=?2", params![level, d.id]).unwrap();
            let err = apply(&conn, dir.path(), d.id, 0, [0.0, 0.0, 10.0, 10.0], &b64_encode(&test_png()), 1).unwrap_err();
            assert!(err.contains(level) && err.contains("prestataire"), "{err}");
        }
        assert_eq!(get(&conn, d.id).unwrap().status, DocumentTab::Todo);
    }

    #[test]
    fn superseded_document_is_not_signable() {
        let (conn, dir) = setup();
        let v2 = import(&conn, dir.path(), "contrat_v2.pdf");
        assert!(v2.signable);
        let v3 = import(&conn, dir.path(), "contrat_v3.pdf");
        assert!(!get(&conn, v2.id).unwrap().signable);
        assert!(get(&conn, v3.id).unwrap().signable);
        assert!(!list(&conn, DocumentTab::Todo).unwrap().iter().find(|d| d.id == v2.id).unwrap().signable);
        let err = apply(&conn, dir.path(), v2.id, 0, [0.0, 0.0, 10.0, 10.0], &b64_encode(&test_png()), 1).unwrap_err();
        assert!(err.contains("remplacé"), "{err}");
    }

    #[test]
    fn bad_signature_or_page_is_reported_without_state_change() {
        let (conn, dir) = setup();
        let d = import(&conn, dir.path(), "contrat.pdf");
        assert!(apply(&conn, dir.path(), d.id, 0, [0.0, 0.0, 10.0, 10.0], "###", 1).is_err());
        assert!(apply(&conn, dir.path(), d.id, 7, [0.0, 0.0, 10.0, 10.0], &b64_encode(&test_png()), 1).is_err());
        assert_eq!(get(&conn, d.id).unwrap().status, DocumentTab::Todo);
        assert_eq!(proof(&conn, d.id).unwrap().events.len(), 1);
    }

    #[test]
    fn export_requires_signature_and_logs_sent() {
        let (conn, dir) = setup();
        let d = import(&conn, dir.path(), "contrat.pdf");
        assert!(export_for_reply(&conn, d.id, 1).unwrap_err().contains("pas encore signé"));
        apply(&conn, dir.path(), d.id, 0, [10.0, 10.0, 90.0, 40.0], &b64_encode(&test_png()), 2).unwrap();
        let path = export_for_reply(&conn, d.id, 3).unwrap();
        assert!(Path::new(&path).is_file());
        assert_eq!(proof(&conn, d.id).unwrap().events.last().unwrap().kind, "sent");
    }

    #[test]
    fn signature_image_roundtrip() {
        let dir = tempdir::Dir::new();
        assert_eq!(image_get(dir.path()).unwrap(), None);
        assert!(image_save(dir.path(), &b64_encode(b"pas un png")).is_err());
        let b64 = b64_encode(&test_png());
        image_save(&dir.path().join("sous/dossier"), &b64).unwrap();
        assert_eq!(image_get(&dir.path().join("sous/dossier")).unwrap(), Some(b64));
    }
}
