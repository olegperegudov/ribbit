//! Strips known Whisper silence-hallucinations from raw STT output.
//!
//! Whisper was trained on a huge pile of Russian subtitle boilerplate, so on
//! silence or near-silence it emits phantom captions — overwhelmingly
//! "Продолжение следует..." — either as the entire "transcript" (mic opened,
//! nothing said) or tacked onto the end of a real dictation as its own
//! sentence. Neither the LLM edit (told to keep every word verbatim) nor
//! `vocab::apply` removes them, so we cut them here, on the raw text, before
//! either pass runs.

/// Phrases Whisper invents on silence. Matched case-insensitively with any
/// trailing ellipsis / period / whitespace ignored. Keep entries lowercase and
/// without trailing dots.
const PHANTOMS: &[&str] = &[
    "продолжение следует",
];

/// Phantoms only when they are the entire transcript: said at the end of a
/// real sentence they are ordinary speech. Whisper's English silence captions,
/// and the language hint it echoes back on silence (`transcribe` sends
/// "Dictation in Russian and English." as the prompt). Seen on the Steam Deck,
/// whose mic lets more near-silence through than the Mac's.
const WHOLE_ONLY: &[&str] = &[
    "thank you",
    "thanks for watching",
    "thank you for watching",
    "you",
    "dictation in russian and english",
    "dictation in english and russian",
];

/// Openings of the subtitle credits Whisper recites on silence, whatever name
/// follows — "Субтитры создавал DimaTorzok", and with a glossary in the prompt
/// "Субтитры создавал DPS". Only a transcript that starts with one is dropped.
const CREDIT_OPENINGS: &[&str] = &[
    "субтитры создавал",
    "субтитры сделал",
    "субтитры подготовил",
    "редактор субтитров",
];

/// Trailing separators a phantom drags along (ellipsis, dots, whitespace).
const TAIL: &[char] = &['.', '…', '!', ' ', '\t', '\n', '\r'];

/// Remove a trailing phantom phrase, or return "" if the text is nothing but
/// one. Returns the text unchanged when no phantom is present.
pub fn strip(text: &str) -> String {
    let cleaned = text.trim_end_matches(TAIL);
    let probe = cleaned.trim_start().to_lowercase();
    // A lone "." or "…" is what's left of silence once there is no phrase to
    // invent: nothing was said, and a dot is not worth pasting.
    let no_words = !probe.chars().any(char::is_alphanumeric);
    if no_words || WHOLE_ONLY.contains(&probe.as_str()) || CREDIT_OPENINGS.iter().any(|c| probe.starts_with(c)) {
        return String::new();
    }
    for p in PHANTOMS {
        if probe == *p {
            return String::new();
        }
        if probe.ends_with(p) {
            // Cyrillic upper/lower are 1:1 per char, so the phantom occupies the
            // same char count in the original-case `cleaned` — drop exactly that
            // many chars. Trim only whitespace afterwards: the real sentence's
            // own period (". Продолжение...") must survive.
            let keep = cleaned.chars().count() - p.chars().count();
            let head: String = cleaned.chars().take(keep).collect();
            return head.trim_end().to_string();
        }
    }
    text.to_string()
}

/// On silence Whisper may hand back a stretch of its own prompt as the
/// transcript — with a game glossary in the prompt that is "rogue, mage,
/// warlock". Three words or more, so a one-word call like "tank", which also
/// sits in the glossary, still goes through.
pub fn echoes_prompt(text: &str, prompt: &str) -> bool {
    let words = |s: &str| -> Vec<String> {
        s.split(|c: char| !c.is_alphanumeric())
            .filter(|w| !w.is_empty())
            .map(str::to_lowercase)
            .collect()
    };
    let said = words(text);
    said.len() >= 3 && words(prompt).windows(said.len()).any(|w| w == said.as_slice())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_echo_is_caught_but_short_calls_pass() {
        let p = "Dictation in Russian and English. WoW group chat: rogue, mage, warlock, priest.";
        assert!(echoes_prompt("Rogue, mage, warlock.", p));
        assert!(echoes_prompt("WoW group chat", p));
        assert!(!echoes_prompt("Tank!", p));
        assert!(!echoes_prompt("rogue mage", p));
        assert!(!echoes_prompt("rogue, come help me", p));
    }

    #[test]
    fn whole_text_is_phantom() {
        assert_eq!(strip("Продолжение следует..."), "");
        assert_eq!(strip("  продолжение следует  "), "");
        assert_eq!(strip("Продолжение следует…"), "");
    }

    #[test]
    fn trailing_phantom_after_real_text() {
        assert_eq!(
            strip("Давай начнём с аудита. Продолжение следует..."),
            "Давай начнём с аудита."
        );
        assert_eq!(
            strip("глянь что там с ribbit Продолжение следует..."),
            "глянь что там с ribbit"
        );
    }

    #[test]
    fn whole_only_phantoms() {
        assert_eq!(strip("Thank you."), "");
        assert_eq!(strip(" Thanks for watching!"), "");
        assert_eq!(strip("you"), "");
        assert_eq!(strip("Dictation in Russian and English."), "");
        // as part of a real message they stay
        assert_eq!(strip("Great run, thank you."), "Great run, thank you.");
        assert_eq!(strip("Thank you for the carry"), "Thank you for the carry");
    }

    #[test]
    fn punctuation_alone_is_nothing() {
        assert_eq!(strip("."), "");
        assert_eq!(strip(" … "), "");
        assert_eq!(strip("?!"), "");
        assert_eq!(strip("ok."), "ok.");
    }

    #[test]
    fn subtitle_credits_are_phantoms() {
        assert_eq!(strip("Субтитры создавал DPS."), "");
        assert_eq!(strip("Субтитры создавал DimaTorzok"), "");
        assert_eq!(strip("Редактор субтитров А.Синецкая Корректор А.Егорова"), "");
        // Talking about subtitles is still speech.
        assert_eq!(strip("Включи субтитры, создавал же"), "Включи субтитры, создавал же");
    }

    #[test]
    fn leaves_clean_text_untouched() {
        assert_eq!(strip("Обычный текст без артефакта."), "Обычный текст без артефакта.");
        assert_eq!(strip(""), "");
    }
}
