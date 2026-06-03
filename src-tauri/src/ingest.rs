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
            let html = reqwest::blocking::get(url)?.text()?;
            Ok((html_to_text(&html), None))
        }
        "pdf" | "docx" | "pptx" => {
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

/// Convert pdf/docx/pptx to text via `libreoffice --headless --convert-to txt`.
fn libreoffice_to_text(path: &str) -> Result<(String, Option<String>)> {
    use std::process::Command;
    let src = Path::new(path);
    if !src.exists() {
        return Err(Error::NotFound(format!("file not found: {path}")));
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
                Err(e) => Ok((
                    String::new(),
                    Some(format!("libreoffice produced no text output: {e}")),
                )),
            }
        }
        Ok(o) => Ok((
            String::new(),
            Some(format!(
                "libreoffice conversion failed: {}",
                String::from_utf8_lossy(&o.stderr)
            )),
        )),
        Err(e) => Ok((
            String::new(),
            Some(format!("libreoffice not runnable ({e}); install it to ingest {path}")),
        )),
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
