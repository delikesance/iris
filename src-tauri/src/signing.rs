//! Signature simple de PDF : placement d'une image, scellement, journal de preuve.
//!
//! Conventions : `page` est 1-based ; `rect` = `[x0, y0, x1, y1]` en points PDF
//! (origine en bas à gauche de la page).

use lopdf::content::{Content, Operation};
use lopdf::{dictionary, Dictionary, Document, Object, ObjectId, Stream};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProofEvent {
    /// Horodatage Unix (secondes).
    pub at: i64,
    /// created | sent | viewed | signed | reminded | sealed | …
    pub kind: String,
    pub detail: String,
}

impl ProofEvent {
    pub fn new(at: i64, kind: &str, detail: &str) -> Self {
        Self { at, kind: kind.into(), detail: detail.into() }
    }
}

#[derive(Debug, Clone)]
pub struct Sealed {
    pub pdf: Vec<u8>,
    /// SHA-256 (hex) du PDF signé avant ajout de la page de preuve.
    pub sha256: String,
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

// ---------- classification ----------

const WORD_MIMES: [&str; 3] = [
    "application/msword",
    "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
    "application/vnd.oasis.opendocument.text",
];

fn ext(filename: &str) -> String {
    Path::new(filename)
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default()
}

fn is_generic(mime: &str) -> bool {
    mime.is_empty() || mime.eq_ignore_ascii_case("application/octet-stream")
}

fn is_pdf(mime: &str, filename: &str) -> bool {
    mime.eq_ignore_ascii_case("application/pdf") || (is_generic(mime) && ext(filename) == "pdf")
}

/// Un document est signable s'il est PDF ou convertible en PDF (Word/ODT).
pub fn is_signable(mime: &str, filename: &str) -> bool {
    if is_pdf(mime, filename) || needs_conversion(mime) {
        return true;
    }
    is_generic(mime) && matches!(ext(filename).as_str(), "doc" | "docx" | "odt")
}

pub fn needs_conversion(mime: &str) -> bool {
    WORD_MIMES.iter().any(|m| mime.eq_ignore_ascii_case(m))
}

pub fn soffice_available() -> bool {
    Command::new("soffice")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Convertit un document Office en PDF (dans le même dossier) via LibreOffice.
pub fn convert_to_pdf(path: &Path) -> Result<PathBuf, String> {
    if !path.is_file() {
        return Err(format!("fichier introuvable : {}", path.display()));
    }
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let out = Command::new("soffice")
        .args(["--headless", "--convert-to", "pdf", "--outdir"])
        .arg(dir)
        .arg(path)
        .output()
        .map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                "LibreOffice (soffice) n'est pas installé : impossible de convertir ce document en PDF".to_string()
            } else {
                format!("échec de soffice : {e}")
            }
        })?;
    if !out.status.success() {
        return Err(format!("conversion échouée : {}", String::from_utf8_lossy(&out.stderr)));
    }
    let pdf = path.with_extension("pdf");
    if pdf.is_file() {
        Ok(pdf)
    } else {
        Err("conversion terminée mais PDF introuvable".into())
    }
}

// ---------- PNG -> image XObject ----------

struct Png {
    w: u32,
    h: u32,
    rgb: Vec<u8>,
    alpha: Option<Vec<u8>>,
}

