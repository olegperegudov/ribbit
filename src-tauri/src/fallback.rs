//! Provider-stack + auto-fallback state machine, shared by the STT (audio) and
//! LLM-edit (text) pipelines.
//!
//! Where it sits: each stack is an ordered list of provider entries stored in
//! `config.json` (`audio_providers` / `text_providers`). Entry `[0]` is the
//! primary, the rest are fallbacks tried in order. A request that fails with a
//! *transient* signal — HTTP 429 (rate limit), 5xx (provider down) or a network
//! timeout — counts toward a per-stack consecutive-failure tally; once it
//! reaches the configured threshold the active pointer advances to the next
//! entry and stays there for a cooldown window, after which it snaps back to the
//! primary. A *hard* client error (400/401/403/404 — bad key/url/model) never
//! advances: that is a config bug to surface, not a reason to mask behind a
//! slower backup.
//!
//! The runtime state (active index, fail tally, switch timestamp) is in-memory
//! and per-stack, so an app restart always starts fresh from the primary. The
//! transition logic is pure (`StackState` methods take an explicit `now`) and
//! unit-tested; the public fns just wrap it behind the global mutexes.

use std::sync::Mutex;
use std::time::{Duration, Instant};

/// One provider in a stack. `key_env` names the `.env` variable holding this
/// entry's API key, keeping secrets out of `config.json` and reusing the
/// existing `.env` load path. `url` is the full endpoint (chat-completions for
/// text, audio-transcriptions for audio).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProviderEntry {
    pub id: String,
    #[serde(default)]
    pub label: String,
    pub url: String,
    #[serde(default)]
    pub model: String,
    pub key_env: String,
}

/// Selects which in-memory state slot a call belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stack {
    Audio,
    Text,
}

impl Stack {
    /// Config key holding this stack's ordered entry list.
    pub fn config_key(self) -> &'static str {
        match self {
            Stack::Audio => "audio_providers",
            Stack::Text => "text_providers",
        }
    }
}

/// How the caller should react to a failed request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailKind {
    /// Transient (429 / 5xx / timeout / transport) — counts toward the switch
    /// threshold.
    Switch,
    /// Hard client error (400/401/403/404, or content rejected) — surface it,
    /// never switch.
    Hard,
}

/// Everything the state machine needs to know about a failed request. Built by
/// the request layer (`transcribe` / `postprocess`) so the classification rule
/// lives in exactly one place. A `status` of `Some(200)` means the HTTP call
/// succeeded but the content was rejected (empty / unparseable / runaway) — a
/// hard failure, not a provider-availability problem.
#[derive(Debug, Clone)]
pub struct CallError {
    pub status: Option<u16>,
    pub is_timeout: bool,
    pub message: String,
}

impl CallError {
    pub fn transport(is_timeout: bool, message: String) -> Self {
        Self { status: None, is_timeout, message }
    }
    pub fn http(status: u16, message: String) -> Self {
        Self { status: Some(status), is_timeout: false, message }
    }
    /// HTTP succeeded but the body was unusable — never a switch trigger.
    pub fn rejected(message: String) -> Self {
        Self { status: Some(200), is_timeout: false, message }
    }
}

/// Map a failure to switch-or-not. Only called on failure; success is handled
/// by the caller. A connect/transport error (no status) is treated as transient
/// — the backup is worth a try when the primary can't be reached at all.
pub fn classify(status: Option<u16>, is_timeout: bool) -> FailKind {
    if is_timeout {
        return FailKind::Switch;
    }
    match status {
        Some(429) => FailKind::Switch,
        Some(s) if (500..600).contains(&s) => FailKind::Switch,
        Some(_) => FailKind::Hard, // 400/401/403/404/200-rejected — config/auth/content bug
        None => FailKind::Switch,  // transport error — primary unreachable
    }
}

/// Per-stack runtime state. `active` indexes into the entry list (0 = primary).
struct StackState {
    active: usize,
    consec_fail: u32,
    switched_at: Option<Instant>,
}

