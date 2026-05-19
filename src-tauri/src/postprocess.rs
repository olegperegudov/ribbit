//! Optional LLM post-processing of transcribed text via an OpenAI-compatible
//! chat-completions endpoint (RouterAI, OpenAI, OpenRouter, ...).
//!
//! Pipeline role: replaces the strict `vocab::apply` step when enabled.
//! The vocabulary is passed to the model as part of the system prompt so it
//! can fix both exact aliases and obvious misheard variants by context (e.g.
//! «Роса» → «Алроса» when "Алроса" is in vocab).
//!
//! On any error/timeout the caller falls back to plain `vocab::apply` so the
//! user is never blocked beyond the 3s timeout.

use std::collections::HashMap;
use std::sync::OnceLock;

/// Connection + defaults for one OpenAI-compatible LLM endpoint.
pub struct ProviderConfig {
    pub name: &'static str,
    pub env_var: &'static str,
    pub label: &'static str,
    pub base_url: &'static str,
    pub default_model: &'static str,
}

/// All providers Ribbit currently knows about. Order matches the UI dropdown.
pub const PROVIDERS: &[ProviderConfig] = &[
    ProviderConfig {
        name: "routerai",
        env_var: "ROUTERAI_API_KEY",
        label: "routerai",
        base_url: "https://routerai.ru/api/v1/chat/completions",
        default_model: "google/gemma-4-26b-a4b-it",
    },
    ProviderConfig {
        name: "openai",
        env_var: "OPENAI_API_KEY",
        label: "openai",
        base_url: "https://api.openai.com/v1/chat/completions",
        default_model: "gpt-4o-mini",
    },
    ProviderConfig {
        name: "openrouter",
        env_var: "OPENROUTER_API_KEY",
        label: "openrouter",
        base_url: "https://openrouter.ai/api/v1/chat/completions",
        default_model: "google/gemini-2.0-flash-001",
    },
];

pub const DEFAULT_PROVIDER: &str = "routerai";

// 5s gives the model headroom for occasional 3-4s responses while still
// keeping the paste latency bearable. Worst-case with one retry: ~10s.
const TIMEOUT_SECS: u64 = 5;

pub fn find_provider(name: &str) -> Option<&'static ProviderConfig> {
    PROVIDERS.iter().find(|p| p.name == name)
}

/// Vocab section appended to the system prompt. Empty when no vocab — keeps
/// the prompt minimal for users who don't need this feature.
fn vocab_section(vocab: &HashMap<String, Vec<String>>) -> String {
    if vocab.is_empty() {
        return String::new();
    }
    // Stable order so the prompt is deterministic across calls.
    let mut keys: Vec<&String> = vocab.keys().collect();
    keys.sort();
    let mut s = String::from("\n\nСловарь поправок. Слева — правильное написание, справа — варианты, как речь могла быть распознана с ошибкой:\n");
    for k in keys {
        let aliases = &vocab[k];
        if aliases.is_empty() {
            s.push_str(&format!("- {}\n", k));
        } else {
            s.push_str(&format!("- {} ← {}\n", k, aliases.join(", ")));
        }
    }
    s.push_str("Используй словарь как подсказку: подставляй правильное написание не только при точном совпадении с алиасом, но и при очевидных искажениях по смыслу (например, если в контексте речь явно про слово из словаря — заменяй, даже если форма отличается). Не подставляй когда смысл не подходит.");
    s
}

/// System prompt for the editor. Pinned by snapshot test so we change it
/// intentionally. With vocab the prompt grows by a list + one instruction.
pub fn system_prompt(vocab: &HashMap<String, Vec<String>>) -> String {
    let base = "Ты редактор русского текста. На вход — фраза, распознанная из речи. \
Задача: исправь орфографию, расставь пунктуацию, приведи англицизмы \
к привычному кириллическому написанию там где так общепринято \
(например \"девопс\" а не \"DevOps\" в обычной речи). Не меняй смысл, \
не добавляй ничего от себя, не пиши комментариев. Верни ТОЛЬКО \
исправленный текст одной строкой без префиксов и кавычек.";
    format!("{}{}", base, vocab_section(vocab))
}

/// Build the JSON request body. Deterministic — covered by unit tests.
pub fn build_payload(text: &str, vocab: &HashMap<String, Vec<String>>, model: &str) -> serde_json::Value {
    serde_json::json!({
        "model": model,
        "messages": [
            {"role": "system", "content": system_prompt(vocab)},
            {"role": "user", "content": text}
        ],
        "temperature": 0.0,
        "max_tokens": 512,
    })
}

