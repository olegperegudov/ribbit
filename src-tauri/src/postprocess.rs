//! Optional LLM post-processing of transcribed text via RouterAI.
//!
//! Lives after `vocab::apply` in the pipeline, before the log entry is emitted.
//! Disabled by default. When enabled and a key is present, runs a small,
//! fast model (gemma-4-26b-a4b-it, ~150ms) to fix punctuation, spelling,
//! and anglicisms. On any error/timeout we silently fall back to the
//! vocab-processed text — never block the user.

use std::sync::OnceLock;

/// Default model. Same one wiki_updater uses. ~150ms typical latency.
pub const DEFAULT_MODEL: &str = "google/gemma-4-26b-a4b-it";
const API_URL: &str = "https://routerai.ru/api/v1/chat/completions";
const TIMEOUT_SECS: u64 = 3;

/// System prompt — pinned by snapshot test so we change it intentionally.
pub fn system_prompt() -> &'static str {
    "Ты редактор русского текста. На вход — фраза, распознанная из речи. \
Задача: исправь орфографию, расставь пунктуацию, приведи англицизмы \
к привычному кириллическому написанию там где так общепринято \
(например \"девопс\" а не \"DevOps\" в обычной речи). Не меняй смысл, \
не добавляй ничего от себя, не пиши комментариев. Верни ТОЛЬКО \
исправленный текст одной строкой без префиксов и кавычек."
}

/// Build the JSON request body. Deterministic — covered by unit tests.
pub fn build_payload(text: &str, model: &str) -> serde_json::Value {
    serde_json::json!({
        "model": model,
        "messages": [
            {"role": "system", "content": system_prompt()},
            {"role": "user", "content": text}
        ],
        "temperature": 0.0,
        "max_tokens": 512,
    })
}

/// Extract message content from an OpenAI-style chat-completion response,
/// stripping common LLM-isms (surrounding quotes, "Исправленный текст:" prefix).
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

static HTTP_CLIENT: OnceLock<reqwest::blocking::Client> = OnceLock::new();

fn client() -> &'static reqwest::blocking::Client {
    HTTP_CLIENT.get_or_init(|| {
        reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(TIMEOUT_SECS))
            .build()
            .expect("failed to build postprocess HTTP client")
    })
}

/// Call RouterAI with the given text. Returns the cleaned content on success.
/// Caller is responsible for falling back to the original text on error.
pub fn edit_text(text: &str, api_key: &str) -> Result<String, String> {
    if text.trim().is_empty() {
        return Ok(text.to_string());
    }
    if api_key.is_empty() {
        return Err("no routerai api key".into());
    }

    let t0 = std::time::Instant::now();
    let payload = build_payload(text, DEFAULT_MODEL);

    let response = client()
        .post(API_URL)
        .header("Authorization", format!("Bearer {}", api_key))
        .header("Content-Type", "application/json")
        .json(&payload)
        .send()
        .map_err(|e| format!("network error: {}", e))?;

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
        "postprocess: {:?} → {:?} ({:.2}s)",
        text.chars().take(60).collect::<String>(),
        edited.chars().take(60).collect::<String>(),
        elapsed.as_secs_f32()
    ));
    Ok(edited)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_prompt_snapshot() {
        // Pin the prompt — change here = visible diff in PR.
        assert!(system_prompt().contains("редактор русского текста"));
        assert!(system_prompt().contains("Верни ТОЛЬКО"));
        assert!(system_prompt().contains("англицизмы"));
    }

    #[test]
    fn build_payload_has_required_fields() {
        let p = build_payload("привет", "google/gemma-4-26b-a4b-it");
        assert_eq!(p["model"], "google/gemma-4-26b-a4b-it");
        assert_eq!(p["temperature"], 0.0);
        assert_eq!(p["max_tokens"], 512);
        assert_eq!(p["messages"][0]["role"], "system");
        assert_eq!(p["messages"][1]["role"], "user");
        assert_eq!(p["messages"][1]["content"], "привет");
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
        // Empty input — no network call, return as-is
        assert_eq!(edit_text("", "fake_key").unwrap(), "");
        assert_eq!(edit_text("   ", "fake_key").unwrap(), "   ");
    }

    #[test]
    fn edit_text_errors_without_key() {
        assert!(edit_text("hello", "").is_err());
    }
}