impl StackState {
    const fn new() -> Self {
        Self { active: 0, consec_fail: 0, switched_at: None }
    }

    /// Snap back to the primary once the cooldown since the last switch has
    /// elapsed. `cooldown` of zero disables auto-reset.
    fn maybe_reset(&mut self, now: Instant, cooldown: Duration) {
        if self.active != 0 && !cooldown.is_zero() {
            if let Some(at) = self.switched_at {
                if now.duration_since(at) >= cooldown {
                    *self = Self::new();
                }
            }
        }
    }

    /// A request on the active entry succeeded — clear the tally so a later blip
    /// needs its own full `threshold` run to switch.
    fn record_success(&mut self) {
        self.consec_fail = 0;
    }

    /// A transient failure on the active entry. Returns `true` if it advanced to
    /// a new entry (caller logs the switch).
    fn record_switch_fail(&mut self, now: Instant, stack_len: usize, threshold: u32) -> bool {
        self.consec_fail = self.consec_fail.saturating_add(1);
        if self.consec_fail >= threshold && self.active + 1 < stack_len {
            self.active += 1;
            self.consec_fail = 0;
            self.switched_at = Some(now);
            true
        } else {
            false
        }
    }
}

static AUDIO_STATE: Mutex<StackState> = Mutex::new(StackState::new());
static TEXT_STATE: Mutex<StackState> = Mutex::new(StackState::new());

fn state(stack: Stack) -> &'static Mutex<StackState> {
    match stack {
        Stack::Audio => &AUDIO_STATE,
        Stack::Text => &TEXT_STATE,
    }
}

/// Current active entry index for this stack, after applying the cooldown
/// reset. Call once at the start of each request.
pub fn active_index(stack: Stack, cooldown: Duration) -> usize {
    let mut st = state(stack).lock().unwrap();
    st.maybe_reset(Instant::now(), cooldown);
    st.active
}

/// Record a successful request on the active entry.
pub fn on_success(stack: Stack) {
    state(stack).lock().unwrap().record_success();
}

/// Record a transient failure on the active entry. Returns the (possibly new)
/// active index.
pub fn on_switch_fail(stack: Stack, stack_len: usize, threshold: u32) -> usize {
    let mut st = state(stack).lock().unwrap();
    st.record_switch_fail(Instant::now(), stack_len, threshold);
    st.active
}

/// Live snapshot for the Settings status line: active index, how long ago the
/// switch happened. `None` when sitting on the primary (nothing to show).
pub fn snapshot(stack: Stack) -> Option<(usize, Duration)> {
    let st = state(stack).lock().unwrap();
    match (st.active, st.switched_at) {
        (0, _) | (_, None) => None,
        (idx, Some(at)) => Some((idx, at.elapsed())),
    }
}

// --- config readers -------------------------------------------------------

pub fn read_stack(cfg: &serde_json::Value, stack: Stack) -> Vec<ProviderEntry> {
    cfg.get(stack.config_key())
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|e| serde_json::from_value(e.clone()).ok())
                .collect()
        })
        .unwrap_or_default()
}

/// Consecutive transient failures before switching (operator-tunable). Clamped
/// so a stray config value can't disable or runaway the feature.
pub fn threshold(cfg: &serde_json::Value) -> u32 {
    cfg["fallback_threshold"].as_u64().unwrap_or(2).clamp(1, 100) as u32
}

