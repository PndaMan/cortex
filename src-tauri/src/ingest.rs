//! Source ingestion: detect → parse → chunk → embed → store.
//! Parsers are real for txt/md/html (+ web fetch) and use the installed
//! LibreOffice headless converter for pdf/docx/pptx. audio/youtube return a
//! graceful placeholder (Whisper / yt-dlp wiring is a later slice).

use crate::embed::Embedder;
use crate::error::{Error, Result};
use crate::models::AddSourceInput;
use std::path::Path;

/// Detect the source kind from explicit input, then path/url extension.
pub fn detect_kind(input: &AddSourceInput) -> String {
    if let Some(k) = &input.kind {
        if !k.is_empty() {
            return k.clone();
        }
    }
    if input.url.is_some() {
        let u = input.url.as_deref().unwrap_or("");
        if u.contains("youtube.com") || u.contains("youtu.be") {
            return "yt".into();
        }
        return "web".into();
    }
    if let Some(p) = &input.path {
        let ext = Path::new(p)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        return match ext.as_str() {
            "pdf" => "pdf",
            "docx" | "doc" => "docx",
            "pptx" | "ppt" => "pptx",
            "md" | "markdown" => "md",
            "txt" | "text" | "" => "txt",
            "png" | "jpg" | "jpeg" | "webp" => "image",
            "m4a" | "mp3" | "wav" | "ogg" | "opus" => "audio",
            other => other,
        }
        .to_string();
    }
    "txt".into()
}

/// Extract plaintext for a source. Returns `(text, optional_warning)`.
pub fn parse(kind: &str, input: &AddSourceInput) -> Result<(String, Option<String>)> {
    match kind {
        "txt" | "md" => {
            if let Some(t) = &input.text {
                Ok((t.clone(), None))
            } else if let Some(p) = &input.path {
                Ok((std::fs::read_to_string(p)?, None))
            } else {
                Err(Error::Other("no text or path provided".into()))
            }
        }
        "web" => {
            let url = input
                .url
                .as_deref()
                .ok_or_else(|| Error::Other("web source needs a url".into()))?;
            let client = crate::commands::http_client(30);
            let html = client.get(url).send()?.text()?;
            Ok((html_to_text(&html), None))
        }
        "pdf" => {
            let p = input
                .path
                .as_deref()
                .ok_or_else(|| Error::Other("pdf source needs a file path".into()))?;
            pdf_to_text(p)
        }
        "docx" | "pptx" => {
            let p = input
                .path
                .as_deref()
                .ok_or_else(|| Error::Other(format!("{kind} source needs a file path")))?;
            libreoffice_to_text(p)
        }
        "audio" | "yt" | "image" => Ok((
            String::new(),
            Some(format!(
                "{kind} ingestion is stubbed in this build (Whisper / yt-dlp / OCR land in a later slice)"
            )),
        )),
        other => Err(Error::Unsupported(format!("unknown source kind: {other}"))),
    }
}

/// Strip HTML to readable text: drop script/style, remove tags, decode a few
/// common entities, collapse whitespace. Dependency-light on purpose.
pub fn html_to_text(html: &str) -> String {
    // The `regex` crate has no backreferences, so drop each non-content element
    // with its own pattern rather than a captured-group close tag.
    let mut stripped = html.to_string();
    for tag in ["script", "style", "head", "nav", "footer"] {
        let re = regex::Regex::new(&format!(r"(?is)<{tag}\b[^>]*>.*?</{tag}>")).unwrap();
        stripped = re.replace_all(&stripped, " ").into_owned();
    }
    let re_tag = regex::Regex::new(r"(?s)<[^>]+>").unwrap();
    let text = re_tag.replace_all(&stripped, " ").into_owned();
    let text = decode_entities(&text);
    // Drop zero-width / BOM characters that make extracted web text look garbled.
    let text: String = text
        .chars()
        .filter(|&c| !matches!(c, '\u{200b}' | '\u{200c}' | '\u{200d}' | '\u{feff}' | '\u{ad}'))
        .collect();
    let re_ws = regex::Regex::new(r"\s+").unwrap();
    re_ws.replace_all(text.trim(), " ").to_string()
}

