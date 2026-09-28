//! Recorded speech → finished text: everything Ribbit does between the key
//! release and the paste. Shared by the desktop app (which records and types)
//! and `ribbit-server` (which receives a recording over the network and hands
//! the text back), so a dictation reads the same wherever it was spoken.

use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::{debug_log, fallback, hallucinations, postprocess, transcribe, vocab};

/// One finished dictation plus what it took to get there — the caller logs it.
pub struct Transcript {
    pub text: String,
    /// Speech-to-text output before the editor; kept for the log when the
    /// editor ran, so an over-eager edit can be traced back.
    pub raw_text: String,
    pub edited: bool,
    pub llm_attempted: bool,
    pub stt_secs: f32,
    pub stt_model: String,
    pub llm_secs: Option<f32>,
    pub llm_model: Option<String>,
    pub llm_host: Option<String>,
    /// Why this dictation came back unedited, in the user's words. The
    /// yellow dot alone only said "the editor didn't run"; the whole
    /// question a user has at that moment is whether to wait it out
    /// (rate limit, provider down) or go fix something (no key, bad
    /// model). Travels with the entry — event and daily log both — so
    /// the answer is still there after a restart.
    pub llm_error: Option<&'static str>,
}

/// Last LLM post-process failure, surfaced in Settings. Without this the
/// feature rots silently: a provider retires a model id, every call 404s, the
/// code falls back to plain vocab, and the user just sees "the LLM does
/// nothing" with no clue why. Cleared on the next successful edit.
static LAST_LLM_ERROR: Mutex<Option<String>> = Mutex::new(None);

fn set_last_llm_error(e: Option<String>) {
    if let Ok(mut g) = LAST_LLM_ERROR.lock() {
        *g = e;
    }
}

pub fn last_llm_error() -> Option<String> {
    LAST_LLM_ERROR.lock().ok().and_then(|g| g.clone())
}

/// Endpoint host of a stack entry, e.g. "routerai.ru".
pub fn entry_host(e: &fallback::ProviderEntry) -> &str {
    e.url.split('/').nth(2).unwrap_or("?")
}

/// "provider/model" label for the transcription log; label falls back to the
/// endpoint host for custom entries.
pub fn entry_label(e: &fallback::ProviderEntry) -> String {
    let prov = if e.label.is_empty() { entry_host(e) } else { e.label.as_str() };
    format!("{}/{}", prov, e.model)
}

