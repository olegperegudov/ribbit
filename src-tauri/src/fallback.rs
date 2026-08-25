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
//!
//! `run_with_failover` additionally walks the stack *within one request*: a
//! transient failure tries the next entry immediately, so the current
//! dictation is rescued by a backup instead of only the next one.

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

    /// Short name for log lines.
    pub fn name(self) -> &'static str {
        match self {
            Stack::Audio => "audio",
            Stack::Text => "text",
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
    /// `Retry-After` as the provider sent it: how long this entry will keep
    /// refusing. Groq answers an exhausted daily token pool with the seconds
    /// left until midnight UTC (34421 — nine and a half hours), which is worth
    /// far more than guessing: without it the stack snaps back to the dead
    /// primary every cooldown window, pays a 429 per dictation and drops to the
    /// slow backup again, all day.
    pub retry_after: Option<Duration>,
    /// The same failure in the user's words, for the history row next to the
    /// yellow dot ("timed out", "rate limit / free tier").
    /// Written where the error is born, so nothing downstream has to re-read
    /// `message` — which is a provider's raw body and changes shape per vendor.
    pub reason: &'static str,
    /// Switch-or-not for this failure, decided where the failure is born. Most
    /// errors get it from `classify`; the ones that can't be read off status
    /// alone (a 200 whose body carried no usable answer) set it themselves.
    pub kind: FailKind,
}

impl CallError {
    pub fn transport(is_timeout: bool, message: String) -> Self {
        let reason = if is_timeout { "timed out" } else { "provider unreachable" };
        Self { status: None, is_timeout, message, reason, retry_after: None, kind: classify(None, is_timeout) }
    }
    pub fn http(status: u16, message: String, retry_after: Option<Duration>) -> Self {
        let kind = classify(Some(status), false);
        Self { status: Some(status), is_timeout: false, message, reason: http_reason(status), retry_after, kind }
    }
    /// HTTP succeeded and the answer itself is the problem — the guards refused
    /// it (a runaway rewrite, a model arguing with the prompt). Another provider
    /// would be asked the same thing about the same text, so this one is final.
    pub fn rejected(message: String) -> Self {
        Self { status: Some(200), is_timeout: false, message, reason: "reply unusable", retry_after: None, kind: FailKind::Hard }
    }
    /// HTTP succeeded but there was no answer in it to judge: the completion hit
    /// the token cap (a reasoning model can spend the whole budget thinking), the
    /// content came back empty, or the body wasn't the shape the API promises.
    /// That is this provider failing, not the text being bad — the next rung
    /// answers the same prompt fine, so it gets its turn.
    pub fn no_answer(message: String) -> Self {
        Self { status: Some(200), is_timeout: false, message, reason: "reply incomplete", retry_after: None, kind: FailKind::Switch }
    }
    /// Nothing in the stack had a key, so no request was ever made.
    pub fn no_key(message: String) -> Self {
        Self { status: Some(200), is_timeout: false, message, reason: "no key set", retry_after: None, kind: FailKind::Hard }
    }
}

/// `Retry-After` in seconds, the form every provider here sends. The RFC also
/// allows an HTTP date; nobody in this stack uses it, and a misread date that
/// parks the primary for a day is worse than ignoring the header, so an
/// unparseable value simply falls back to the configured cooldown. Capped at a
/// day — the state is in memory anyway, so a restart clears it.
pub fn retry_after_secs(headers: &reqwest::header::HeaderMap) -> Option<Duration> {
    let secs: u64 = headers.get(reqwest::header::RETRY_AFTER)?.to_str().ok()?.trim().parse().ok()?;
    Some(Duration::from_secs(secs.min(24 * 3600)))
}

/// The user-facing half of `classify`: what an HTTP status means for the person
/// looking at the log. 429 gets the free-tier wording because that is what it
/// almost always is here — the alternative reading (too many dictations per
/// minute) leads to the same place, waiting or another provider.
fn http_reason(status: u16) -> &'static str {
    match status {
        429 => "rate limit / free tier",
        s if (500..600).contains(&s) => "provider is down",
        401 | 403 => "key rejected",
        404 => "model or url not found",
        400 => "request rejected",
        _ => "call failed",
    }
}

