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
            "epub" => "epub",
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
            // Native OOXML extraction first (no external tools). Fall back to
            // LibreOffice only for legacy binary .doc/.ppt (not zip-based) or if
            // the native pass yields nothing.
            match ooxml_to_text(p) {
                Ok(t) if !t.trim().is_empty() => Ok((t, None)),
                _ => libreoffice_to_text(p),
            }
        }
        "epub" => {
            let p = input
                .path
                .as_deref()
                .ok_or_else(|| Error::Other("epub source needs a file path".into()))?;
            epub_to_text(p)
        }
        "yt" => {
            let url = input
                .url
                .as_deref()
                .ok_or_else(|| Error::Other("youtube source needs a url".into()))?;
            youtube_to_text(url)
        }
        // Enriched at the command layer: audio → Whisper, image → vision OCR.
        "audio" | "image" => Ok((String::new(), None)),
        other => Err(Error::Unsupported(format!("unknown source kind: {other}"))),
    }
}

/// Strip HTML to readable text: drop script/style, remove tags, decode a few
/// common entities, collapse whitespace. Dependency-light on purpose.
pub fn html_to_text(html: &str) -> String {
    // The `regex` crate has no backreferences, so drop each non-content element
    // with its own pattern rather than a captured-group close tag.
    // Compile the strip patterns once (was recompiled on every call — html_to_text
    // runs per web page and per EPUB chapter).
    static BLOCK_RES: std::sync::OnceLock<Vec<regex::Regex>> = std::sync::OnceLock::new();
    static TAG_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    static WS_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    // The `regex` crate has no backreferences, so drop each non-content element
    // with its own pattern rather than a captured-group close tag.
    let block_res = BLOCK_RES.get_or_init(|| {
        ["script", "style", "head", "nav", "footer"]
            .iter()
            .map(|tag| regex::Regex::new(&format!(r"(?is)<{tag}\b[^>]*>.*?</{tag}>")).unwrap())
            .collect()
    });
    let mut stripped = html.to_string();
    for re in block_res {
        stripped = re.replace_all(&stripped, " ").into_owned();
    }
    let re_tag = TAG_RE.get_or_init(|| regex::Regex::new(r"(?s)<[^>]+>").unwrap());
    let text = re_tag.replace_all(&stripped, " ").into_owned();
    let text = decode_entities(&text);
    // Drop zero-width / BOM characters that make extracted web text look garbled.
    let text: String = text
        .chars()
        .filter(|&c| !matches!(c, '\u{200b}' | '\u{200c}' | '\u{200d}' | '\u{feff}' | '\u{ad}'))
        .collect();
    let re_ws = WS_RE.get_or_init(|| regex::Regex::new(r"\s+").unwrap());
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
    static NUM_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
    let re_num = NUM_RE.get_or_init(|| regex::Regex::new(r"&#(x?[0-9A-Fa-f]+);").unwrap());
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
    const NO_TEXT: &str = "warning: no extractable text (scanned PDF?); preview still available";

    // Desktop: poppler's `pdftotext -layout` is fastest + highest fidelity. On a hard
    // failure with a real message, surface it; an empty/soft result falls through to
    // the pure-Rust pass below (some PDFs pdftotext can't read but pdf-extract can).
    if which("pdftotext").is_some() {
        match Command::new("pdftotext")
            .args(["-layout", "-enc", "UTF-8"])
            .arg(src)
            .arg("-")
            .output()
        {
            Ok(o) if o.status.success() => {
                let text = String::from_utf8_lossy(&o.stdout).into_owned();
                if !text.trim().is_empty() {
                    return Ok((text, None));
                }
            }
            Ok(o) => {
                let stderr = String::from_utf8_lossy(&o.stderr).trim().to_string();
                if !stderr.is_empty() {
                    return Err(Error::Other(format!("pdftotext failed for {path}: {stderr}")));
                }
            }
            Err(_) => {} // binary vanished between which() and exec → try pure-Rust
        }
    }

    // No poppler (iOS/Android, or a desktop without it) — extract the text layer in
    // pure Rust, on-device. Text-layer PDFs (lecture slides, papers, exported notes)
    // come through fully; a scanned/image-only PDF yields nothing here and degrades to
    // the "scanned?" warning (OCR is the homelab ingest service's job).
    match pdf_extract_text_layer(src) {
        Some(t) if !t.trim().is_empty() => Ok((t, None)),
        _ => Ok((String::new(), Some(NO_TEXT.into()))),
    }
}