fn decode_png(data: &[u8]) -> Result<Png, String> {
    const SIG: [u8; 8] = [137, 80, 78, 71, 13, 10, 26, 10];
    if data.len() < 8 || data[..8] != SIG {
        return Err("image de signature : PNG invalide".into());
    }
    let (mut w, mut h, mut depth, mut ctype, mut interlace) = (0u32, 0u32, 0u8, 0u8, 0u8);
    let mut idat = Vec::new();
    let mut p = 8;
    while p + 8 <= data.len() {
        let len = u32::from_be_bytes(data[p..p + 4].try_into().unwrap()) as usize;
        let kind = &data[p + 4..p + 8];
        let body = data.get(p + 8..p + 8 + len).ok_or("PNG tronqué")?;
        match kind {
            b"IHDR" if len >= 13 => {
                w = u32::from_be_bytes(body[0..4].try_into().unwrap());
                h = u32::from_be_bytes(body[4..8].try_into().unwrap());
                depth = body[8];
                ctype = body[9];
                interlace = body[12];
            }
            b"IDAT" => idat.extend_from_slice(body),
            b"IEND" => break,
            _ => {}
        }
        p += 12 + len;
    }
    if w == 0 || h == 0 || idat.is_empty() {
        return Err("PNG sans données".into());
    }
    if depth != 8 || interlace != 0 || !matches!(ctype, 0 | 2 | 4 | 6) {
        return Err("PNG non supporté (8 bits, non entrelacé, sans palette requis)".into());
    }
    let ch = match ctype {
        0 => 1,
        2 => 3,
        4 => 2,
        _ => 4,
    };
    let raw = Stream::new(dictionary! { "Filter" => "FlateDecode" }, idat)
        .decompressed_content()
        .map_err(|e| format!("PNG illisible : {e}"))?;
    let stride = w as usize * ch;
    if raw.len() < (stride + 1) * h as usize {
        return Err("PNG tronqué".into());
    }
    let mut img = vec![0u8; stride * h as usize];
    for y in 0..h as usize {
        let f = raw[y * (stride + 1)];
        let line = &raw[y * (stride + 1) + 1..(y + 1) * (stride + 1)];
        for x in 0..stride {
            let a = if x >= ch { img[y * stride + x - ch] } else { 0 };
            let b = if y > 0 { img[(y - 1) * stride + x] } else { 0 };
            let c = if x >= ch && y > 0 { img[(y - 1) * stride + x - ch] } else { 0 };
            let v = match f {
                0 => line[x],
                1 => line[x].wrapping_add(a),
                2 => line[x].wrapping_add(b),
                3 => line[x].wrapping_add(((a as u16 + b as u16) / 2) as u8),
                4 => {
                    let (ia, ib, ic) = (a as i32, b as i32, c as i32);
                    let pp = ia + ib - ic;
                    let (pa, pb, pc) = ((pp - ia).abs(), (pp - ib).abs(), (pp - ic).abs());
                    let pred = if pa <= pb && pa <= pc {
                        a
                    } else if pb <= pc {
                        b
                    } else {
                        c
                    };
                    line[x].wrapping_add(pred)
                }
                _ => return Err("PNG : filtre inconnu".into()),
            };
            img[y * stride + x] = v;
        }
    }
    let n = (w * h) as usize;
    let (mut rgb, mut alpha) = (Vec::with_capacity(n * 3), None);
    match ctype {
        0 => img.iter().for_each(|&g| rgb.extend_from_slice(&[g, g, g])),
        2 => rgb = img,
        4 => {
            let mut al = Vec::with_capacity(n);
            for px in img.chunks(2) {
                rgb.extend_from_slice(&[px[0], px[0], px[0]]);
                al.push(px[1]);
            }
            alpha = Some(al);
        }
        _ => {
            let mut al = Vec::with_capacity(n);
            for px in img.chunks(4) {
                rgb.extend_from_slice(&px[..3]);
                al.push(px[3]);
            }
            alpha = Some(al);
        }
    }
    Ok(Png { w, h, rgb, alpha })
}

fn flate_stream(dict: Dictionary, data: Vec<u8>) -> Stream {
    let mut s = Stream::new(dict, data);
    let _ = s.compress();
    s
}

// ---------- placement ----------

/// Résout (ou crée) le dictionnaire Resources de la page et y ajoute un XObject.
fn add_xobject(doc: &mut Document, page: ObjectId, name: &str, id: ObjectId) -> Result<(), String> {
    // Resources éventuellement héritées : on part d'une copie.
    let mut res = match doc.get_page_resources(page).map_err(|e| e.to_string())? {
        (Some(d), _) => d.clone(),
        (None, ids) => ids
            .first()
            .and_then(|i| doc.get_object(*i).ok())
            .and_then(|o| o.as_dict().ok())
            .cloned()
            .unwrap_or_default(),
    };
    let mut xo = match res.get(b"XObject") {
        Ok(Object::Dictionary(d)) => d.clone(),
        Ok(Object::Reference(r)) => doc
            .get_object(*r)
            .ok()
            .and_then(|o| o.as_dict().ok())
            .cloned()
            .unwrap_or_default(),
        _ => Dictionary::new(),
    };
    xo.set(name, Object::Reference(id));
    res.set("XObject", Object::Dictionary(xo));
    let pd = doc
        .get_object_mut(page)
        .and_then(|o| o.as_dict_mut())
        .map_err(|e| e.to_string())?;
    pd.set("Resources", Object::Dictionary(res));
    Ok(())
}