/// Read a 2xx response body as JSON, keeping the two ways that can fail apart.
///
/// Reading the body and parsing it look like one step (`response.json()`) but
/// mean opposite things. A read that dies mid-body — the request timeout firing
/// while the model is still generating, a dropped connection — is the provider
/// failing to deliver, and the next entry in the stack is exactly the cure. A
/// body that arrives whole and isn't JSON is *this* provider answering wrong,
/// which no backup fixes. Collapsed into one verdict, the first case inherits
/// the second's "don't switch" and the stack stops rescuing slow providers:
/// that is how a Groq daily-limit day left every over-5s RouterAI edit falling
/// straight through to raw text (2026-08-13).
pub fn read_json(response: reqwest::blocking::Response) -> Result<serde_json::Value, CallError> {
    let body = response
        .text()
        .map_err(|e| CallError::transport(e.is_timeout(), format!("body read failed: {}", e)))?;
    parse_json_body(&body)
}

/// Body → JSON, with a slice of the offending text in the error so the debug
/// log says *what* came back instead of just "invalid syntax".
fn parse_json_body(body: &str) -> Result<serde_json::Value, CallError> {
    serde_json::from_str(body).map_err(|e| {
        CallError::rejected(format!(
            "parse error: {} (body: {})",
            e,
            body.chars().take(200).collect::<String>()
        ))
    })
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
    /// The provider's own `Retry-After` for the entry we switched away from,
    /// when it sent one. Overrides the configured cooldown while it is longer:
    /// the config says how long to wait on a *guess*, this says how long the
    /// provider knows it will keep saying no.
    hold: Option<Duration>,
}

impl StackState {
    const fn new() -> Self {
        Self { active: 0, consec_fail: 0, switched_at: None, hold: None }
    }