/// Extract a PDF's embedded text layer with the pure-Rust `pdf-extract` crate.
/// Returns None on failure. `pdf-extract` can panic on some malformed PDFs, so it
/// runs inside `catch_unwind` (the build keeps `panic = unwind`) — a bad PDF must
/// degrade to a warning, never crash the ingest worker.
fn pdf_extract_text_layer(path: &Path) -> Option<String> {
    let owned = path.to_path_buf();
    std::panic::catch_unwind(move || pdf_extract::extract_text(&owned).ok())
        .ok()
        .flatten()
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
        .arg(lo_profile_arg(&outdir))
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
        .arg(lo_profile_arg(&outdir))
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

/// Is an office→PDF converter (LibreOffice) available? Used to decide whether to
/// attempt a faithful slide-preview render — when it isn't, ingestion still
/// succeeds via native text extraction, just without the rendered PDF preview.
pub fn office_converter_available() -> bool {
    which("libreoffice").is_some() || which("soffice").is_some()
}

/// Extract text from a `.docx`/`.pptx` natively (they're just zip + XML) so
/// office documents ingest on every OS with **no external tool** — Windows and
/// macOS users don't need LibreOffice. Legacy binary `.doc`/`.ppt` aren't
/// zip-based, so this returns `Err` for them and the caller falls back to
/// LibreOffice. Paragraph (`</w:p>`, `</a:p>`) and slide boundaries become
/// newlines so the extracted text stays readable and chunkable.
pub fn ooxml_to_text(path: &str) -> Result<String> {
    use std::io::Read;
    let file = std::fs::File::open(path)?;
    let mut zip = zip::ZipArchive::new(file)
        .map_err(|e| Error::Other(format!("{path} is not a valid OOXML (zip) file: {e}")))?;

    let ext = Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    // Which XML parts hold the body text, in reading order.
    let parts: Vec<String> = if ext == "docx" {
        vec!["word/document.xml".to_string()]
    } else {
        let mut slides: Vec<String> = (0..zip.len())
            .filter_map(|i| zip.by_index(i).ok().map(|f| f.name().to_string()))
            .filter(|n| n.starts_with("ppt/slides/slide") && n.ends_with(".xml"))
            .collect();
        // slide2.xml before slide10.xml — sort by the embedded number, not lexically.
        slides.sort_by_key(|n| {
            n.trim_start_matches("ppt/slides/slide")
                .trim_end_matches(".xml")
                .parse::<u32>()
                .unwrap_or(u32::MAX)
        });
        slides
    };

    // A run of text is <w:t>…</w:t> (Word) or <a:t>…</a:t> (PowerPoint); a
    // paragraph end (</w:p> / </a:p>) becomes a newline.
    let re = regex::Regex::new(r"(?s)<(?:w|a):t[^>]*>(.*?)</(?:w|a):t>|</(?:w|a):p>").unwrap();
    let mut out = String::new();
    for part in &parts {
        let mut xml = String::new();
        if let Ok(mut f) = zip.by_name(part) {
            if f.read_to_string(&mut xml).is_err() {
                continue;
            }
        } else {
            continue;
        }
        for cap in re.captures_iter(&xml) {
            match cap.get(1) {
                Some(m) => out.push_str(&decode_entities(m.as_str())),
                None => out.push('\n'),
            }
        }
        out.push_str("\n\n"); // blank line between slides / the document body
    }
    Ok(out.trim().to_string())
}

/// Extract reading-order text from an EPUB natively — it's a zip of XHTML
/// documents, so the same dependency-light approach as `ooxml_to_text` applies
/// (no epub crate, no external tool; works keyless on every OS incl. mobile).
/// We read `META-INF/container.xml` to locate the OPF package, follow its
/// `<spine>` for chapter order, resolve each chapter's href against the package
/// directory, and strip the XHTML to text with `html_to_text`. If the package
/// or spine can't be parsed we fall back to every `.xhtml`/`.html` entry in name
/// order, so even a malformed e-book yields its prose rather than nothing.
pub fn epub_to_text(path: &str) -> Result<(String, Option<String>)> {
    use std::io::Read;
    let file = std::fs::File::open(path)?;
    let mut zip = zip::ZipArchive::new(file)
        .map_err(|e| Error::Other(format!("{path} is not a valid EPUB (zip) file: {e}")))?;

    // Read a zip entry to a String by name (None if absent/unreadable).
    fn read_entry(zip: &mut zip::ZipArchive<std::fs::File>, name: &str) -> Option<String> {
        let mut s = String::new();
        zip.by_name(name).ok()?.read_to_string(&mut s).ok()?;
        Some(s)
    }

    // Resolve the spine to an ordered list of XHTML entry paths via the manifest.
    let ordered: Vec<String> = (|| {
        let container = read_entry(&mut zip, "META-INF/container.xml")?;
        let opf_path = regex::Regex::new(r#"(?is)full-path\s*=\s*["']([^"']+)["']"#)
            .unwrap()
            .captures(&container)
            .map(|c| percent_decode(&c[1]))?;
        let opf = read_entry(&mut zip, &opf_path)?;
        let opf_dir = opf_path.rsplit_once('/').map(|(d, _)| d).unwrap_or("");

        // Pull a quoted attribute value out of a single start-tag. One shared regex,
        // compiled once — was recompiled per attribute per <item> (per-book hot loop).
        let attr = |tag: &str, want: &str| {
            static ATTR_RE: std::sync::OnceLock<regex::Regex> = std::sync::OnceLock::new();
            ATTR_RE
                .get_or_init(|| regex::Regex::new(r#"(?is)([\w:-]+)\s*=\s*["']([^"']*)["']"#).unwrap())
                .captures_iter(tag)
                .find(|c| c[1].eq_ignore_ascii_case(want))
                .map(|c| c[2].to_string())
        };

        // manifest: id -> (href, media-type)
        let mut manifest: std::collections::HashMap<String, (String, String)> =
            std::collections::HashMap::new();
        for m in regex::Regex::new(r"(?is)<item\b[^>]*>").unwrap().find_iter(&opf) {
            let tag = m.as_str();
            if let (Some(id), Some(href)) = (attr(tag, "id"), attr(tag, "href")) {
                manifest.insert(id, (href, attr(tag, "media-type").unwrap_or_default()));
            }
        }
        // spine: ordered idrefs → manifest hrefs (XHTML content documents only)
        let mut order = Vec::new();
        for m in regex::Regex::new(r"(?is)<itemref\b[^>]*>").unwrap().find_iter(&opf) {
            if let Some(idref) = attr(m.as_str(), "idref") {
                if let Some((href, mtype)) = manifest.get(&idref) {
                    if is_xhtml(href, mtype) {
                        order.push(zip_join(opf_dir, &percent_decode(href)));
                    }
                }
            }
        }
        if order.is_empty() {
            None
        } else {
            Some(order)
        }
    })()
    .unwrap_or_else(|| {
        // Fallback: every (x)html document in the archive, in name order.
        let mut names: Vec<String> = (0..zip.len())
            .filter_map(|i| zip.by_index(i).ok().map(|f| f.name().to_string()))
            .filter(|n| is_xhtml(n, ""))
            .collect();
        names.sort();
        names
    });

    let mut out = String::new();
    for name in &ordered {
        if let Some(html) = read_entry(&mut zip, name) {
            let text = html_to_text(&html);
            if !text.trim().is_empty() {
                out.push_str(text.trim());
                out.push_str("\n\n");
            }
        }
    }
    let out = out.trim().to_string();
    if out.is_empty() {
        Ok((String::new(), Some("warning: no readable text found in EPUB".into())))
    } else {
        Ok((out, None))
    }
}

/// Is this href / media-type an (X)HTML content document worth extracting?
fn is_xhtml(href: &str, media_type: &str) -> bool {
    let h = href.to_lowercase();
    media_type.contains("xhtml")
        || media_type == "text/html"
        || h.ends_with(".xhtml")
        || h.ends_with(".html")
        || h.ends_with(".htm")
}

/// Join an EPUB-internal href onto its package directory, resolving `.`/`..`
/// segments and dropping any `#fragment`/`?query`. Zip entry names always use
/// forward slashes, so this stays slash-based on every OS.
fn zip_join(base_dir: &str, href: &str) -> String {
    let href = href.split(['#', '?']).next().unwrap_or(href);
    let mut segs: Vec<&str> = base_dir.split('/').filter(|s| !s.is_empty()).collect();
    for part in href.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                segs.pop();
            }
            p => segs.push(p),
        }
    }
    segs.join("/")
}

/// Decode `%XX` escapes in an OPF href so it matches the raw zip entry name
/// (a space is `%20` in the href but literal in the archive). Leaves a trailing
/// or malformed `%` untouched.
fn percent_decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            if let Ok(b) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(b);
                i += 3;
                continue;
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// A `-env:UserInstallation` arg pointing at a throwaway profile inside `outdir`.
/// Every headless conversion MUST use its own profile: otherwise a second
/// concurrent soffice — another source ingesting, a pptx/docx preview rendering,
/// or the user's own open LibreOffice — collides on the shared default profile
/// lock and silently fails (exit 1, empty stderr, no output file). That lock
/// collision is the real cause of "pptx/docx upload is broken / produced no
/// output". An isolated profile per call removes the contention entirely.
/// Fetch a YouTube transcript via yt-dlp (subtitles → text). Uses yt-dlp if it's
/// on PATH; otherwise returns an actionable error. Prefers manual subs, falls
/// back to auto-generated captions.
fn youtube_to_text(url: &str) -> Result<(String, Option<String>)> {
    use std::process::Command;
    // No yt-dlp (e.g. on mobile)? Fall back to a pure-HTTP caption fetch — no binary.
    let Some(bin) = tool("yt-dlp") else {
        return youtube_via_http(url);
    };
    let dir = std::env::temp_dir().join(format!("cortex-yt-{}", crate::db::new_id()));
    std::fs::create_dir_all(&dir)?;
    let out_tmpl = dir.join("%(id)s.%(ext)s");
    let out = Command::new(&bin)
        .args([
            "--skip-download",
            "--write-subs",
            "--write-auto-subs",
            "--sub-langs",
            "en.*,en",
            "--sub-format",
            "vtt",
            "--no-playlist",
            "-o",
        ])
        .arg(&out_tmpl)
        .arg(url)
        .output();
    let res: Result<(String, Option<String>)> = match out {
        Ok(o) if o.status.success() => {
            let vtt = std::fs::read_dir(&dir)?
                .flatten()
                .map(|e| e.path())
                .find(|p| p.extension().and_then(|x| x.to_str()) == Some("vtt"));
            match vtt {
                Some(p) => {
                    let text = vtt_to_text(&std::fs::read_to_string(&p)?);
                    if text.trim().is_empty() {
                        Err(Error::Other("the video's transcript was empty".into()))
                    } else {
                        Ok((text, None))
                    }
                }
                None => Err(Error::Other(
                    "no English captions are available for this video".into(),
                )),
            }
        }
        Ok(o) => Err(Error::Other(format!(
            "yt-dlp failed: {}",
            String::from_utf8_lossy(&o.stderr).trim().chars().take(200).collect::<String>()
        ))),
        Err(e) => Err(Error::Other(format!("yt-dlp not runnable: {e}"))),
    };
    let _ = std::fs::remove_dir_all(&dir);
    res
}

/// Pure-HTTP YouTube transcript fetch (no yt-dlp): pull the watch page, find a
/// caption track URL in the player response, fetch the timedtext, strip to prose.
/// Best-effort (YouTube can change this) — used as the mobile/binary-less fallback.
fn youtube_via_http(url: &str) -> Result<(String, Option<String>)> {
    let client = crate::commands::http_client(30);
    let html = client
        .get(url)
        .header("Accept-Language", "en-US,en;q=0.9")
        .send()?
        .text()?;
    let tracks = regex::Regex::new(r#""captionTracks":(\[.*?\])"#)
        .unwrap()
        .captures(&html)
        .map(|c| c[1].to_string())
        .ok_or_else(|| Error::Other("no captions are available for this video".into()))?;
    let burl_re = regex::Regex::new(r#""baseUrl":"(.*?)""#).unwrap();
    let lang_re = regex::Regex::new(r#""languageCode":"(.*?)""#).unwrap();
    // Prefer an English track; otherwise take the first.
    let mut chosen: Option<String> = None;
    for chunk in tracks.split("},{") {
        let Some(burl) = burl_re.captures(chunk).map(|c| c[1].to_string()) else { continue };
        let lang = lang_re.captures(chunk).map(|c| c[1].to_string()).unwrap_or_default();
        if lang.starts_with("en") {
            chosen = Some(burl);
            break;
        }
        chosen.get_or_insert(burl);
    }
    let base = chosen
        .ok_or_else(|| Error::Other("no caption track URL found".into()))?
        .replace("\\u0026", "&")
        .replace("\\/", "/");
    let xml = client.get(&base).send()?.text()?;
    let text = timedtext_to_text(&xml);
    if text.trim().is_empty() {
        Err(Error::Other("the video's transcript was empty".into()))
    } else {
        Ok((text, None))
    }
}

/// Strip YouTube timedtext XML (`<text start=… dur=…>…</text>`) to plain prose.
fn timedtext_to_text(xml: &str) -> String {
    let text_re = regex::Regex::new(r"(?s)<text[^>]*>(.*?)</text>").unwrap();
    let tag_re = regex::Regex::new(r"<[^>]+>").unwrap();
    let mut parts: Vec<String> = Vec::new();
    for c in text_re.captures_iter(xml) {
        let inner = tag_re.replace_all(&c[1], " ").into_owned();
        let t = decode_entities(&inner)
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" ");
        if !t.is_empty() {
            parts.push(t);
        }
    }
    parts.join(" ")
}

/// Strip a WebVTT subtitle file to plain deduped prose.
fn vtt_to_text(vtt: &str) -> String {
    let mut out: Vec<String> = Vec::new();
    for line in vtt.lines() {
        let t = line.trim();
        if t.is_empty()
            || t == "WEBVTT"
            || t.starts_with("Kind:")
            || t.starts_with("Language:")
            || t.starts_with("NOTE")
            || t.contains("-->")
            || t.chars().all(|c| c.is_ascii_digit())
        {
            continue;
        }
        // Drop inline timing/style tags like <00:00:01.000> and <c>…</c>.
        let mut cleaned = String::with_capacity(t.len());
        let mut in_tag = false;
        for c in t.chars() {
            match c {
                '<' => in_tag = true,
                '>' => in_tag = false,
                _ if !in_tag => cleaned.push(c),
                _ => {}
            }
        }
        let cleaned = cleaned.trim().to_string();
        // Auto-captions roll the same line repeatedly — drop consecutive repeats.
        if cleaned.is_empty() || out.last() == Some(&cleaned) {
            continue;
        }
        out.push(cleaned);
    }
    out.join(" ")
}

/// MIME type for an image source path (for OCR data URLs).
pub fn image_mime(path: &str) -> &'static str {
    match Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase()
        .as_str()
    {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "webp" => "image/webp",
        "gif" => "image/gif",
        _ => "image/png",
    }
}

/// Render up to `max_pages` of a PDF to PNG page images (bytes) via poppler's
/// `pdftoppm`. Used to OCR scanned/image-only PDFs through a vision model.
pub fn pdf_page_images(path: &str, max_pages: usize) -> Result<Vec<Vec<u8>>> {
    use std::process::Command;
    let Some(pdftoppm) = which("pdftoppm") else {
        return Err(Error::Other(
            "pdftoppm (poppler) not found — needed to OCR scanned PDFs".into(),
        ));
    };
    let dir = std::env::temp_dir().join(format!("cortex-ocr-{}", crate::db::new_id()));
    std::fs::create_dir_all(&dir)?;
    let prefix = dir.join("page");
    let out = Command::new(&pdftoppm)
        .args(["-png", "-r", "150", "-l", &max_pages.to_string()])
        .arg(path)
        .arg(&prefix)
        .output();
    let result: Result<Vec<Vec<u8>>> = match out {
        Ok(o) if o.status.success() => {
            let mut paths: Vec<_> = std::fs::read_dir(&dir)?
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().and_then(|x| x.to_str()) == Some("png"))
                .collect();
            paths.sort();
            Ok(paths.iter().filter_map(|p| std::fs::read(p).ok()).collect())
        }
        Ok(o) => Err(Error::Other(format!(
            "pdftoppm failed: {}",
            String::from_utf8_lossy(&o.stderr).trim()
        ))),
        Err(e) => Err(Error::Other(format!("pdftoppm not runnable: {e}"))),
    };
    let _ = std::fs::remove_dir_all(&dir);
    result
}

fn lo_profile_arg(outdir: &Path) -> String {
    let profile = outdir.join("lo-profile");
    format!("-env:UserInstallation=file://{}", profile.to_string_lossy())
}

fn chromium_bin() -> Option<String> {
    for c in [
        "chromium",
        "chromium-browser",
        "google-chrome",
        "google-chrome-stable",
        "chrome",
        "brave",
    ] {
        if let Some(p) = which(c) {
            return Some(p);
        }
    }
    None
}

/// Render a self-contained HTML string to a PDF at `dest`. Prefers headless
/// Chromium (full CSS/markdown-table/callout fidelity) and falls back to
/// LibreOffice's HTML import (lower fidelity) when no Chromium is present.
/// The frontend builds the styled HTML and chooses `dest` via the save dialog,
/// so this is a pure render-to-file step. Errors are hard (never a silent
/// empty/zero-byte PDF) so the UI can surface a real failure.
pub fn html_to_pdf(html: &str, dest: &Path) -> Result<()> {
    use std::process::Command;
    let tmp = std::env::temp_dir().join(format!("cortex-export-{}", crate::db::new_id()));
    std::fs::create_dir_all(&tmp)?;
    let html_path = tmp.join("doc.html");
    std::fs::write(&html_path, html)?;
    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let result: Result<()> = if let Some(bin) = chromium_bin() {
        let profile = tmp.join("profile");
        let mut pdf_arg = std::ffi::OsString::from("--print-to-pdf=");
        pdf_arg.push(dest);
        let mut user_data = std::ffi::OsString::from("--user-data-dir=");
        user_data.push(&profile);
        let out = Command::new(&bin)
            .arg("--headless")
            .arg("--no-sandbox")
            .arg("--disable-gpu")
            .arg("--no-pdf-header-footer")
            .arg("--virtual-time-budget=5000")
            .arg(user_data)
            .arg(pdf_arg)
            .arg(&html_path)
            .output();
        match out {
            Ok(o) if o.status.success() && dest.exists() => Ok(()),
            Ok(o) => Err(Error::Other(format!(
                "chromium PDF export failed: {}",
                String::from_utf8_lossy(&o.stderr).trim()
            ))),
            Err(e) => Err(Error::Other(format!("chromium is not runnable: {e}"))),
        }
    } else {
        // Fallback: LibreOffice opens the .html and exports a PDF.
        libreoffice_to_pdf(
            html_path
                .to_str()
                .ok_or_else(|| Error::Other("bad temp path".into()))?,
            dest,
        )
    };
    let _ = std::fs::remove_dir_all(&tmp);
    result
}

pub fn which(cmd: &str) -> Option<String> {
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            let full = dir.join(cmd);
            if full.is_file() {
                return Some(full.to_string_lossy().into_owned());
            }
        }
    }
    // Desktop launchers often start the app with a minimal PATH that misses
    // the user's shell additions — probe the standard install dirs too.
    let mut cands: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(h) = std::env::var("HOME") {
        cands.push(std::path::Path::new(&h).join(".local/bin").join(cmd));
    }
    // Homebrew: Apple Silicon (/opt/homebrew) first, then Intel (/usr/local). macOS GUI
    // apps launched from Finder get a minimal PATH that omits both, so probe directly.
    cands.push(std::path::Path::new("/opt/homebrew/bin").join(cmd));
    cands.push(std::path::Path::new("/opt/homebrew/sbin").join(cmd));
    cands.push(std::path::Path::new("/usr/local/bin").join(cmd));
    cands.push(std::path::Path::new("/usr/local/sbin").join(cmd));
    cands.push(std::path::Path::new("/usr/bin").join(cmd));
    cands
        .into_iter()
        .find(|p| p.is_file())
        .map(|p| p.to_string_lossy().into_owned())
}

