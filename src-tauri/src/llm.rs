//! LLM providers behind one trait, mirroring embed.rs. Default is an offline
//! `stub` that returns a clearly-marked placeholder so the app works with zero
//! config; Gemini `generateContent` (BYOK) is the real path. Per-task model is
//! read from settings as `model_<task>` = "provider:model" (e.g. "gemini:gemini-2.5-flash").

use crate::error::{Error, Result};

pub trait Llm: Send + Sync {
    fn complete(&self, system: &str, user: &str) -> Result<String>;
    fn name(&self) -> String;
    /// Cap output tokens for this provider's request. Wiring the per-task token
    /// budget here is what stops OpenRouter from defaulting to a huge max_tokens
    /// (its model default) and 402-ing when the key's credit limit can't cover it.
    /// Default: no-op (providers that don't store it ignore the cap).
    fn set_max_tokens(&mut self, _max: u32) {}
    /// OCR/transcribe images (each `(mime, base64)`) to Markdown text. Default:
    /// unsupported — only vision-capable providers override this.
    fn ocr(&self, _images: &[(String, String)]) -> Result<String> {
        Err(Error::Unsupported(format!(
            "{} can't read images — pick a vision model (e.g. an OpenRouter/Gemini multimodal model) in Settings",
            self.name()
        )))
    }
    /// Generate an image from a text prompt; returns a `data:image/...;base64,…`
    /// URL. Only image-capable models (e.g. Gemini "nano-banana") override this.
    fn gen_image(&self, _prompt: &str) -> Result<String> {
        Err(Error::Unsupported(format!(
            "{} can't generate images — use an image model like openrouter:google/gemini-2.5-flash-image",
            self.name()
        )))
    }
}

/// Standard base64 (with padding). Inline to avoid a dependency just for OCR
/// data URLs / inline image payloads.
pub fn b64_encode(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as u32;
        let b1 = *chunk.get(1).unwrap_or(&0) as u32;
        let b2 = *chunk.get(2).unwrap_or(&0) as u32;
        let n = (b0 << 16) | (b1 << 8) | b2;
        out.push(T[((n >> 18) & 63) as usize] as char);
        out.push(T[((n >> 12) & 63) as usize] as char);
        out.push(if chunk.len() > 1 { T[((n >> 6) & 63) as usize] as char } else { '=' });
        out.push(if chunk.len() > 2 { T[(n & 63) as usize] as char } else { '=' });
    }
    out
}

const OCR_PROMPT: &str = "Transcribe ALL text in these page image(s) EXACTLY, in reading order — \
including HANDWRITTEN text (photographed notes, whiteboards): transcribe messy writing as \
faithfully as you can and mark genuinely unreadable words as [illegible]. \
Preserve headings, lists, tables (as Markdown tables), and math. Do not summarise, \
translate, or add commentary — output ONLY the transcribed text as Markdown.";

/// Offline placeholder — no network, no key. Produces something usable so the
/// UI flow works, while telling the user to connect a model for real output.
#[allow(dead_code)]
pub struct StubLlm;

impl Llm for StubLlm {
    fn complete(&self, _system: &str, user: &str) -> Result<String> {
        let preview: String = user.chars().take(280).collect();
        Ok(format!(
            "[Offline draft — connect a model in Settings → API keys for real generation.]\n\n\
             Based on the provided sources:\n{preview}…"
        ))
    }
    fn name(&self) -> String {
        "stub".into()
    }
}

/// Google Gemini `generateContent` (BYOK).
pub struct GeminiLlm {
    pub api_key: String,
    pub model: String,
    pub max_tokens: Option<u32>,
}