/// How long to stay on a fallback before snapping back to the primary.
pub fn cooldown(cfg: &serde_json::Value) -> Duration {
    let mins = cfg["fallback_cooldown_mins"].as_u64().unwrap_or(60).clamp(1, 1440);
    Duration::from_secs(mins * 60)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_transient_switches() {
        assert_eq!(classify(Some(429), false), FailKind::Switch);
        assert_eq!(classify(Some(500), false), FailKind::Switch);
        assert_eq!(classify(Some(503), false), FailKind::Switch);
        assert_eq!(classify(None, true), FailKind::Switch); // timeout
        assert_eq!(classify(None, false), FailKind::Switch); // transport/connect
    }

    #[test]
    fn classify_hard_does_not_switch() {
        assert_eq!(classify(Some(400), false), FailKind::Hard);
        assert_eq!(classify(Some(401), false), FailKind::Hard);
        assert_eq!(classify(Some(403), false), FailKind::Hard);
        assert_eq!(classify(Some(404), false), FailKind::Hard);
        assert_eq!(classify(Some(200), false), FailKind::Hard); // content rejected
    }

    #[test]
    fn advances_only_after_threshold_consecutive() {
        let mut st = StackState::new();
        let now = Instant::now();
        // threshold 2, stack of 3
        assert!(!st.record_switch_fail(now, 3, 2)); // 1st fail — no switch
        assert_eq!(st.active, 0);
        assert!(st.record_switch_fail(now, 3, 2)); // 2nd consecutive — switch
        assert_eq!(st.active, 1);
        assert_eq!(st.consec_fail, 0); // tally reset after switch
    }

    #[test]
    fn success_resets_tally() {
        let mut st = StackState::new();
        let now = Instant::now();
        st.record_switch_fail(now, 3, 2); // 1 fail
        st.record_success(); // recovered
        assert!(!st.record_switch_fail(now, 3, 2)); // counts from scratch — no switch
        assert_eq!(st.active, 0);
    }

    #[test]
    fn chains_through_entries() {
        let mut st = StackState::new();
        let now = Instant::now();
        st.record_switch_fail(now, 3, 1); // threshold 1 → switch to 1
        assert_eq!(st.active, 1);
        st.record_switch_fail(now, 3, 1); // switch to 2
        assert_eq!(st.active, 2);
        assert!(!st.record_switch_fail(now, 3, 1)); // last entry — cannot advance
        assert_eq!(st.active, 2);
    }

    #[test]
    fn cooldown_resets_to_primary() {
        let mut st = StackState::new();
        let now = Instant::now();
        st.record_switch_fail(now, 2, 1); // on fallback
        assert_eq!(st.active, 1);
        // not yet elapsed
        st.maybe_reset(now + Duration::from_secs(30), Duration::from_secs(60));
        assert_eq!(st.active, 1);
        // elapsed → back to primary
        st.maybe_reset(now + Duration::from_secs(61), Duration::from_secs(60));
        assert_eq!(st.active, 0);
        assert_eq!(st.consec_fail, 0);
        assert!(st.switched_at.is_none());
    }

    #[test]
    fn primary_never_cooldown_resets() {
        let mut st = StackState::new();
        let now = Instant::now();
        // sitting on primary, no switch — maybe_reset is a no-op regardless
        st.maybe_reset(now + Duration::from_secs(9999), Duration::from_secs(60));
        assert_eq!(st.active, 0);
    }

    #[test]
    fn read_stack_parses_entries() {
        let cfg = serde_json::json!({
            "audio_providers": [
                {"id": "a1", "label": "groq", "url": "https://x/v1/audio", "model": "whisper", "key_env": "GROQ_API_KEY"},
                {"id": "a2", "url": "https://y/v1/audio", "key_env": "OPENAI_API_KEY"}
            ]
        });
        let s = read_stack(&cfg, Stack::Audio);
        assert_eq!(s.len(), 2);
        assert_eq!(s[0].id, "a1");
        assert_eq!(s[1].model, ""); // defaulted
        assert!(read_stack(&cfg, Stack::Text).is_empty());
    }

    #[test]
    fn threshold_and_cooldown_clamped() {
        assert_eq!(threshold(&serde_json::json!({})), 2);
        assert_eq!(threshold(&serde_json::json!({"fallback_threshold": 0})), 1);
        assert_eq!(cooldown(&serde_json::json!({})), Duration::from_secs(3600));
        assert_eq!(cooldown(&serde_json::json!({"fallback_cooldown_mins": 5})), Duration::from_secs(300));
    }
}