    /// Snap back to the primary once the wait since the last switch has elapsed.
    /// `cooldown` of zero disables auto-reset.
    fn maybe_reset(&mut self, now: Instant, cooldown: Duration) {
        if self.active != 0 && !cooldown.is_zero() {
            let wait = self.hold.unwrap_or_default().max(cooldown);
            if let Some(at) = self.switched_at {
                if now.duration_since(at) >= wait {
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
    fn record_switch_fail(
        &mut self,
        now: Instant,
        stack_len: usize,
        threshold: u32,
        retry_after: Option<Duration>,
    ) -> bool {
        self.consec_fail = self.consec_fail.saturating_add(1);
        if self.consec_fail >= threshold && self.active + 1 < stack_len {
            self.active += 1;
            self.consec_fail = 0;
            self.switched_at = Some(now);
            self.hold = retry_after;
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

/// Live snapshot for the Settings status line: active index, how long ago the
/// switch happened. `None` when sitting on the primary (nothing to show).
pub fn snapshot(stack: Stack) -> Option<(usize, Duration)> {
    let st = state(stack).lock().unwrap();
    match (st.active, st.switched_at) {
        (0, _) | (_, None) => None,
        (idx, Some(at)) => Some((idx, at.elapsed())),
    }
}

/// Walk the stack from `start`, trying each entry that has a key. A transient
/// failure (per `classify`) moves on to the next entry, so the *current*
/// request is rescued by a backup instead of only the next one. A hard error
/// (bad key/url/model) surfaces immediately — that's a config bug the user
/// must see, not mask behind a slower backup. Entries whose `key_env` is
/// empty are skipped without counting as failures.
///
/// Sticky-state interaction: only the entry the walk *started* at feeds the
/// consecutive-failure tally (deeper rungs are walk-local), and only a success
/// on that same entry clears it. Otherwise a healthy backup would reset the
/// tally every dictation and the sticky switch would never trip — every
/// request would keep paying the dead primary's timeout first.
///
/// `budget` caps the whole walk for stacks where the wait is worse than the
/// loss: when a flaky network makes every entry time out, marching through the
/// full stack multiplies one provider's timeout by the number of rungs (a real
/// 26s edit on a 5s timeout). Once the budget is spent no further entry is
/// tried and the caller's own fallback takes over. `None` = walk to the end,
/// for the audio stack — there a dropped dictation is unrecoverable, so waiting
/// out the whole stack beats giving up on the user's speech.
///
/// Returns the call's value plus the index of the entry that produced it.
pub fn run_with_failover<T>(
    stack: Stack,
    entries: &[ProviderEntry],
    start: usize,
    threshold: u32,
    budget: Option<Duration>,
    call: impl Fn(&ProviderEntry, &str) -> Result<T, CallError>,
) -> Result<(T, usize), CallError> {
    let t0 = Instant::now();
    run_with_failover_on(state(stack), stack, entries, start, threshold, budget, move || t0.elapsed(), call)
}

/// Core of `run_with_failover` with the state slot and the clock injected — unit
/// tests pass their own `Mutex<StackState>` so they don't race on the process
/// globals, and their own `elapsed` so budget behaviour is asserted on exact
/// values instead of on how fast the machine happened to run (the same reason
/// `StackState` takes an explicit `now`).
fn run_with_failover_on<T>(
    st: &Mutex<StackState>,
    stack: Stack,
    entries: &[ProviderEntry],
    start: usize,
    threshold: u32,
    budget: Option<Duration>,
    elapsed: impl Fn() -> Duration,
    call: impl Fn(&ProviderEntry, &str) -> Result<T, CallError>,
) -> Result<(T, usize), CallError> {
    let mut last_err: Option<CallError> = None;
    for (i, entry) in entries.iter().enumerate().skip(start) {
        let name = if entry.label.is_empty() { entry.url.as_str() } else { entry.label.as_str() };
        // Checked before the call, not after: the budget bounds how long the
        // *user* waits, so a rung is only worth starting while there's budget
        // left. The starting entry always gets its try — a zero-attempt walk
        // would fail the request without ever touching a provider.
        if i > start && budget.is_some_and(|b| elapsed() >= b) {
            crate::debug_log::log(&format!(
                "{} stack: budget spent after {:.1}s, not trying '{}'",
                stack.name(), elapsed().as_secs_f32(), name
            ));
            break;
        }
        let key = std::env::var(&entry.key_env).unwrap_or_default();
        if key.is_empty() {
            crate::debug_log::log(&format!("{} stack: '{}' has no key, skipping", stack.name(), name));
            continue;
        }
        match call(entry, &key) {
            Ok(v) => {
                if i == start {
                    st.lock().unwrap().record_success();
                }
                return Ok((v, i));
            }
            Err(e) => match e.kind {
                FailKind::Switch => {
                    if i == start {
                        st.lock().unwrap().record_switch_fail(
                            Instant::now(),
                            entries.len(),
                            threshold,
                            e.retry_after,
                        );
                    }
                    crate::debug_log::log(&format!(
                        "{} stack: transient fail on '{}' ({}); trying next entry",
                        stack.name(), name, e.message
                    ));
                    last_err = Some(e);
                }
                FailKind::Hard => {
                    crate::debug_log::log(&format!(
                        "{} stack: hard fail on '{}': {} (surfacing, no failover)",
                        stack.name(), name, e.message
                    ));
                    return Err(e);
                }
            },
        }
    }
    Err(last_err.unwrap_or_else(
        || CallError::no_key("no API key set for any provider — add one in Settings".to_string()),
    ))
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
    fn unparseable_body_is_the_providers_fault_not_the_networks() {
        // Whole body, wrong content: this provider answered badly, and walking
        // to the next one would just re-ask a working endpoint.
        let e = parse_json_body("<html>gateway</html>").unwrap_err();
        assert_eq!(classify(e.status, e.is_timeout), FailKind::Hard);
        assert!(e.message.contains("<html>gateway</html>"), "body must be quoted back: {}", e.message);
    }

    /// A provider that promises a body and dies halfway through it — what a
    /// firing timeout or a dropped connection looks like from `read_json`. The
    /// same reqwest call as an invalid-JSON body, the opposite verdict, so it
    /// gets a real socket: asserting on a hand-built `CallError` would keep
    /// passing after a regression put both back on one branch.
    #[test]
    fn a_severed_body_reads_as_transport_not_content() {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        std::thread::spawn(move || {
            let (mut sock, _) = listener.accept().unwrap();
            let _ = sock.read(&mut [0u8; 1024]);
            let _ = sock.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\n{\"choices\":");
        });

        let resp = reqwest::blocking::Client::new()
            .get(format!("http://{}", addr))
            .send()
            .unwrap();
        let e = read_json(resp).unwrap_err();
        assert_eq!(classify(e.status, e.is_timeout), FailKind::Switch, "{}", e.message);
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
        assert!(!st.record_switch_fail(now, 3, 2, None)); // 1st fail — no switch
        assert_eq!(st.active, 0);
        assert!(st.record_switch_fail(now, 3, 2, None)); // 2nd consecutive — switch
        assert_eq!(st.active, 1);
        assert_eq!(st.consec_fail, 0); // tally reset after switch
    }

    #[test]
    fn success_resets_tally() {
        let mut st = StackState::new();
        let now = Instant::now();
        st.record_switch_fail(now, 3, 2, None); // 1 fail
        st.record_success(); // recovered
        assert!(!st.record_switch_fail(now, 3, 2, None)); // counts from scratch — no switch
        assert_eq!(st.active, 0);
    }

    #[test]
    fn chains_through_entries() {
        let mut st = StackState::new();
        let now = Instant::now();
        st.record_switch_fail(now, 3, 1, None); // threshold 1 → switch to 1
        assert_eq!(st.active, 1);
        st.record_switch_fail(now, 3, 1, None); // switch to 2
        assert_eq!(st.active, 2);
        assert!(!st.record_switch_fail(now, 3, 1, None)); // last entry — cannot advance
        assert_eq!(st.active, 2);
    }

    #[test]
    fn cooldown_resets_to_primary() {
        let mut st = StackState::new();
        let now = Instant::now();
        st.record_switch_fail(now, 2, 1, None); // on fallback
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
    fn a_providers_own_retry_after_outranks_the_cooldown() {
        let mut st = StackState::new();
        let now = Instant::now();
        // Groq's exhausted daily pool: nine and a half hours, against a 30-min
        // configured cooldown. Snapping back on the cooldown would spend the
        // rest of the day paying a 429 before every dictation.
        let day_left = Duration::from_secs(34421);
        st.record_switch_fail(now, 2, 1, Some(day_left));
        st.maybe_reset(now + Duration::from_secs(1800), Duration::from_secs(1800));
        assert_eq!(st.active, 1, "cooldown must not override the provider's own wait");
        st.maybe_reset(now + day_left, Duration::from_secs(1800));
        assert_eq!(st.active, 0, "back to primary once the provider's wait is over");
    }

    #[test]
    fn a_short_retry_after_never_shortens_the_cooldown() {
        // The other direction: a provider asking for 5s must not turn the
        // cooldown into 5s, or a flapping primary gets retried every dictation.
        let mut st = StackState::new();
        let now = Instant::now();
        st.record_switch_fail(now, 2, 1, Some(Duration::from_secs(5)));
        st.maybe_reset(now + Duration::from_secs(10), Duration::from_secs(1800));
        assert_eq!(st.active, 1);
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

    // --- run_with_failover ---------------------------------------------

    /// Entry whose key lives in `env` — set it (or not) per test with a name
    /// unique to that test, since process env is shared across test threads.
    fn entry(id: &str, env: &str) -> ProviderEntry {
        ProviderEntry {
            id: id.into(),
            label: id.into(),
            url: format!("https://example.test/{}", id),
            model: "m".into(),
            key_env: env.into(),
        }
    }

    fn set_key(env: &str) {
        unsafe { std::env::set_var(env, "k") };
    }

    /// Clock for the walks that run without a budget — never consulted.
    fn no_time() -> Duration {
        Duration::ZERO
    }

    /// Clock that advances by `step` on every call, so "how much of the budget
    /// is left" is exact instead of depending on how fast the machine ran.
    fn ticking(step: Duration) -> impl Fn() -> Duration {
        let elapsed = std::cell::Cell::new(Duration::ZERO);
        move || {
            elapsed.set(elapsed.get() + step);
            elapsed.get()
        }
    }

    #[test]
    fn failover_success_on_start_entry() {
        set_key("FO_T1_A");
        let st = Mutex::new(StackState::new());
        let entries = [entry("a", "FO_T1_A"), entry("b", "FO_T1_A")];
        let out = run_with_failover_on(&st, Stack::Audio, &entries, 0, 2, None, no_time, |e, _| {
            Ok::<_, CallError>(e.id.clone())
        })
        .unwrap();
        assert_eq!(out, ("a".to_string(), 0));
        assert_eq!(st.lock().unwrap().consec_fail, 0);
    }

    #[test]
    fn failover_transient_rescued_by_next_entry_same_request() {
        set_key("FO_T2_A");
        let st = Mutex::new(StackState::new());
        let entries = [entry("a", "FO_T2_A"), entry("b", "FO_T2_A")];
        let run = |st: &Mutex<StackState>| {
            // Take `start` in its own statement — the guard must drop before
            // run_with_failover_on locks the same mutex.
            let start = st.lock().unwrap().active;
            run_with_failover_on(st, Stack::Audio, &entries, start, 2, None, no_time, |e, _| {
                if e.id == "a" {
                    Err(CallError::http(429, "rate limited".into(), None))
                } else {
                    Ok(e.id.clone())
                }
            })
        };
        // The dictation itself survives via entry b...
        assert_eq!(run(&st).unwrap(), ("b".to_string(), 1));
        // ...while the starting entry's failure still counts toward the sticky
        // switch: threshold 2 trips on the second request.
        assert_eq!(st.lock().unwrap().active, 0);
        assert_eq!(run(&st).unwrap(), ("b".to_string(), 1));
        assert_eq!(st.lock().unwrap().active, 1);
        // Third request starts at b directly; success there resets its tally.
        assert_eq!(run(&st).unwrap(), ("b".to_string(), 1));
        assert_eq!(st.lock().unwrap().consec_fail, 0);
    }

    #[test]
    fn failover_hard_error_stops_walk() {
        set_key("FO_T3_A");
        let st = Mutex::new(StackState::new());
        let entries = [entry("a", "FO_T3_A"), entry("b", "FO_T3_A")];
        let calls = std::cell::Cell::new(0);
        let out = run_with_failover_on(&st, Stack::Text, &entries, 0, 2, None, no_time, |_, _| {
            calls.set(calls.get() + 1);
            Err::<String, _>(CallError::http(401, "bad key".into(), None))
        });
        assert_eq!(out.as_ref().unwrap_err().message, "bad key");
        assert_eq!(out.unwrap_err().reason, "key rejected");
        assert_eq!(calls.get(), 1, "hard error must not try further entries");
        assert_eq!(st.lock().unwrap().consec_fail, 0, "hard errors never feed the switch tally");
    }

    #[test]
    fn failover_skips_entries_without_key() {
        set_key("FO_T4_B");
        let st = Mutex::new(StackState::new());
        let entries = [entry("a", "FO_T4_MISSING"), entry("b", "FO_T4_B")];
        let out = run_with_failover_on(&st, Stack::Audio, &entries, 0, 2, None, no_time, |e, _| {
            Ok::<_, CallError>(e.id.clone())
        })
        .unwrap();
        assert_eq!(out, ("b".to_string(), 1));
        assert_eq!(st.lock().unwrap().consec_fail, 0, "a key-less skip is not a failure");
    }

    #[test]
    fn failover_no_keys_at_all_is_actionable_error() {
        let st = Mutex::new(StackState::new());
        let entries = [entry("a", "FO_T5_MISSING")];
        let out = run_with_failover_on(&st, Stack::Audio, &entries, 0, 2, None, no_time, |_, _| {
            Ok::<_, CallError>(String::new())
        });
        let err = out.unwrap_err();
        assert!(err.message.contains("no API key"));
        assert_eq!(err.reason, "no key set");
    }

    #[test]
    fn failover_budget_stops_the_walk() {
        // The flaky-network case: every entry times out, and marching through
        // the whole stack multiplies one provider's timeout by the rung count.
        // A budget of ~2 timeouts must cut the walk short instead.
        set_key("FO_T7_A");
        let st = Mutex::new(StackState::new());
        let entries = [entry("a", "FO_T7_A"), entry("b", "FO_T7_A"), entry("c", "FO_T7_A")];
        let calls = std::cell::Cell::new(0);
        // Each rung burns a 5s timeout against an 8s budget: the second still
        // starts (5 < 8), the third does not (10 ≥ 8).
        let out = run_with_failover_on(
            &st, Stack::Text, &entries, 0, 5, Some(Duration::from_secs(8)),
            ticking(Duration::from_secs(5)),
            |_, _| {
                calls.set(calls.get() + 1);
                Err::<String, _>(CallError::transport(true, "timeout".into()))
            },
        );
        let err = out.unwrap_err();
        assert_eq!(err.message, "timeout");
        assert_eq!(err.reason, "timed out");
        assert_eq!(calls.get(), 2, "third entry starts past the budget and must be skipped");
    }

    #[test]
    fn shipped_budget_lets_an_unreachable_stack_reach_its_last_rung() {
        // The 2026-08-25 outage: the two foreign providers wouldn't connect at
        // all while the domestic one answered fine. Connect failures cost the
        // 2s handshake cap, not the 5s answer cap, so the shipped budget has to
        // leave room for the third rung — that one is the endpoint that works.
        set_key("FO_T9_A");
        let st = Mutex::new(StackState::new());
        let entries = [entry("a", "FO_T9_A"), entry("b", "FO_T9_A"), entry("c", "FO_T9_A")];
        let calls = std::cell::Cell::new(0);
        let out = run_with_failover_on(
            &st, Stack::Text, &entries, 0, 5,
            Some(Duration::from_secs(crate::postprocess::STACK_BUDGET_SECS)),
            ticking(Duration::from_secs(crate::postprocess::CONNECT_TIMEOUT_SECS)),
            |e, _| {
                calls.set(calls.get() + 1);
                if e.id == "c" { Ok(e.id.clone()) } else { Err(CallError::transport(false, "connect".into())) }
            },
        );
        assert_eq!(out.unwrap(), ("c".to_string(), 2));
        assert_eq!(calls.get(), 3, "the last rung must still get its turn on a stack that fails to connect");
    }

    #[test]
    fn an_answerless_reply_hands_the_turn_to_the_next_provider() {
        // A 200 with nothing usable in it (completion truncated by the token
        // cap, empty content, body the wrong shape) is this provider failing,
        // not the text being bad — the walk must go on. Regression: groq's
        // reasoning model returned a truncated edit, that surfaced as a hard
        // fail, and the router sitting one rung below was never asked.
        set_key("FO_T10_A");
        let st = Mutex::new(StackState::new());
        let entries = [entry("a", "FO_T10_A"), entry("b", "FO_T10_A")];
        let out = run_with_failover_on(&st, Stack::Text, &entries, 0, 2, None, no_time, |e, _| {
            if e.id == "a" {
                Err(CallError::no_answer("output truncated (finish_reason=length)".into()))
            } else {
                Ok(e.id.clone())
            }
        });
        assert_eq!(out.unwrap(), ("b".to_string(), 1));
    }

    #[test]
    fn a_refused_answer_is_final_and_does_not_walk_on() {
        // The other half of the pair: the guards refused the answer itself (a
        // runaway rewrite). Every provider gets asked the same thing about the
        // same text, so re-asking is just another wait.
        set_key("FO_T11_A");
        let st = Mutex::new(StackState::new());
        let entries = [entry("a", "FO_T11_A"), entry("b", "FO_T11_A")];
        let calls = std::cell::Cell::new(0);
        let out = run_with_failover_on(&st, Stack::Text, &entries, 0, 2, None, no_time, |_, _| {
            calls.set(calls.get() + 1);
            Err::<String, _>(CallError::rejected("answer 4x the input".into()))
        });
        assert_eq!(out.unwrap_err().reason, "reply unusable");
        assert_eq!(calls.get(), 1, "a refused answer must not cost the user a second provider");
    }

    #[test]
    fn failover_budget_never_skips_the_starting_entry() {
        // A budget already spent (zero) must still let the walk try its first
        // rung — otherwise a request fails without any provider being called.
        set_key("FO_T8_A");
        let st = Mutex::new(StackState::new());
        let entries = [entry("a", "FO_T8_A"), entry("b", "FO_T8_A")];
        let calls = std::cell::Cell::new(0);
        let out = run_with_failover_on(
            &st, Stack::Text, &entries, 0, 5, Some(Duration::ZERO),
            ticking(Duration::from_secs(5)),
            |e, _| {
                calls.set(calls.get() + 1);
                Ok::<_, CallError>(e.id.clone())
            },
        );
        assert_eq!(out.unwrap(), ("a".to_string(), 0));
        assert_eq!(calls.get(), 1);
    }

    #[test]
    fn failover_deep_failure_does_not_feed_sticky_tally() {
        set_key("FO_T6_A");
        let st = Mutex::new(StackState::new());
        let entries = [entry("a", "FO_T6_A"), entry("b", "FO_T6_A")];
        let out = run_with_failover_on(&st, Stack::Audio, &entries, 0, 5, None, no_time, |_, _| {
            Err::<String, _>(CallError::transport(true, "timeout".into()))
        });
        assert_eq!(out.unwrap_err().message, "timeout");
        // Both entries failed, but only the starting one counts.
        assert_eq!(st.lock().unwrap().consec_fail, 1);
    }
}