impl Llm for GeminiLlm {
    fn set_max_tokens(&mut self, max: u32) {
        self.max_tokens = Some(max);
    }
    fn complete(&self, system: &str, user: &str) -> Result<String> {
        let client = llm_client();
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            self.model, self.api_key
        );
        // Gemini 2.5 models reason with hidden "thinking" tokens that draw from
        // the SAME maxOutputTokens budget. A small budget is routinely exhausted
        // by thinking before any answer text is emitted, leaving an empty
        // response with a MAX_TOKENS finishReason — the real cause of
        // "generation doesn't work at all" for cheatsheet/material.
        // Two defenses: a roomy budget, and — for flash models, which allow it —
        // disable thinking entirely so every token goes to the actual output.
        // (gemini-2.5-pro rejects thinkingBudget:0, so only set it for flash.)
        // Roomy output budget: a full 7-section cheatsheet (or a merged subject
        // digest) easily exceeds a small cap, and the model would otherwise stop
        // mid-answer — e.g. a table cut off halfway. 2.5 models support 65536.
        let mut generation_config = serde_json::json!({
            "temperature": 0.3,
            "maxOutputTokens": self.max_tokens.unwrap_or(65536)
        });
        if self.model.contains("flash") {
            generation_config["thinkingConfig"] = serde_json::json!({ "thinkingBudget": 0 });
        }
        let body = serde_json::json!({
            "system_instruction": { "parts": [{ "text": system }] },
            "contents": [{ "role": "user", "parts": [{ "text": user }] }],
            "generationConfig": generation_config
        });
        let json = send_json("gemini", client.post(&url).json(&body))?;
        // Concatenate every text part (some responses split across parts).
        let text: String = json["candidates"][0]["content"]["parts"]
            .as_array()
            .map(|parts| {
                parts
                    .iter()
                    .filter_map(|p| p["text"].as_str())
                    .collect::<String>()
            })
            .unwrap_or_default();
        if text.is_empty() {
            // Surface the finishReason (e.g. MAX_TOKENS / SAFETY) so the failure
            // is diagnosable instead of an opaque "empty response".
            let reason = json["candidates"][0]["finishReason"]
                .as_str()
                .unwrap_or("unknown");
            return Err(Error::Other(format!(
                "gemini: empty response (finishReason={reason}). \
                 If MAX_TOKENS, the model spent its budget on reasoning — try a flash model."
            )));
        }
        Ok(text)
    }
    fn name(&self) -> String {
        format!("gemini:{}", self.model)
    }
    fn ocr(&self, images: &[(String, String)]) -> Result<String> {
        let client = llm_client();
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            self.model, self.api_key
        );
        let mut parts = vec![serde_json::json!({ "text": OCR_PROMPT })];
        for (mime, b64) in images {
            parts.push(serde_json::json!({ "inline_data": { "mime_type": mime, "data": b64 } }));
        }
        let body = serde_json::json!({
            "contents": [{ "role": "user", "parts": parts }],
            "generationConfig": { "temperature": 0.0, "maxOutputTokens": 8192 }
        });
        let json = send_json("gemini", client.post(&url).json(&body))?;
        Ok(json["candidates"][0]["content"]["parts"]
            .as_array()
            .map(|ps| ps.iter().filter_map(|p| p["text"].as_str()).collect::<String>())
            .unwrap_or_default())
    }
}

fn truncate(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// Pull the affordable token count out of an OpenRouter 402 message, e.g.
/// "...you requested up to 64000 tokens, but can only afford 55765." Returns the
/// last number after "afford" so we can retry within the key's credit limit.
fn parse_affordable(msg: &str) -> Option<u32> {
    let idx = msg.find("afford")?;
    let tail = &msg[idx..];
    let digits: String = tail.chars().skip_while(|c| !c.is_ascii_digit()).take_while(|c| c.is_ascii_digit()).collect();
    digits.parse::<u32>().ok().filter(|n| *n > 0)
}

/// Shared blocking HTTP client for LLM calls: a generous request timeout (model
/// generation can be slow) so a stalled connection fails cleanly instead of
/// hanging forever.
fn llm_client() -> reqwest::blocking::Client {
    // Force identity (uncompressed) transfer encoding. OpenRouter streams a gzip
    // body padded with whitespace keep-alives during slow generation, which
    // reqwest's automatic gzip decoder fails to decode — surfacing as the opaque
    // "error decoding response body" that broke cheatsheet/material generation.
    // Setting Accept-Encoding ourselves ALSO disables reqwest's auto-decompress,
    // so the body is read verbatim regardless of what the server sends.
    let mut headers = reqwest::header::HeaderMap::new();
    headers.insert(
        reqwest::header::ACCEPT_ENCODING,
        reqwest::header::HeaderValue::from_static("identity"),
    );
    reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(300))
        .default_headers(headers)
        .build()
        .unwrap_or_default()
}

