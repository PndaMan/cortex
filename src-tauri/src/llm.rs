//! LLM providers behind one trait, mirroring embed.rs. Default is an offline
//! `stub` that returns a clearly-marked placeholder so the app works with zero
//! config; Gemini `generateContent` (BYOK) is the real path. Per-task model is
//! read from settings as `model_<task>` = "provider:model" (e.g. "gemini:gemini-2.5-flash").

use crate::error::{Error, Result};

pub trait Llm: Send + Sync {
    fn complete(&self, system: &str, user: &str) -> Result<String>;
    fn name(&self) -> String;
}

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
}

impl Llm for GeminiLlm {
    fn complete(&self, system: &str, user: &str) -> Result<String> {
        let client = reqwest::blocking::Client::new();
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            self.model, self.api_key
        );
        // Gemini 2.5 models reason with hidden "thinking" tokens that draw from
        // the SAME maxOutputTokens budget. A small budget (4096) is routinely
        // exhausted by thinking before any answer text is emitted, leaving an
        // empty `parts[0].text` and a MAX_TOKENS finishReason — the real cause
        // of "generation doesn't work at all" for cheatsheet/quiz/flashcards.
        // A roomier budget lets the model finish the actual JSON.
        let body = serde_json::json!({
            "system_instruction": { "parts": [{ "text": system }] },
            "contents": [{ "role": "user", "parts": [{ "text": user }] }],
            "generationConfig": { "temperature": 0.3, "maxOutputTokens": 8192 }
        });
        let resp = client.post(&url).json(&body).send()?;
        if !resp.status().is_success() {
            let code = resp.status();
            let txt = resp.text().unwrap_or_default();
            return Err(Error::Other(format!("gemini {code}: {}", truncate(&txt, 300))));
        }
        let json: serde_json::Value = resp.json()?;
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
}

fn truncate(s: &str, n: usize) -> String {
    s.chars().take(n).collect()
}

/// OpenAI-compatible chat provider — used for OpenRouter and OpenAI (and any
/// custom OpenAI-compatible gateway). One code path, different base URL/key.
pub struct OpenAiCompatLlm {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub label: &'static str,
}

impl Llm for OpenAiCompatLlm {
    fn complete(&self, system: &str, user: &str) -> Result<String> {
        let key = self.api_key.trim();
        if key.is_empty() {
            return Err(Error::Other(format!(
                "{}: API key is empty — paste it in Settings → API keys and click Save keys.",
                self.label
            )));
        }
        let client = reqwest::blocking::Client::new();
        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
        let body = serde_json::json!({
            "model": self.model,
            "messages": [
                { "role": "system", "content": system },
                { "role": "user", "content": user }
            ],
            "temperature": 0.3
        });
        let resp = client
            .post(&url)
            .header("Authorization", format!("Bearer {key}"))
            // OpenRouter likes these; harmless elsewhere.
            .header("HTTP-Referer", "https://cortex.study")
            .header("X-Title", "Cortex")
            .json(&body)
            .send()?;
        if !resp.status().is_success() {
            let code = resp.status();
            let txt = resp.text().unwrap_or_default();
            // key-length is a non-secret diagnostic: distinguishes "key empty"
            // (length 0) from "key present but rejected" for 401s.
            return Err(Error::Other(format!(
                "{} {code} [{}:{} chars]: {}",
                self.label, self.model, key.len(), truncate(&txt, 300)
            )));
        }
        let json: serde_json::Value = resp.json()?;
        let text = json["choices"][0]["message"]["content"]
            .as_str()
            .ok_or_else(|| Error::Other(format!("{}: empty response", self.label)))?
            .to_string();
        Ok(text)
    }
    fn name(&self) -> String {
        format!("{}:{}", self.label, self.model)
    }
}

/// Anthropic Claude Messages API (BYOK).
pub struct ClaudeLlm {
    pub api_key: String,
    pub model: String,
}

impl Llm for ClaudeLlm {
    fn complete(&self, system: &str, user: &str) -> Result<String> {
        let client = reqwest::blocking::Client::new();
        let body = serde_json::json!({
            "model": self.model,
            "max_tokens": 4096,
            "temperature": 0.3,
            "system": system,
            "messages": [{ "role": "user", "content": user }]
        });
        let resp = client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()?;
        if !resp.status().is_success() {
            let code = resp.status();
            let txt = resp.text().unwrap_or_default();
            return Err(Error::Other(format!("claude {code}: {}", truncate(&txt, 300))));
        }
        let json: serde_json::Value = resp.json()?;
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
            Box::new(GeminiLlm { api_key: k.to_string(), model }) as Box<dyn Llm>
        }),
        "openrouter" => nonempty(&keys.openrouter).map(|k| {
            Box::new(OpenAiCompatLlm {
                base_url: "https://openrouter.ai/api/v1".into(),
                api_key: k.to_string(),
                model,
                label: "openrouter",
            }) as Box<dyn Llm>
        }),
        "openai" => nonempty(&keys.openai).map(|k| {
            Box::new(OpenAiCompatLlm {
                base_url: "https://api.openai.com/v1".into(),
                api_key: k.to_string(),
                model,
                label: "openai",
            }) as Box<dyn Llm>
        }),
        "claude" | "anthropic" => nonempty(&keys.claude).map(|k| {
            Box::new(ClaudeLlm { api_key: k.to_string(), model }) as Box<dyn Llm>
        }),
        "custom" => nonempty(&keys.custom_endpoint).map(|base| {
            Box::new(OpenAiCompatLlm {
                base_url: base.to_string(),
                api_key: nonempty(&keys.openai).unwrap_or("").to_string(),
                model,
                label: "custom",
            }) as Box<dyn Llm>
        }),
        // ollama lands in a later slice.
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
pub fn extract_json(text: &str) -> Result<serde_json::Value> {
    let t = text.trim();
    // strip code fences
    let t = t.strip_prefix("```json").or_else(|| t.strip_prefix("```")).unwrap_or(t);
    let t = t.trim_start_matches("```").trim();
    // find the first { or [ and matching close
    let start = t.find(['{', '[']);
    if let Some(s) = start {
        let open = t.as_bytes()[s] as char;
        let close = if open == '{' { '}' } else { ']' };
        let mut depth = 0i32;
        let mut in_str = false;
        let mut esc = false;
        for (i, ch) in t[s..].char_indices() {
            if in_str {
                if esc {
                    esc = false;
                } else if ch == '\\' {
                    esc = true;
                } else if ch == '"' {
                    in_str = false;
                }
                continue;
            }
            match ch {
                '"' => in_str = true,
                c if c == open => depth += 1,
                c if c == close => {
                    depth -= 1;
                    if depth == 0 {
                        let slice = &t[s..s + i + ch.len_utf8()];
                        return Ok(serde_json::from_str(slice)?);
                    }
                }
                _ => {}
            }
        }
    }
    Err(Error::Other("no JSON found in LLM reply".into()))
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
    fn stub_is_offline_safe() {
        let s = StubLlm;
        let out = s.complete("sys", "hello world").unwrap();
        assert!(out.contains("Offline draft"));
    }
}