/// Transcribe, clean and (if enabled) edit one recording. `Err` carries the
/// user-facing STT failure — nothing was recognised, there is no text to keep.
pub fn run(
    audio_data: &[f32],
    sample_rate: u32,
    languages: &[String],
    cfg: &serde_json::Value,
) -> Result<Transcript, String> {
    // STT with in-request failover: walk the audio stack from the sticky
    // active entry, so a transient failure (429/5xx/timeout) tries the
    // next provider for THIS dictation — speech is never lost just because
    // the primary blinked. Hard errors (bad key/url/model) still surface.
    let t_stt = Instant::now();
    let entries = fallback::read_stack(cfg, fallback::Stack::Audio);
    if entries.is_empty() {
        return Err("No audio provider configured. Add one in Settings.".into());
    }
    let start = fallback::active_index(fallback::Stack::Audio, fallback::cooldown(cfg))
        .min(entries.len() - 1);
    let (raw_text, used) = fallback::run_with_failover(
        // No budget: a dropped dictation can't be recovered, so the
        // audio stack is allowed to wait the network out.
        fallback::Stack::Audio, &entries, start, fallback::threshold(cfg), None,
        |e, key| transcribe::transcribe_audio_blocking(
            audio_data, sample_rate, languages, &e.url, key, &e.model,
        ),
    )
    .map_err(|e| e.message)?;
    let stt_model = entry_label(&entries[used]);
    let stt_secs = t_stt.elapsed().as_secs_f32();

    // Whisper hallucinates "Продолжение следует..." (and kin) on
    // silence — cut it off the raw text before any downstream pass,
    // which would otherwise preserve it verbatim.
    let raw_text = {
        let stripped = hallucinations::strip(&raw_text);
        if stripped != raw_text {
            debug_log::log(&format!(
                "hallucination stripped: {:?} → {:?}", raw_text, stripped
            ));
        }
        stripped
    };
    // Pipeline: if LLM post-processing is enabled we send raw text +
    // vocab to the model (it handles both punctuation and vocab
    // mapping with context). Otherwise — strict vocab::apply.
    // On LLM error we fall back to strict vocab::apply. Like STT,
    // the edit walks the text stack within this request; entries
    // without a key are skipped. Unlike STT the walk is capped by a
    // time budget — the transcript is already safe, so a sick
    // network must not hold the paste hostage.
    let postprocess_enabled = cfg["postprocess_enabled"].as_bool().unwrap_or(false);

    let mut llm_secs: Option<f32> = None;
    let mut llm_model: Option<String> = None;
    let mut llm_host: Option<String> = None;
    let mut llm_attempted = false;
    let mut llm_error: Option<&'static str> = None;

    let text_entries = fallback::read_stack(cfg, fallback::Stack::Text);
    let any_text_key = text_entries
        .iter()
        .any(|e| std::env::var(&e.key_env).map(|k| !k.is_empty()).unwrap_or(false));
    let (text, edited): (String, bool) = if postprocess_enabled && !text_entries.is_empty() && !raw_text.trim().is_empty() {
        if !any_text_key {
            let msg = "no key set for any text provider".to_string();
            debug_log::log(&format!("postprocess: {} — falling back to strict vocab", msg));
            set_last_llm_error(Some(msg));
            llm_error = Some("no key set");
            (vocab::apply(&raw_text), false)
        } else {
            let vocab_data = vocab::read_vocab();
            llm_attempted = true;
            let start = fallback::active_index(fallback::Stack::Text, fallback::cooldown(cfg))
                .min(text_entries.len() - 1);
            // Timed even on failure: a timed-out LLM burns its full
            // timeout before we fall back to vocab, and that lost
            // time must show up in the log.
            let t_llm = Instant::now();
            let outcome = fallback::run_with_failover(
                fallback::Stack::Text, &text_entries, start, fallback::threshold(cfg),
                // The transcript is already in hand; the edit is worth
                // a bounded wait, never an open-ended one.
                Some(Duration::from_secs(postprocess::STACK_BUDGET_SECS)),
                |e, key| postprocess::edit_text(&raw_text, &e.url, key, &e.model),
            );
            llm_secs = Some(t_llm.elapsed().as_secs_f32());
            match outcome {
                // Clearing on success means the Settings note only ever
                // reflects the *current* state, not a stale failure.
                Ok((edited_text, used)) => {
                    // Host (not the label) so the history shows the
                    // real endpoint that ran the edit — including
                    // which fallback rung it was.
                    let e = &text_entries[used];
                    llm_host = Some(entry_host(e).to_string());
                    llm_model = Some(e.model.clone());
                    set_last_llm_error(None);
                    // Terms are owned by this deterministic pass, not
                    // the model: the editor only punctuates and fixes
                    // ordinary spelling. The strict pass then maps every
                    // exact alias to its canonical term — the mandatory
                    // table a model can't be trusted to apply (it either
                    // skipped aliases or invented its own "corrections").
                    (vocab::apply_with(&edited_text, &vocab_data), true)
                }
                Err(err) => {
                    debug_log::log(&format!("postprocess failed ({}) — falling back to strict vocab", err.message));
                    llm_error = Some(err.reason);
                    // Settings gets the plain-words version plus who
                    // failed ("api.groq.com: rate limit / free tier");
                    // the raw provider body stays in the debug log,
                    // where the person reading it asked for detail.
                    // A panel that says "parse error: error decoding
                    // response body" tells the user nothing they can
                    // act on.
                    set_last_llm_error(Some(format!(
                        "{}: {}",
                        entry_host(&text_entries[start]),
                        err.reason
                    )));
                    // The failed attempt still identifies itself in
                    // the transcription log (edited=false).
                    let e = &text_entries[start];
                    llm_host = Some(entry_host(e).to_string());
                    llm_model = Some(e.model.clone());
                    (vocab::apply(&raw_text), false)
                }
            }
        }
    } else {
        (vocab::apply(&raw_text), false)
    };

    debug_log::log(&format!(
        "transcription OK (edited={}, {} chars)",
        edited,
        text.chars().count()
    ));
    Ok(Transcript {
        text,
        raw_text,
        edited,
        llm_attempted,
        stt_secs,
        stt_model,
        llm_secs,
        llm_model,
        llm_host,
        llm_error,
    })
}

/// Read `KEY=value` lines into the process environment — where every provider
/// entry looks its key up (`key_env`). `overwrite` false lets an already-set
/// variable win (development fallback).
pub fn load_env_file(path: &std::path::Path, overwrite: bool) {
    if let Ok(contents) = std::fs::read_to_string(path) {
        for line in contents.lines() {
            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim();
                if !key.is_empty() && !key.starts_with('#') {
                    if overwrite || std::env::var(key).is_err() {
                        unsafe { std::env::set_var(key, value); }
                    }
                }
            }
        }
    }
}