/// Send a request, then read its body as text and parse it as JSON — with
/// retries on TRANSIENT transport failures. Long LLM generations behind proxies
/// (notably OpenRouter, which streams whitespace keep-alives during slow
/// generation) occasionally drop the connection mid-body; reqwest surfaces that
/// as the opaque "error decoding response body". A couple of retries recover it.
/// Non-2xx responses and JSON-parse failures are returned verbatim (with the
/// body for diagnosis) and are NOT retried. Reading as text (not `resp.json()`)
/// keeps errors diagnosable instead of opaque.
fn send_json(provider: &str, req: reqwest::blocking::RequestBuilder) -> Result<serde_json::Value> {
    let transient = |e: &reqwest::Error| {
        e.is_timeout() || e.is_connect() || e.is_request() || e.is_body() || e.is_decode()
    };
    let mut attempt = 0u32;
    loop {
        attempt += 1;
        let this = req
            .try_clone()
            .ok_or_else(|| Error::Other(format!("{provider}: request could not be retried")))?;
        let resp = match this.send() {
            Ok(r) => r,
            Err(e) if attempt < 3 && transient(&e) => {
                std::thread::sleep(std::time::Duration::from_millis(800 * attempt as u64));
                continue;
            }
            Err(e) => return Err(e.into()),
        };
        let status = resp.status();
        let raw = match resp.text() {
            Ok(t) => t,
            Err(e) if attempt < 3 && transient(&e) => {
                std::thread::sleep(std::time::Duration::from_millis(800 * attempt as u64));
                continue;
            }
            Err(e) => {
                return Err(Error::Other(format!(
                    "{provider}: could not read response body: {e}"
                )))
            }
        };
        if !status.is_success() {
            return Err(Error::Other(format!(
                "{provider} {status}: {}",
                truncate(raw.trim(), 300)
            )));
        }
        match serde_json::from_str(&raw) {
            Ok(v) => return Ok(v),
            // A successful (2xx) response that doesn't parse is almost always a
            // TRUNCATED body (connection cut mid-stream during a long/large
            // generation — e.g. OCR with big images). Retry rather than fail.
            Err(_) if attempt < 3 => {
                std::thread::sleep(std::time::Duration::from_millis(800 * attempt as u64));
                continue;
            }
            Err(e) => {
                return Err(Error::Other(format!(
                    "{provider}: response was not valid JSON ({e}): {}",
                    truncate(raw.trim(), 300)
                )))
            }
        }
    }
}

/// OpenAI-compatible chat provider — used for OpenRouter and OpenAI (and any
/// custom OpenAI-compatible gateway). One code path, different base URL/key.
pub struct OpenAiCompatLlm {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub label: &'static str,
    pub max_tokens: Option<u32>,
}

impl OpenAiCompatLlm {
    /// One chat/completions attempt with an explicit max_tokens (or none).
    fn complete_once(&self, system: &str, user: &str, max_tokens: Option<u32>) -> Result<String> {
        let key = self.api_key.trim();
        let client = llm_client();
        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
        let mut body = serde_json::json!({
            "model": self.model,
            "messages": [
                { "role": "system", "content": system },
                { "role": "user", "content": user }
            ],
            "temperature": 0.3
        });
        if let Some(n) = max_tokens {
            body["max_tokens"] = serde_json::json!(n);
        }
        let json = send_json(
            self.label,
            client
                .post(&url)
                .header("Authorization", format!("Bearer {key}"))
                // OpenRouter likes these; harmless elsewhere.
                .header("HTTP-Referer", "https://cortex.study")
                .header("X-Title", "Cortex")
                .json(&body),
        )?;
        json["choices"][0]["message"]["content"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| Error::Other(format!("{}: empty response", self.label)))
    }
}