/// Extract message content from an OpenAI-style chat-completion response,
/// stripping common LLM-isms (surrounding quotes).
pub fn parse_response(json: &serde_json::Value) -> Result<String, String> {
    let content = json
        .get("choices")
        .and_then(|c| c.get(0))
        .and_then(|c| c.get("message"))
        .and_then(|m| m.get("content"))
        .and_then(|c| c.as_str())
        .ok_or_else(|| "missing choices[0].message.content".to_string())?;

    let cleaned = clean_content(content);
    if cleaned.is_empty() {
        return Err("empty content".into());
    }
    Ok(cleaned)
}

fn clean_content(s: &str) -> String {
    let mut t = s.trim().to_string();

    // Strip wrapping quotes if the LLM returned them despite the prompt
    // ("..." or «...»). Anything more aggressive (label-stripping by `:`)
    // is intentionally NOT done here — too easy to chop real sentence content.
    // If the LLM consistently prepends labels, fix the prompt, not the parser.
    let pairs = [('"', '"'), ('\'', '\''), ('«', '»'), ('“', '”')];
    for (open, close) in pairs {
        if t.starts_with(open) && t.ends_with(close) && t.chars().count() >= 2 {
            t = t
                .strip_prefix(open)
                .unwrap_or(&t)
                .strip_suffix(close)
                .unwrap_or(&t)
                .trim()
                .to_string();
            break;
        }
    }

    t
}

/// Short label for the kind of reqwest error — useful in debug log when
/// diagnosing why postprocess fell back. Native error text is verbose and
/// often hides the actual class (timeout vs connect vs TLS, etc).
fn error_kind(e: &reqwest::Error) -> &'static str {
    if e.is_timeout() { "timeout" }
    else if e.is_connect() { "connect" }
    else if e.is_request() { "request" }
    else if e.is_body() { "body" }
    else if e.is_decode() { "decode" }
    else { "other" }
}

static HTTP_CLIENT: OnceLock<reqwest::blocking::Client> = OnceLock::new();

fn client() -> &'static reqwest::blocking::Client {
    HTTP_CLIENT.get_or_init(|| {
        reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(TIMEOUT_SECS))
            .build()
            .expect("failed to build postprocess HTTP client")
    })
}