/// Reduce a fetched HTML page to readable content for the in-app reader view:
/// `(title, body_text, links)` where links are `(absolute_href, anchor_text)`.
/// No JS executes — safe to render. Dependency-light (regex only).
pub fn readable_page(html: &str, base_url: &str) -> (String, String, Vec<(String, String)>) {
    // Title: <title>…</title>, falling back to the page host.
    let title = regex::Regex::new(r"(?is)<title[^>]*>(.*?)</title>")
        .unwrap()
        .captures(html)
        .map(|c| decode_entities(c[1].trim()).trim().to_string())
        .filter(|t| !t.is_empty())
        .unwrap_or_else(|| base_url.to_string());

    let text = html_to_text(html);

    // Outbound links: <a href="…">text</a>, resolved to absolute http(s) URLs.
    let re_a = regex::Regex::new(r#"(?is)<a\b[^>]*href\s*=\s*["']([^"']+)["'][^>]*>(.*?)</a>"#).unwrap();
    let mut links: Vec<(String, String)> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for caps in re_a.captures_iter(html) {
        let href_raw = caps[1].trim();
        if href_raw.is_empty()
            || href_raw.starts_with('#')
            || href_raw.starts_with("javascript:")
            || href_raw.starts_with("mailto:")
        {
            continue;
        }
        let abs = resolve_url(base_url, href_raw);
        if !abs.starts_with("http://") && !abs.starts_with("https://") {
            continue;
        }
        let label = html_to_text(&caps[2]);
        if label.is_empty() || !seen.insert(abs.clone()) {
            continue;
        }
        links.push((abs, label));
        if links.len() >= 120 {
            break;
        }
    }
    (title, text, links)
}

/// Resolve a possibly-relative href against a base URL (handles absolute,
/// scheme-relative `//`, root-relative `/path`, and simple relative paths).
fn resolve_url(base: &str, href: &str) -> String {
    if href.starts_with("http://") || href.starts_with("https://") {
        return href.to_string();
    }
    let scheme_end = base.find("://").map(|i| i + 3).unwrap_or(0);
    let after = &base[scheme_end..];
    let scheme = &base[..scheme_end]; // includes "://"
    let host = after.split(['/', '?', '#']).next().unwrap_or(after);
    if let Some(rest) = href.strip_prefix("//") {
        let s = if scheme.is_empty() { "https://" } else { scheme };
        return format!("{s}{rest}");
    }
    if let Some(path) = href.strip_prefix('/') {
        return format!("{scheme}{host}/{path}");
    }
    // relative to the current directory of the base path
    let base_no_query = base.split(['?', '#']).next().unwrap_or(base);
    let dir = base_no_query.rsplit_once('/').map(|(d, _)| d).unwrap_or(base_no_query);
    format!("{dir}/{href}")
}

/// Decode the common named entities plus numeric (`&#8203;`) and hex (`&#x27;`) refs.
fn decode_entities(s: &str) -> String {
    let named = s
        .replace("&nbsp;", " ")
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&mdash;", "—")
        .replace("&ndash;", "–")
        .replace("&hellip;", "…")
        .replace("&rsquo;", "’")
        .replace("&lsquo;", "‘")
        .replace("&ldquo;", "“")
        .replace("&rdquo;", "”");
    let re_num = regex::Regex::new(r"&#(x?[0-9A-Fa-f]+);").unwrap();
    re_num
        .replace_all(&named, |caps: &regex::Captures| {
            let raw = &caps[1];
            let code = if let Some(hex) = raw.strip_prefix('x').or_else(|| raw.strip_prefix('X')) {
                u32::from_str_radix(hex, 16).ok()
            } else {
                raw.parse::<u32>().ok()
            };
            code.and_then(char::from_u32).map(|c| c.to_string()).unwrap_or_default()
        })
        .into_owned()
}

/// Extract text from a PDF via `pdftotext -layout -enc UTF-8 <path> -` (poppler),
/// streaming output to stdout. PDFs are wrong for LibreOffice's txt converter
/// (it opens them in Draw and produces no .txt), so we use poppler instead.
///
/// This is forgiving on purpose: a scanned/image-only PDF has no extractable
/// text, but we still want it to ingest because the original bytes are copied
/// and remain previewable. So a missing `pdftotext` binary OR empty/whitespace
/// output returns `Ok((empty, Some(warning)))` rather than an error. Only a real
/// non-zero exit with stderr is a hard error.
fn pdf_to_text(path: &str) -> Result<(String, Option<String>)> {
    use std::process::Command;
    let src = Path::new(path);
    if !src.exists() {
        return Err(Error::NotFound(format!("file not found: {path}")));
    }

    if which("pdftotext").is_none() {
        return Ok((
            String::new(),
            Some("warning: no extractable text (scanned PDF?); preview still available".into()),
        ));
    }

    let output = Command::new("pdftotext")
        .args(["-layout", "-enc", "UTF-8"])
        .arg(src)
        .arg("-")
        .output();

    match output {
        Ok(o) if o.status.success() => {
            let text = String::from_utf8_lossy(&o.stdout).into_owned();
            if text.trim().is_empty() {
                Ok((
                    String::new(),
                    Some(
                        "warning: no extractable text (scanned PDF?); preview still available"
                            .into(),
                    ),
                ))
            } else {
                Ok((text, None))
            }
        }
        Ok(o) => {
            let stderr = String::from_utf8_lossy(&o.stderr).trim().to_string();
            if stderr.is_empty() {
                // non-zero exit but nothing to report → degrade gracefully
                Ok((
                    String::new(),
                    Some(
                        "warning: no extractable text (scanned PDF?); preview still available"
                            .into(),
                    ),
                ))
            } else {
                Err(Error::Other(format!(
                    "pdftotext failed for {path}: {stderr}"
                )))
            }
        }
        // binary vanished between the which() check and exec → degrade gracefully
        Err(_) => Ok((
            String::new(),
            Some("warning: no extractable text (scanned PDF?); preview still available".into()),
        )),
    }
}

/// Convert docx/pptx to text via `libreoffice --headless --convert-to txt`.
/// Unlike before, a missing binary or failed conversion is a hard `Err` (with
/// actionable detail) rather than an `Ok(empty)` that silently makes an empty draft.
fn libreoffice_to_text(path: &str) -> Result<(String, Option<String>)> {
    use std::process::Command;
    let src = Path::new(path);
    if !src.exists() {
        return Err(Error::NotFound(format!("file not found: {path}")));
    }

    // LibreOffice Impress (pptx/ppt) cannot export the Writer "Text" filter —
    // `--convert-to txt:Text` fails with an Io/Write error. Render the deck to a
    // temporary PDF (Impress CAN do that) and pull the text out with pdftotext.
    let ext = src
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if matches!(ext.as_str(), "pptx" | "ppt") {
        let tmp = std::env::temp_dir().join(format!("cortex-ppt-{}", crate::db::new_id()));
        std::fs::create_dir_all(&tmp)?;
        let pdf = tmp.join("deck.pdf");
        let res = libreoffice_to_pdf(path, &pdf).and_then(|_| {
            pdf_to_text(pdf.to_str().ok_or_else(|| Error::Other("bad temp path".into()))?)
        });
        let _ = std::fs::remove_dir_all(&tmp);
        return res;
    }

    let outdir = std::env::temp_dir().join(format!("cortex-ingest-{}", crate::db::new_id()));
    std::fs::create_dir_all(&outdir)?;

    let bin = libreoffice_bin();
    let output = Command::new(&bin)
        .args(["--headless", "--convert-to", "txt:Text", "--outdir"])
        .arg(&outdir)
        .arg(src)
        .output();

    let result = match output {
        Ok(o) if o.status.success() => {
            let stem = src.file_stem().and_then(|s| s.to_str()).unwrap_or("out");
            let txt_path = outdir.join(format!("{stem}.txt"));
            match std::fs::read_to_string(&txt_path) {
                Ok(t) => Ok((t, None)),
                Err(e) => Err(Error::Other(format!(
                    "libreoffice produced no text output for {path}: {e}"
                ))),
            }
        }
        Ok(o) => Err(Error::Other(format!(
            "libreoffice conversion of {path} failed: {}",
            String::from_utf8_lossy(&o.stderr).trim()
        ))),
        Err(e) => Err(Error::Other(format!(
            "libreoffice is not runnable ({e}); install LibreOffice to ingest {path}"
        ))),
    };
    let _ = std::fs::remove_dir_all(&outdir);
    result
}

/// Render a docx/pptx (or any LibreOffice-openable doc) to PDF and copy the
/// result to `dest`. Returns a hard `Err` (with stderr) if conversion fails so
/// callers never persist a half-ingested source silently.
pub fn libreoffice_to_pdf(path: &str, dest: &Path) -> Result<()> {
    use std::process::Command;
    let src = Path::new(path);
    if !src.exists() {
        return Err(Error::NotFound(format!("file not found: {path}")));
    }
    let outdir = std::env::temp_dir().join(format!("cortex-pdf-{}", crate::db::new_id()));
    std::fs::create_dir_all(&outdir)?;

    let bin = libreoffice_bin();
    let output = Command::new(&bin)
        .args(["--headless", "--convert-to", "pdf", "--outdir"])
        .arg(&outdir)
        .arg(src)
        .output();

    let result: Result<()> = match output {
        Ok(o) if o.status.success() => {
            let stem = src.file_stem().and_then(|s| s.to_str()).unwrap_or("out");
            let pdf_path = outdir.join(format!("{stem}.pdf"));
            if pdf_path.exists() {
                if let Some(parent) = dest.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                std::fs::copy(&pdf_path, dest)?;
                Ok(())
            } else {
                Err(Error::Other(format!(
                    "libreoffice did not produce a PDF for {path}"
                )))
            }
        }
        Ok(o) => Err(Error::Other(format!(
            "libreoffice PDF conversion of {path} failed: {}",
            String::from_utf8_lossy(&o.stderr).trim()
        ))),
        Err(e) => Err(Error::Other(format!(
            "libreoffice is not runnable ({e}); install LibreOffice to render {path}"
        ))),
    };
    let _ = std::fs::remove_dir_all(&outdir);
    result
}

fn libreoffice_bin() -> String {
    for c in ["libreoffice", "soffice"] {
        if which(c).is_some() {
            return c.to_string();
        }
    }
    "libreoffice".to_string()
}

pub fn which(cmd: &str) -> Option<String> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let full = dir.join(cmd);
        if full.is_file() {
            return Some(full.to_string_lossy().into_owned());
        }
    }
    None
}