impl Llm for OpenAiCompatLlm {
    fn set_max_tokens(&mut self, max: u32) {
        self.max_tokens = Some(max);
    }
    fn complete(&self, system: &str, user: &str) -> Result<String> {
        let key = self.api_key.trim();
        if key.is_empty() {
            return Err(Error::Other(format!(
                "{}: API key is empty — paste it in Settings → API keys and click Save keys.",
                self.label
            )));
        }
        match self.complete_once(system, user, self.max_tokens) {
            Ok(t) => Ok(t),
            // OpenRouter returns 402 with "...can only afford N" when the key's
            // credit limit can't cover the requested max_tokens. Retry once with
            // the affordable amount so generation succeeds instead of hard-failing.
            Err(Error::Other(msg)) if msg.contains("402") => {
                if let Some(afford) = parse_affordable(&msg) {
                    // Leave a little headroom under the affordable cap.
                    let retry = afford.saturating_sub(512).max(256);
                    return self.complete_once(system, user, Some(retry));
                }
                Err(Error::Other(msg))
            }
            Err(e) => Err(e),
        }
    }
    fn name(&self) -> String {
        format!("{}:{}", self.label, self.model)
    }
    fn ocr(&self, images: &[(String, String)]) -> Result<String> {
        let key = self.api_key.trim();
        if key.is_empty() {
            return Err(Error::Other(format!("{}: API key is empty", self.label)));
        }
        let client = llm_client();
        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
        let mut content = vec![serde_json::json!({ "type": "text", "text": OCR_PROMPT })];
        for (mime, b64) in images {
            content.push(serde_json::json!({
                "type": "image_url",
                "image_url": { "url": format!("data:{mime};base64,{b64}") }
            }));
        }
        let body = serde_json::json!({
            "model": self.model,
            "messages": [{ "role": "user", "content": content }],
            "temperature": 0.0
        });
        let json = send_json(
            self.label,
            client
                .post(&url)
                .header("Authorization", format!("Bearer {key}"))
                .header("HTTP-Referer", "https://cortex.study")
                .header("X-Title", "Cortex")
                .json(&body),
        )?;
        Ok(json["choices"][0]["message"]["content"]
            .as_str()
            .unwrap_or_default()
            .to_string())
    }
    fn gen_image(&self, prompt: &str) -> Result<String> {
        let key = self.api_key.trim();
        if key.is_empty() {
            return Err(Error::Other(format!("{}: API key is empty", self.label)));
        }
        let client = llm_client();
        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
        let body = serde_json::json!({
            "model": self.model,
            "messages": [{ "role": "user", "content": prompt }],
            "modalities": ["image", "text"],
        });
        let json = send_json(
            self.label,
            client
                .post(&url)
                .header("Authorization", format!("Bearer {key}"))
                .header("HTTP-Referer", "https://cortex.study")
                .header("X-Title", "Cortex")
                .json(&body),
        )?;
        json["choices"][0]["message"]["images"][0]["image_url"]["url"]
            .as_str()
            .map(|s| s.to_string())
            .ok_or_else(|| Error::Other(format!("{}: model returned no image", self.label)))
    }
}

/// Anthropic Claude Messages API (BYOK).
pub struct ClaudeLlm {
    pub api_key: String,
    pub model: String,
    pub max_tokens: Option<u32>,
}

impl Llm for ClaudeLlm {
    fn set_max_tokens(&mut self, max: u32) {
        self.max_tokens = Some(max);
    }
    fn complete(&self, system: &str, user: &str) -> Result<String> {
        let client = llm_client();
        let body = serde_json::json!({
            "model": self.model,
            // Roomy enough for a full cheatsheet without cutting off mid-section.
            // Modern Claude models support far more (Sonnet 4.x: 64K, Opus: 128K
            // output tokens); 32K covers any cheatsheet while staying within limits.
            "max_tokens": self.max_tokens.unwrap_or(32000),
            "temperature": 0.3,
            "system": system,
            "messages": [{ "role": "user", "content": user }]
        });
        let json = send_json(
            "claude",
            client
                .post("https://api.anthropic.com/v1/messages")
                .header("x-api-key", &self.api_key)
                .header("anthropic-version", "2023-06-01")
                .json(&body),
        )?;
        let text = json["content"][0]["text"]
            .as_str()
            .ok_or_else(|| Error::Other("claude: empty response".into()))?
            .to_string();
        Ok(text)
    }
    fn name(&self) -> String {
        format!("claude:{}", self.model)
    }
}