/// Call the configured provider with the text + vocab. Returns the cleaned
/// content on success. Caller is responsible for falling back on error.
pub fn edit_text(
    text: &str,
    vocab: &HashMap<String, Vec<String>>,
    provider: &ProviderConfig,
    api_key: &str,
) -> Result<String, String> {
    if text.trim().is_empty() {
        return Ok(text.to_string());
    }
    if api_key.is_empty() {
        return Err(format!("no {} api key", provider.name));
    }

    let t0 = std::time::Instant::now();
    let payload = build_payload(text, vocab, provider.default_model);

    // Single retry on any send() error: pooled TLS connections occasionally
    // go stale between dictations and reqwest reports a generic transport
    // error. Chat completion is idempotent, so a duplicate POST is safe.
    let send_once = || {
        client()
            .post(provider.base_url)
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
    };
    let response = match send_once() {
        Ok(r) => r,
        Err(first) => {
            crate::debug_log::log(&format!(
                "postprocess retry after {} ({})",
                error_kind(&first),
                first
            ));
            send_once().map_err(|e| {
                format!("{} after retry: {}", error_kind(&e), e)
            })?
        }
    };

    let elapsed = t0.elapsed();

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().unwrap_or_default();
        return Err(format!("http {}: {}", status, body.chars().take(200).collect::<String>()));
    }

    let json: serde_json::Value = response
        .json()
        .map_err(|e| format!("parse error: {}", e))?;

    let edited = parse_response(&json)?;
    crate::debug_log::log(&format!(
        "postprocess[{}/{}]: {:?} → {:?} ({:.2}s)",
        provider.name,
        provider.default_model,
        text.chars().take(60).collect::<String>(),
        edited.chars().take(60).collect::<String>(),
        elapsed.as_secs_f32()
    ));
    Ok(edited)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn empty_vocab() -> HashMap<String, Vec<String>> {
        HashMap::new()
    }

    fn sample_vocab() -> HashMap<String, Vec<String>> {
        let mut v = HashMap::new();
        v.insert("Алроса".into(), vec!["роса".into(), "алроза".into()]);
        v.insert("девопс".into(), vec!["дев опс".into()]);
        v
    }

    #[test]
    fn providers_table_well_formed() {
        // Each provider has unique name and non-empty fields. UI relies on this.
        let mut seen = std::collections::HashSet::new();
        for p in PROVIDERS {
            assert!(seen.insert(p.name), "duplicate provider name: {}", p.name);
            assert!(!p.env_var.is_empty());
            assert!(p.base_url.starts_with("https://"));
            assert!(!p.default_model.is_empty());
            assert!(!p.label.is_empty());
        }
        assert!(find_provider(DEFAULT_PROVIDER).is_some(), "default provider must be in PROVIDERS");
    }

    #[test]
    fn find_provider_unknown() {
        assert!(find_provider("nonesuch").is_none());
    }

    #[test]
    fn system_prompt_snapshot_no_vocab() {
        let p = system_prompt(&empty_vocab());
        assert!(p.contains("редактор русского текста"));
        assert!(p.contains("Верни ТОЛЬКО"));
        assert!(p.contains("англицизмы"));
        assert!(!p.contains("Словарь"));
    }

    #[test]
    fn system_prompt_snapshot_with_vocab() {
        let p = system_prompt(&sample_vocab());
        assert!(p.contains("Словарь поправок"));
        assert!(p.contains("- Алроса ← роса, алроза"));
        assert!(p.contains("- девопс ← дев опс"));
        assert!(p.contains("Используй словарь как подсказку"));
    }

    #[test]
    fn system_prompt_vocab_is_deterministic() {
        // Two builds with same vocab must produce identical prompt
        let a = system_prompt(&sample_vocab());
        let b = system_prompt(&sample_vocab());
        assert_eq!(a, b);
    }

    #[test]
    fn build_payload_has_required_fields() {
        let p = build_payload("привет", &empty_vocab(), "google/gemma-4-26b-a4b-it");
        assert_eq!(p["model"], "google/gemma-4-26b-a4b-it");
        assert_eq!(p["temperature"], 0.0);
        assert_eq!(p["max_tokens"], 512);
        assert_eq!(p["messages"][0]["role"], "system");
        assert_eq!(p["messages"][1]["role"], "user");
        assert_eq!(p["messages"][1]["content"], "привет");
    }

    #[test]
    fn build_payload_includes_vocab_in_system_message() {
        let p = build_payload("ехал к росе утром", &sample_vocab(), "x");
        let sys = p["messages"][0]["content"].as_str().unwrap();
        assert!(sys.contains("Алроса"), "vocab key missing from system prompt");
    }

    #[test]
    fn parse_response_happy_path() {
        let r = serde_json::json!({
            "choices": [{"message": {"content": "Привет, мир."}}]
        });
        assert_eq!(parse_response(&r).unwrap(), "Привет, мир.");
    }

    #[test]
    fn parse_response_missing_choices() {
        let r = serde_json::json!({"error": "oops"});
        assert!(parse_response(&r).is_err());
    }

    #[test]
    fn parse_response_empty_content() {
        let r = serde_json::json!({"choices": [{"message": {"content": ""}}]});
        assert!(parse_response(&r).is_err());
    }

    #[test]
    fn parse_response_strips_double_quotes() {
        let r = serde_json::json!({
            "choices": [{"message": {"content": "\"Привет, мир.\""}}]
        });
        assert_eq!(parse_response(&r).unwrap(), "Привет, мир.");
    }

    #[test]
    fn parse_response_strips_guillemets() {
        let r = serde_json::json!({
            "choices": [{"message": {"content": "«Привет, мир.»"}}]
        });
        assert_eq!(parse_response(&r).unwrap(), "Привет, мир.");
    }

    #[test]
    fn parse_response_keeps_content_with_colons_intact() {
        // Regression: an earlier overly-eager "label stripping" heuristic
        // chopped off everything before the first ':', destroying real content.
        // Whatever the LLM returns inside the sentence stays untouched.
        let r = serde_json::json!({
            "choices": [{"message": {"content": "Сегодня я сделал следующее: купил хлеб и молоко."}}]
        });
        let out = parse_response(&r).unwrap();
        assert_eq!(out, "Сегодня я сделал следующее: купил хлеб и молоко.");
    }

    #[test]
    fn edit_text_returns_input_for_empty() {
        let p = find_provider("routerai").unwrap();
        assert_eq!(edit_text("", &empty_vocab(), p, "fake_key").unwrap(), "");
        assert_eq!(edit_text("   ", &empty_vocab(), p, "fake_key").unwrap(), "   ");
    }

    #[test]
    fn edit_text_errors_without_key() {
        let p = find_provider("routerai").unwrap();
        assert!(edit_text("hello", &empty_vocab(), p, "").is_err());
    }
}