/// Resolve a binary bundled as a Tauri sidecar — shipped next to the app
/// executable (`bundle.externalBin`). Returns `None` in `tauri dev` or if the
/// sidecar isn't present, so callers fall back to PATH / runtime download.
pub fn bundled(name: &str) -> Option<String> {
    let exe = std::env::current_exe().ok()?;
    let dir = exe.parent()?;
    let fname = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    };
    let p = dir.join(fname);
    if p.is_file() {
        Some(p.to_string_lossy().into_owned())
    } else {
        None
    }
}

/// A tool's path: the bundled sidecar if present, else found on PATH.
pub fn tool(name: &str) -> Option<String> {
    bundled(name).or_else(|| which(name))
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
    fn ooxml_text_extraction_docx_and_pptx_no_tools() {
        use std::io::Write;
        use zip::write::SimpleFileOptions;

        let dir = std::env::temp_dir().join(format!("cortex-ooxml-test-{}", crate::db::new_id()));
        std::fs::create_dir_all(&dir).unwrap();

        // Minimal .docx: two paragraphs with an escaped ampersand.
        let docx = dir.join("doc.docx");
        {
            let mut zw = zip::ZipWriter::new(std::fs::File::create(&docx).unwrap());
            zw.start_file("word/document.xml", SimpleFileOptions::default()).unwrap();
            zw.write_all(
                br#"<?xml version="1.0"?><w:document><w:body>
                <w:p><w:r><w:t>Hello</w:t></w:r><w:r><w:t xml:space="preserve"> world</w:t></w:r></w:p>
                <w:p><w:r><w:t>Cats &amp; dogs</w:t></w:r></w:p>
                </w:body></w:document>"#,
            ).unwrap();
            zw.finish().unwrap();
        }
        let dtext = ooxml_to_text(docx.to_str().unwrap()).unwrap();
        assert!(dtext.contains("Hello world"), "docx text: {dtext:?}");
        assert!(dtext.contains("Cats & dogs"), "entity decode: {dtext:?}");

        // Minimal .pptx: two slides, slide10 must sort after slide2.
        let pptx = dir.join("deck.pptx");
        {
            let mut zw = zip::ZipWriter::new(std::fs::File::create(&pptx).unwrap());
            for (n, body) in [(2u32, "Second"), (10u32, "Tenth")] {
                zw.start_file(format!("ppt/slides/slide{n}.xml"), SimpleFileOptions::default()).unwrap();
                zw.write_all(format!(r#"<p:sld><a:p><a:r><a:t>{body}</a:t></a:r></a:p></p:sld>"#).as_bytes()).unwrap();
            }
            zw.finish().unwrap();
        }
        let ptext = ooxml_to_text(pptx.to_str().unwrap()).unwrap();
        assert!(ptext.contains("Second") && ptext.contains("Tenth"), "pptx text: {ptext:?}");
        assert!(ptext.find("Second").unwrap() < ptext.find("Tenth").unwrap(), "slide order: {ptext:?}");

        // A non-OOXML file (legacy binary) errors so the caller falls back to LibreOffice.
        let doc = dir.join("legacy.doc");
        std::fs::write(&doc, b"\xD0\xCF\x11\xE0 not a zip").unwrap();
        assert!(ooxml_to_text(doc.to_str().unwrap()).is_err());

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn epub_text_extraction_no_tools() {
        use std::io::Write;
        use zip::write::SimpleFileOptions;

        let dir = std::env::temp_dir().join(format!("cortex-epub-test-{}", crate::db::new_id()));
        std::fs::create_dir_all(&dir).unwrap();
        let epub = dir.join("book.epub");
        {
            let mut zw = zip::ZipWriter::new(std::fs::File::create(&epub).unwrap());
            let opts = SimpleFileOptions::default();
            zw.start_file("META-INF/container.xml", opts).unwrap();
            zw.write_all(
                br#"<?xml version="1.0"?>
                <container xmlns="urn:oasis:names:tc:opendocument:xmlns:container" version="1.0">
                <rootfiles><rootfile full-path="OEBPS/content.opf" media-type="application/oebps-package+xml"/></rootfiles>
                </container>"#,
            ).unwrap();
            zw.start_file("OEBPS/content.opf", opts).unwrap();
            zw.write_all(
                br#"<?xml version="1.0"?>
                <package xmlns="http://www.idpf.org/2007/opf" version="3.0">
                <manifest>
                <item id="c1" href="text/1.xhtml" media-type="application/xhtml+xml"/>
                <item id="c2" href="text/2.xhtml" media-type="application/xhtml+xml"/>
                <item id="css" href="style.css" media-type="text/css"/>
                </manifest>
                <spine><itemref idref="c1"/><itemref idref="c2"/></spine>
                </package>"#,
            ).unwrap();
            zw.start_file("OEBPS/text/1.xhtml", opts).unwrap();
            zw.write_all(br#"<html><body><h1>First chapter</h1><p>Cats &amp; dogs</p></body></html>"#).unwrap();
            zw.start_file("OEBPS/text/2.xhtml", opts).unwrap();
            zw.write_all(br#"<html><body><p>Second chapter</p></body></html>"#).unwrap();
            zw.finish().unwrap();
        }

        let (text, warn) = epub_to_text(epub.to_str().unwrap()).unwrap();
        assert!(warn.is_none(), "unexpected warning: {warn:?}");
        assert!(text.contains("First chapter"), "epub text: {text:?}");
        assert!(text.contains("Second chapter"), "epub text: {text:?}");
        assert!(text.contains("Cats & dogs"), "entity decode: {text:?}");
        // The CSS item is not a content document and must be skipped.
        assert!(!text.contains("style.css"), "non-content leaked: {text:?}");
        // Spine order (c1 before c2) must be honored.
        assert!(
            text.find("First").unwrap() < text.find("Second").unwrap(),
            "reading order: {text:?}"
        );

        let _ = std::fs::remove_dir_all(&dir);
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