/// API keys available from settings.
#[derive(Default)]
pub struct Keys {
    pub gemini: Option<String>,
    pub openrouter: Option<String>,
    pub openai: Option<String>,
    pub claude: Option<String>,
    pub custom_endpoint: Option<String>,
    /// Ollama base URL (e.g. http://localhost:11434) — local, keyless.
    pub ollama_url: Option<String>,
}

fn nonempty(o: &Option<String>) -> Option<&str> {
    o.as_deref().filter(|s| !s.is_empty())
}

/// Build an LLM from a "provider:model" spec + the available keys. Returns
/// `None` when no usable provider/key is configured — callers turn that into a
/// clear "add an API key" error instead of silently producing fake output.
pub fn from_spec(spec: &str, keys: &Keys) -> Option<Box<dyn Llm>> {
    // Trim — a stored spec with stray whitespace/newline would otherwise produce
    // an invalid model id (and thus a provider error).
    let spec = spec.trim();
    let (provider, model) = spec.split_once(':').unwrap_or(("gemini", spec));
    let model = model.trim().to_string();
    match provider {
        "gemini" => nonempty(&keys.gemini).map(|k| {
            Box::new(GeminiLlm { api_key: k.to_string(), model, max_tokens: None }) as Box<dyn Llm>
        }),
        "openrouter" => nonempty(&keys.openrouter).map(|k| {
            Box::new(OpenAiCompatLlm {
                base_url: "https://openrouter.ai/api/v1".into(),
                api_key: k.to_string(),
                model,
                label: "openrouter",
                max_tokens: None,
            }) as Box<dyn Llm>
        }),
        "openai" => nonempty(&keys.openai).map(|k| {
            Box::new(OpenAiCompatLlm {
                base_url: "https://api.openai.com/v1".into(),
                api_key: k.to_string(),
                model,
                label: "openai",
                max_tokens: None,
            }) as Box<dyn Llm>
        }),
        "claude" | "anthropic" => nonempty(&keys.claude).map(|k| {
            Box::new(ClaudeLlm { api_key: k.to_string(), model, max_tokens: None }) as Box<dyn Llm>
        }),
        "custom" => nonempty(&keys.custom_endpoint).map(|base| {
            Box::new(OpenAiCompatLlm {
                base_url: base.to_string(),
                api_key: nonempty(&keys.openai).unwrap_or("").to_string(),
                model,
                label: "custom",
                max_tokens: None,
            }) as Box<dyn Llm>
        }),
        // Ollama exposes an OpenAI-compatible API at <base>/v1; it's keyless, so
        // pass a dummy bearer token (Ollama ignores it).
        "ollama" => {
            let base = nonempty(&keys.ollama_url).unwrap_or("http://localhost:11434");
            Some(Box::new(OpenAiCompatLlm {
                base_url: format!("{}/v1", base.trim_end_matches('/')),
                api_key: "ollama".to_string(),
                model,
                label: "ollama",
                max_tokens: None,
            }) as Box<dyn Llm>)
        }
        _ => None,
    }
}

/// Like `from_spec`, but if the configured spec's provider has no key, fall back
/// to whichever provider DOES have a key. This lets generation work as soon as
/// ANY API key is set, even if a per-task model still points at an unkeyed
/// provider (the common "I added my OpenRouter key but cheatsheet still says no
/// model" case — cheatsheet defaulted to gemini).
pub fn from_spec_or_any(spec: &str, keys: &Keys) -> Option<Box<dyn Llm>> {
    if let Some(m) = from_spec(spec, keys) {
        return Some(m);
    }
    let fallback = if nonempty(&keys.openrouter).is_some() {
        "openrouter:openai/gpt-4o-mini"
    } else if nonempty(&keys.gemini).is_some() {
        "gemini:gemini-2.5-flash"
    } else if nonempty(&keys.openai).is_some() {
        "openai:gpt-4o-mini"
    } else if nonempty(&keys.claude).is_some() {
        "claude:claude-3-5-sonnet-20241022"
    } else if nonempty(&keys.custom_endpoint).is_some() {
        "custom:default"
    } else {
        return None;
    };
    from_spec(fallback, keys)
}

