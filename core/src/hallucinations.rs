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

/// Trailing separators a phantom drags along (ellipsis, dots, whitespace).
const TAIL: &[char] = &['.', '…', ' ', '\t', '\n', '\r'];

/// Remove a trailing phantom phrase, or return "" if the text is nothing but
/// one. Returns the text unchanged when no phantom is present.
pub fn strip(text: &str) -> String {
    let cleaned = text.trim_end_matches(TAIL);
    let probe = cleaned.to_lowercase();
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

#[cfg(test)]
mod tests {
    use super::*;

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
    fn leaves_clean_text_untouched() {
        assert_eq!(strip("Обычный текст без артефакта."), "Обычный текст без артефакта.");
        assert_eq!(strip(""), "");
    }
}