/// Place l'image PNG `png` dans `rect` de la page `page` (1-based).
pub fn place_signature(pdf: &[u8], page: u32, rect: [f32; 4], png: &[u8]) -> Result<Vec<u8>, String> {
    let mut doc = Document::load_mem(pdf).map_err(|e| format!("PDF illisible : {e}"))?;
    let page_id = *doc
        .get_pages()
        .get(&page)
        .ok_or_else(|| format!("page {page} inexistante"))?;
    let [x0, y0, x1, y1] = rect;
    let (w, h) = ((x1 - x0).abs(), (y1 - y0).abs());
    if w <= 0.0 || h <= 0.0 {
        return Err("zone de signature vide".into());
    }
    let img = decode_png(png)?;
    let mut d = dictionary! {
        "Type" => "XObject", "Subtype" => "Image",
        "Width" => img.w as i64, "Height" => img.h as i64,
        "ColorSpace" => "DeviceRGB", "BitsPerComponent" => 8,
    };
    if let Some(al) = img.alpha {
        let mask = doc.add_object(flate_stream(
            dictionary! {
                "Type" => "XObject", "Subtype" => "Image",
                "Width" => img.w as i64, "Height" => img.h as i64,
                "ColorSpace" => "DeviceGray", "BitsPerComponent" => 8,
            },
            al,
        ));
        d.set("SMask", Object::Reference(mask));
    }
    let img_id = doc.add_object(flate_stream(d, img.rgb));
    let name = format!("IrisSig{}", img_id.0);
    add_xobject(&mut doc, page_id, &name, img_id)?;
    let content = Content {
        operations: vec![
            Operation::new("q", vec![]),
            Operation::new(
                "cm",
                vec![w.into(), 0.into(), 0.into(), h.into(), x0.min(x1).into(), y0.min(y1).into()],
            ),
            Operation::new("Do", vec![Object::Name(name.into_bytes())]),
            Operation::new("Q", vec![]),
        ],
    };
    doc.add_page_contents(page_id, content.encode().map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    save(&mut doc)
}

fn save(doc: &mut Document) -> Result<Vec<u8>, String> {
    let mut out = Vec::new();
    doc.save_to(&mut out).map_err(|e| e.to_string())?;
    Ok(out)
}

// ---------- preuve ----------

pub fn proof_json(sha256: &str, events: &[ProofEvent]) -> String {
    serde_json::to_string_pretty(&serde_json::json!({
        "document_sha256": sha256,
        "events": events,
    }))
    .expect("json")
}

fn fmt_ts(at: i64) -> String {
    chrono::DateTime::from_timestamp(at, 0)
        .map(|d| d.format("%Y-%m-%d %H:%M:%S UTC").to_string())
        .unwrap_or_else(|| at.to_string())
}

/// Texte en WinAnsi (≈ Latin-1) pour les polices standard.
fn winansi(s: &str) -> Object {
    let bytes: Vec<u8> = s
        .chars()
        .map(|c| if (c as u32) < 256 { c as u8 } else { b'?' })
        .collect();
    Object::string_literal(bytes)
}

/// Ajoute une page « Journal de preuve » en fin de PDF.
pub fn append_proof_page(pdf: &[u8], sha256: &str, events: &[ProofEvent]) -> Result<Vec<u8>, String> {
    let mut doc = Document::load_mem(pdf).map_err(|e| format!("PDF illisible : {e}"))?;
    let pages_id = doc
        .catalog()
        .and_then(|c| c.get(b"Pages"))
        .and_then(|p| p.as_reference())
        .map_err(|e| e.to_string())?;
    let font = doc.add_object(dictionary! {
        "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica", "Encoding" => "WinAnsiEncoding",
    });
    let mut lines: Vec<(f32, String)> = vec![
        (16.0, "Journal de preuve".into()),
        (9.0, format!("SHA-256 du document : {sha256}")),
        (9.0, String::new()),
    ];
    for e in events {
        lines.push((10.0, format!("{}  {}  {}", fmt_ts(e.at), e.kind, e.detail)));
    }
    let mut ops = vec![Operation::new("BT", vec![]), Operation::new("Td", vec![50.into(), 790.into()])];
    for (size, text) in &lines {
        ops.push(Operation::new("Tf", vec!["F1".into(), (*size).into()]));
        ops.push(Operation::new("TL", vec![(size + 6.0).into()]));
        ops.push(Operation::new("Tj", vec![winansi(text)]));
        ops.push(Operation::new("T*", vec![]));
    }
    ops.push(Operation::new("ET", vec![]));
    let content = doc.add_object(Stream::new(
        Dictionary::new(),
        Content { operations: ops }.encode().map_err(|e| e.to_string())?,
    ));
    let page = doc.add_object(dictionary! {
        "Type" => "Page", "Parent" => pages_id,
        "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
        "Resources" => dictionary! { "Font" => dictionary! { "F1" => font } },
        "Contents" => content,
    });
    let pages = doc
        .get_object_mut(pages_id)
        .and_then(|o| o.as_dict_mut())
        .map_err(|e| e.to_string())?;
    let kids = pages
        .get_mut(b"Kids")
        .and_then(|k| k.as_array_mut())
        .map_err(|e| e.to_string())?;
    kids.push(Object::Reference(page));
    let count = kids.len() as i64;
    pages.set("Count", count);
    save(&mut doc)
}

/// Scelle : calcule l'empreinte du PDF signé puis ajoute la page de preuve.
pub fn seal(pdf: &[u8], events: &[ProofEvent]) -> Result<Sealed, String> {
    let sha256 = sha256_hex(pdf);
    let pdf = append_proof_page(pdf, &sha256, events)?;
    Ok(Sealed { pdf, sha256 })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_pdf() -> Vec<u8> {
        let mut doc = Document::with_version("1.5");
        let pages_id = doc.new_object_id();
        let content = doc.add_object(Stream::new(Dictionary::new(), b"BT /F1 12 Tf 72 700 Td (Contrat) Tj ET".to_vec()));
        let font = doc.add_object(dictionary! { "Type" => "Font", "Subtype" => "Type1", "BaseFont" => "Helvetica" });
        let page = doc.add_object(dictionary! {
            "Type" => "Page", "Parent" => pages_id,
            "MediaBox" => vec![0.into(), 0.into(), 595.into(), 842.into()],
            "Resources" => dictionary! { "Font" => dictionary! { "F1" => font } },
            "Contents" => content,
        });
        doc.objects.insert(
            pages_id,
            Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => vec![page.into()], "Count" => 1 }),
        );
        let cat = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
        doc.trailer.set("Root", cat);
        let mut out = Vec::new();
        doc.save_to(&mut out).unwrap();
        out
    }

    /// PNG 16x16 non filtré (assez grand pour que lopdf compresse) (ctype 6 = RGBA, 2 = RGB), IDAT compressé via lopdf.
    fn fixture_png(ctype: u8) -> Vec<u8> {
        let ch = if ctype == 6 { 4 } else { 3 };
        let mut raw = Vec::new();
        for _ in 0..16 {
            raw.push(0u8);
            for _ in 0..16 {
                raw.extend_from_slice(&[10, 20, 30, 255][..ch]);
            }
        }
        let mut s = Stream::new(Dictionary::new(), raw);
        s.compress().unwrap();
        let idat = s.content;
        let mut png = vec![137, 80, 78, 71, 13, 10, 26, 10];
        let mut chunk = |kind: &[u8; 4], body: &[u8]| {
            png.extend_from_slice(&(body.len() as u32).to_be_bytes());
            png.extend_from_slice(kind);
            png.extend_from_slice(body);
            png.extend_from_slice(&[0; 4]); // CRC non vérifié
        };
        let mut ihdr = Vec::new();
        ihdr.extend_from_slice(&16u32.to_be_bytes());
        ihdr.extend_from_slice(&16u32.to_be_bytes());
        ihdr.extend_from_slice(&[8, ctype, 0, 0, 0]);
        chunk(b"IHDR", &ihdr);
        chunk(b"IDAT", &idat);
        chunk(b"IEND", &[]);
        png
    }

    #[test]
    fn signable_classification() {
        assert!(is_signable("application/pdf", "a.pdf"));
        assert!(is_signable("application/octet-stream", "a.PDF"));
        assert!(is_signable(WORD_MIMES[1], "a.docx"));
        assert!(is_signable("application/octet-stream", "a.docx"));
        assert!(!is_signable("image/png", "a.png"));
        assert!(!is_signable("application/octet-stream", "a.zip"));
        assert!(needs_conversion(WORD_MIMES[0]));
        assert!(!needs_conversion("application/pdf"));
    }

    #[test]
    fn place_signature_rgba_adds_image_and_smask() {
        let out = place_signature(&fixture_pdf(), 1, [100.0, 100.0, 220.0, 150.0], &fixture_png(6)).unwrap();
        let doc = Document::load_mem(&out).unwrap();
        assert_eq!(doc.get_pages().len(), 1);
        let imgs: Vec<_> = doc
            .objects
            .values()
            .filter_map(|o| o.as_stream().ok())
            .filter(|s| s.dict.get(b"Subtype").and_then(|v| v.as_name()).map(|n| n == b"Image").unwrap_or(false))
            .collect();
        assert_eq!(imgs.len(), 2);
        assert!(imgs.iter().any(|s| s.dict.has(b"SMask")));
        let page = *doc.get_pages().get(&1).unwrap();
        let content = String::from_utf8_lossy(&doc.get_page_content(page).unwrap()).to_string();
        assert!(content.contains("Contrat"));
        assert!(content.contains(" cm") && content.contains(" Do"), "{content}");
    }

    #[test]
    fn place_signature_rgb_and_errors() {
        let pdf = fixture_pdf();
        assert!(place_signature(&pdf, 1, [0.0, 0.0, 50.0, 20.0], &fixture_png(2)).is_ok());
        assert!(place_signature(&pdf, 2, [0.0, 0.0, 50.0, 20.0], &fixture_png(2)).unwrap_err().contains("page 2"));
        assert!(place_signature(&pdf, 1, [0.0, 0.0, 0.0, 20.0], &fixture_png(2)).is_err());
        assert!(place_signature(&pdf, 1, [0.0, 0.0, 5.0, 5.0], b"nope").is_err());
        assert!(place_signature(b"not a pdf", 1, [0.0, 0.0, 5.0, 5.0], &fixture_png(2)).is_err());
    }

    #[test]
    fn seal_appends_proof_page_with_hash() {
        let signed = place_signature(&fixture_pdf(), 1, [10.0, 10.0, 60.0, 40.0], &fixture_png(6)).unwrap();
        let events = vec![
            ProofEvent::new(1_700_000_000, "created", "import"),
            ProofEvent::new(1_700_000_100, "signed", "Vous <moi@example.org>"),
        ];
        let sealed = seal(&signed, &events).unwrap();
        assert_eq!(sealed.sha256, sha256_hex(&signed));
        assert_eq!(sealed.sha256.len(), 64);
        let doc = Document::load_mem(&sealed.pdf).unwrap();
        assert_eq!(doc.get_pages().len(), 2);
        let text = doc.extract_text(&[2]).unwrap();
        assert!(text.contains("Journal de preuve"), "{text}");
        assert!(text.contains(&sealed.sha256[..16]), "{text}");
        assert!(text.contains("signed"));
        assert!(text.contains("2023-11-14"));
    }

    #[test]
    fn proof_json_export() {
        let ev = vec![ProofEvent::new(5, "signed", "ok")];
        let v: serde_json::Value = serde_json::from_str(&proof_json("abc", &ev)).unwrap();
        assert_eq!(v["document_sha256"], "abc");
        assert_eq!(v["events"][0]["kind"], "signed");
        assert_eq!(v["events"][0]["at"], 5);
    }

    #[test]
    fn convert_to_pdf_errors_cleanly() {
        let e = convert_to_pdf(Path::new("/nonexistent/x.docx")).unwrap_err();
        assert!(e.contains("introuvable"));
        if !soffice_available() {
            let dir = std::env::var("TMPDIR").unwrap_or_else(|_| ".".into());
            let f = Path::new(&dir).join("iris-sign-test.docx");
            std::fs::write(&f, b"x").unwrap();
            let e = convert_to_pdf(&f).unwrap_err();
            std::fs::remove_file(&f).ok();
            assert!(e.contains("soffice"), "{e}");
        }
    }
}