/// Extract the first JSON value (object or array) from an LLM reply that may be
/// wrapped in prose or ```json fences.
///
/// Robustness: we DON'T rely on a strict brace-matcher to find the closing
/// delimiter, because an unescaped `"` inside a rich-markdown string value
/// desyncs the in-string tracking and truncates the slice. Instead we bound the
/// JSON as `first opener .. last matching closer` (LLMs emit a single JSON value,
/// optionally followed by prose that rarely contains a stray `}`/`]`) and hand
/// the slice to `parse_lenient`, which repairs the common invalid-JSON shapes.
pub fn extract_json(text: &str) -> Result<serde_json::Value> {
    let t = text.trim();
    // strip a leading code fence
    let t = t.strip_prefix("```json").or_else(|| t.strip_prefix("```")).unwrap_or(t);
    let t = t.trim_start_matches("```").trim();

    let start = match t.find(['{', '[']) {
        Some(s) => s,
        None => return Err(Error::Other("no JSON found in LLM reply".into())),
    };
    let open = t.as_bytes()[start] as char;
    let close = if open == '{' { '}' } else { ']' };
    let end = t.rfind(close).filter(|&e| e > start);
    if let Some(end) = end {
        let slice = &t[start..=end];
        if let Ok(v) = parse_lenient(slice) {
            return Ok(v);
        }
    }
    // Fallback: repair from the opener to the end of the text (handles a missing
    // closer or trailing-prose confusion).
    parse_lenient(&t[start..])
}

/// Parse a JSON slice, retrying once with a repair pass. LLMs producing rich
/// content (tables, callouts, multi-line explanations) routinely emit invalid
/// JSON inside string values — LITERAL newlines/tabs and UNESCAPED double quotes
/// — which was the cause of "Model returned unstructured output" for rich
/// cheatsheets. The repair pass fixes both.
fn parse_lenient(slice: &str) -> Result<serde_json::Value> {
    match serde_json::from_str(slice) {
        Ok(v) => Ok(v),
        Err(_) => Ok(serde_json::from_str(&repair_json(slice))?),
    }
}

