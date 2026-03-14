use std::collections::HashMap;
use std::fs;

use crate::debug_log;

/// vocab.json structure: {"target_word": ["alias1", "alias2"], ...}
/// At runtime we invert it to: {"alias_lowercase": "target_word", ...}

fn vocab_path() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|d| d.join("ribbit").join("vocab.json"))
}

/// Read the raw vocab map: target → [aliases]
pub fn read_vocab() -> HashMap<String, Vec<String>> {
    vocab_path()
        .and_then(|p| fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Save the vocab map
pub fn save_vocab(vocab: &HashMap<String, Vec<String>>) -> Result<(), String> {
    let path = vocab_path().ok_or("Cannot find config directory")?;
    fs::create_dir_all(path.parent().unwrap()).map_err(|e| e.to_string())?;

    // Sort keys for stable output
    let sorted: std::collections::BTreeMap<_, _> = vocab.iter().collect();
    fs::write(&path, serde_json::to_string_pretty(&sorted).unwrap())
        .map_err(|e| e.to_string())
}

/// Build inverted lookup: alias_lowercase → target_word
fn build_lookup(vocab: &HashMap<String, Vec<String>>) -> HashMap<String, String> {
    let mut lookup = HashMap::new();
    for (target, aliases) in vocab {
        for alias in aliases {
            lookup.insert(alias.to_lowercase(), target.clone());
        }
    }
    lookup
}

/// Apply vocab replacements to text.
/// Multi-word aliases are replaced first (phrase match), then single-word (whole word match).
pub fn apply(text: &str) -> String {
    let vocab = read_vocab();
    if vocab.is_empty() {
        return text.to_string();
    }
    let lookup = build_lookup(&vocab);
    if lookup.is_empty() {
        return text.to_string();
    }

    // Separate multi-word and single-word aliases
    let mut multi: Vec<(&str, &str)> = Vec::new();  // (alias_lower, target)
    let mut single: HashMap<&str, &str> = HashMap::new();
    for (alias, target) in &lookup {
        if alias.contains(' ') {
            multi.push((alias, target));
        } else {
            single.insert(alias, target);
        }
    }

    // Sort multi-word by length descending (longest match first)
    multi.sort_by(|a, b| b.0.len().cmp(&a.0.len()));

    // Phase 1: replace multi-word phrases (case-insensitive)
    let mut result = text.to_string();
    for (alias, target) in &multi {
        let lower = result.to_lowercase();
        let mut new_result = String::new();
        let mut pos = 0;
        while let Some(idx) = lower[pos..].find(*alias) {
            let abs_idx = pos + idx;
            // Check word boundaries
            let before_ok = abs_idx == 0 || !result[..abs_idx].chars().last().map_or(false, |c| c.is_alphanumeric() || c == '_');
            let after_end = abs_idx + alias.len();
            let after_ok = after_end >= result.len() || !result[after_end..].chars().next().map_or(false, |c| c.is_alphanumeric() || c == '_');
            if before_ok && after_ok {
                new_result.push_str(&result[pos..abs_idx]);
                let matched = &result[abs_idx..after_end];
                new_result.push_str(&match_case(matched, target));
                pos = after_end;
            } else {
                new_result.push_str(&result[pos..abs_idx + alias.len()]);
                pos = abs_idx + alias.len();
            }
        }
        new_result.push_str(&result[pos..]);
        result = new_result;
    }

    // Phase 2: replace single words
    if !single.is_empty() {
        let mut word_result = String::with_capacity(result.len());
        let mut chars = result.char_indices().peekable();

        while let Some((i, c)) = chars.next() {
            if c.is_alphanumeric() || c == '_' {
                let start = i;
                let mut end = i + c.len_utf8();
                while let Some(&(j, nc)) = chars.peek() {
                    if nc.is_alphanumeric() || nc == '_' {
                        end = j + nc.len_utf8();
                        chars.next();
                    } else {
                        break;
                    }
                }
                let word = &result[start..end];
                let lower = word.to_lowercase();

                if let Some(target) = single.get(lower.as_str()) {
                    word_result.push_str(&match_case(word, target));
                } else {
                    word_result.push_str(word);
                }
            } else {
                word_result.push(c);
            }
        }
        result = word_result;
    }

    if result != text {
        debug_log::log(&format!("vocab: {:?} → {:?}", text, result));
    }
    result
}

/// Transfer case pattern from source word to target word.
/// "Деф" → "Дев", "ДЕФ" → "ДЕВ", "деф" → "дев"
fn match_case(source: &str, target: &str) -> String {
    let src_chars: Vec<char> = source.chars().collect();

    if src_chars.iter().all(|c| c.is_uppercase() || !c.is_alphabetic()) {
        // ALL CAPS
        target.to_uppercase()
    } else if src_chars.first().map_or(false, |c| c.is_uppercase()) {
        // Title Case
        let mut chars = target.chars();
        match chars.next() {
            Some(first) => {
                let mut s: String = first.to_uppercase().collect();
                s.extend(chars);
                s
            }
            None => String::new(),
        }
    } else {
        // lowercase
        target.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_match_case() {
        assert_eq!(match_case("деф", "dev"), "dev");
        assert_eq!(match_case("Деф", "dev"), "Dev");
        assert_eq!(match_case("ДЕФ", "dev"), "DEV");
        assert_eq!(match_case("hello", "world"), "world");
        assert_eq!(match_case("Hello", "world"), "World");
        assert_eq!(match_case("HELLO", "world"), "WORLD");
    }

    #[test]
    fn test_apply_with_empty_vocab() {
        // With empty vocab, text should pass through unchanged
        let text = "hello world";
        assert_eq!(apply(text), text);
    }
}