/// Split text into bounded, overlapping chunks on word boundaries.
pub fn chunk_text(text: &str, target: usize, overlap: usize) -> Vec<String> {
    let words: Vec<&str> = text.split_whitespace().collect();
    if words.is_empty() {
        return Vec::new();
    }
    let mut chunks = Vec::new();
    let mut cur = String::new();
    let mut start_idx = 0usize;
    let mut i = 0usize;
    while i < words.len() {
        if !cur.is_empty() {
            cur.push(' ');
        }
        cur.push_str(words[i]);
        i += 1;
        if cur.len() >= target {
            chunks.push(cur.clone());
            // step back `overlap` chars worth of words for context continuity
            let mut back = 0usize;
            let mut j = i;
            while j > start_idx && back < overlap {
                j -= 1;
                back += words[j].len() + 1;
            }
            start_idx = j;
            i = j;
            cur.clear();
            // rebuild cur from start_idx is unnecessary; loop will append from i
        }
    }
    if !cur.trim().is_empty() {
        chunks.push(cur.trim().to_string());
    }
    chunks
}

/// Embed a batch of chunk texts with the configured embedder.
pub fn embed_chunks(embedder: &dyn Embedder, chunks: &[String]) -> Result<Vec<Vec<f32>>> {
    if chunks.is_empty() {
        return Ok(Vec::new());
    }
    embedder.embed(chunks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chunker_respects_bounds_and_covers_text() {
        let text = "word ".repeat(500);
        let chunks = chunk_text(&text, 200, 40);
        assert!(chunks.len() > 1);
        for c in &chunks {
            // allow one word of slop past target
            assert!(c.len() <= 200 + 12, "chunk too big: {}", c.len());
        }
    }

    #[test]
    fn chunker_empty_input() {
        assert!(chunk_text("   ", 100, 10).is_empty());
    }

    #[test]
    fn html_strip_removes_tags_and_scripts() {
        let html = "<html><head><style>x{}</style></head><body><p>Hello <b>world</b></p><script>bad()</script></body></html>";
        let t = html_to_text(html);
        assert!(t.contains("Hello world"));
        assert!(!t.contains("bad()"));
        assert!(!t.contains('<'));
    }

    #[test]
    fn detect_kind_from_url_and_path() {
        let yt = AddSourceInput {
            subject_id: "s".into(), topic_id: None, name: None, kind: None,
            text: None, path: None, url: Some("https://youtu.be/x".into()), tags: vec![],
        };
        assert_eq!(detect_kind(&yt), "yt");
        let pdf = AddSourceInput {
            subject_id: "s".into(), topic_id: None, name: None, kind: None,
            text: None, path: Some("/tmp/lec.pdf".into()), url: None, tags: vec![],
        };
        assert_eq!(detect_kind(&pdf), "pdf");
    }
}