/// Repair the two invalid-JSON shapes LLMs emit inside string literals:
///   1. Raw control characters (newline, CR, tab, other <0x20) → escaped.
///   2. Unescaped interior double quotes → escaped to `\"`.
/// For (2) a `"` is treated as the string's CLOSER only when the next
/// non-whitespace character is structural (`:` `,` `}` `]`) or end-of-input;
/// otherwise it's interior content and gets escaped. Already-escaped sequences
/// and structure outside strings are left untouched. The heuristic can't cover a
/// quoted phrase that ends immediately before a comma, but it fixes the common
/// cases and is strictly more tolerant than escaping control chars alone.
fn repair_json(s: &str) -> String {
    let chars: Vec<char> = s.chars().collect();
    let n = chars.len();
    let mut out = String::with_capacity(s.len() + 16);
    let mut in_str = false;
    let mut esc = false;
    let mut i = 0;
    while i < n {
        let ch = chars[i];
        if in_str {
            if esc {
                out.push(ch);
                esc = false;
            } else if ch == '\\' {
                out.push(ch);
                esc = true;
            } else if ch == '"' {
                let mut j = i + 1;
                while j < n && chars[j].is_whitespace() {
                    j += 1;
                }
                let closes = match chars.get(j) {
                    None => true,
                    Some(&c) => matches!(c, ':' | ',' | '}' | ']'),
                };
                if closes {
                    out.push('"');
                    in_str = false;
                } else {
                    out.push_str("\\\"");
                }
            } else {
                match ch {
                    '\n' => out.push_str("\\n"),
                    '\r' => out.push_str("\\r"),
                    '\t' => out.push_str("\\t"),
                    c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
                    c => out.push(c),
                }
            }
        } else {
            if ch == '"' {
                in_str = true;
            }
            out.push(ch);
        }
        i += 1;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_json_from_fenced_prose() {
        let reply = "Sure! Here it is:\n```json\n{\"sections\":[{\"title\":\"Defs\",\"items\":[]}]}\n```\nDone.";
        let v = extract_json(reply).unwrap();
        assert_eq!(v["sections"][0]["title"], "Defs");
    }

    #[test]
    fn extract_json_array() {
        let v = extract_json("[{\"q\":\"a\",\"a\":\"b\"}]").unwrap();
        assert_eq!(v[0]["q"], "a");
    }

    #[test]
    fn extract_json_fenced_array_with_trailing_prose() {
        // The common "wrapped in ```json + closing prose" failure shape.
        let reply = "```json\n[{\"q\":\"x\",\"a\":\"y\"}]\n```\nHope this helps!";
        let v = extract_json(reply).unwrap();
        assert_eq!(v[0]["a"], "y");
    }

    #[test]
    fn extract_json_repairs_literal_newlines_in_strings() {
        // Rich cheatsheet content: a `d` value with LITERAL newlines + tab (invalid
        // JSON until repaired). This previously produced "unstructured output".
        let reply = "{\"sections\":[{\"title\":\"T\",\"items\":[{\"t\":\"Term\",\"d\":\"Line one\n> [!NOTE]\nLine two\twith tab\"}]}]}";
        let v = extract_json(reply).unwrap();
        assert_eq!(v["sections"][0]["items"][0]["t"], "Term");
        assert!(v["sections"][0]["items"][0]["d"]
            .as_str()
            .unwrap()
            .contains("NOTE"));
    }

    // Live diagnostic (ignored by default). Run with the real key to see exactly
    // what the OpenRouter call returns through the app's own client:
    //   OPENROUTER_KEY=sk-or-... cargo test --lib live_openrouter -- --ignored --nocapture
    #[test]
    #[ignore]
    fn live_openrouter() {
        let key = std::env::var("OPENROUTER_KEY").expect("set OPENROUTER_KEY");
        let llm = OpenAiCompatLlm {
            base_url: "https://openrouter.ai/api/v1".into(),
            api_key: key,
            model: "google/gemini-2.5-flash".into(),
            label: "openrouter",
            max_tokens: None,
        };
        match llm.complete("You output JSON only.", "Return {\"ok\":true} and nothing else.") {
            Ok(t) => println!("LIVE OK: {}", truncate(&t, 200)),
            Err(e) => println!("LIVE ERR: {e}"),
        }
    }

    #[test]
    fn extract_json_repairs_unescaped_interior_quotes() {
        // Rich cheatsheet content where the model forgot to escape interior
        // quotes — the real production cause of "unstructured output".
        let reply = "{\"sections\":[{\"title\":\"Defs\",\"items\":[{\"t\":\"Place\",\"d\":\"A \"place\" is a location with meaning.\"}]}]}";
        let v = extract_json(reply).unwrap();
        assert_eq!(v["sections"][0]["items"][0]["t"], "Place");
        assert!(v["sections"][0]["items"][0]["d"]
            .as_str()
            .unwrap()
            .contains("place"));
    }

    #[test]
    fn extract_json_repairs_quotes_and_newlines_together() {
        // Both failure shapes at once inside one rich body.
        let reply = "Here:\n{\"sections\":[{\"title\":\"T\",\"items\":[{\"t\":\"X\",\"d\":\"Line one\nHe said \"hi\" here\"}]}]}\nDone.";
        let v = extract_json(reply).unwrap();
        let d = v["sections"][0]["items"][0]["d"].as_str().unwrap();
        assert!(d.contains("hi"));
        assert!(d.contains("Line one"));
    }

    #[test]
    fn stub_is_offline_safe() {
        let s = StubLlm;
        let out = s.complete("sys", "hello world").unwrap();
        assert!(out.contains("Offline draft"));
    }
}
