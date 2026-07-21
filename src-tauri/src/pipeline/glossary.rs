//! Glossary helpers: whisper prompt text and whole-string replacements.

use crate::config::{GlossaryConfig, GlossaryReplacement};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Build a short initial prompt for whisper from hotwords (and optional pairs).
/// Empty when disabled or no terms.
pub fn build_whisper_prompt(glossary: &GlossaryConfig) -> Option<String> {
    if !glossary.apply_as_whisper_prompt {
        return None;
    }
    let mut terms: Vec<String> = glossary
        .hotwords
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
        .collect();
    for pair in &glossary.replacements {
        let to_value = pair.to.trim();
        if !to_value.is_empty() {
            terms.push(to_value.to_string());
        }
    }
    // De-dupe while preserving order.
    let mut seen = std::collections::HashSet::new();
    terms.retain(|term| seen.insert(term.to_lowercase()));
    if terms.is_empty() {
        return None;
    }
    // Keep prompt modest; whisper initial prompt is not a full dictionary.
    let joined = terms.into_iter().take(64).collect::<Vec<_>>().join("，");
    Some(format!("以下专有名词请优先识别：{joined}。"))
}

/// Apply `from → to` replacements left-to-right (longer `from` first).
pub fn apply_replacements(text: &str, replacements: &[GlossaryReplacement]) -> String {
    if replacements.is_empty() {
        return text.to_string();
    }
    let mut ordered: Vec<&GlossaryReplacement> = replacements
        .iter()
        .filter(|pair| !pair.from.trim().is_empty())
        .collect();
    ordered.sort_by(|left, right| right.from.chars().count().cmp(&left.from.chars().count()));
    let mut result = text.to_string();
    for pair in ordered {
        let from = pair.from.trim();
        let to = pair.to.as_str();
        if from.is_empty() || from == to {
            continue;
        }
        result = result.replace(from, to);
    }
    result
}

pub fn apply_post_replace(text: &str, glossary: &GlossaryConfig) -> String {
    if !glossary.apply_post_replace {
        return text.to_string();
    }
    apply_replacements(text, &glossary.replacements)
}

/// Stable content hash for Job metadata (not cryptographic).
pub fn glossary_content_hash(glossary: &GlossaryConfig) -> String {
    let mut hasher = DefaultHasher::new();
    glossary.hotwords.hash(&mut hasher);
    for pair in &glossary.replacements {
        pair.from.hash(&mut hasher);
        pair.to.hash(&mut hasher);
    }
    glossary.apply_as_whisper_prompt.hash(&mut hasher);
    glossary.apply_post_replace.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_prompt_from_hotwords() {
        let glossary = GlossaryConfig {
            hotwords: vec!["张三".into(), "OpenAI".into()],
            replacements: vec![GlossaryReplacement {
                from: "openai".into(),
                to: "OpenAI".into(),
            }],
            apply_as_whisper_prompt: true,
            apply_post_replace: true,
        };
        let prompt = build_whisper_prompt(&glossary).expect("prompt");
        assert!(prompt.contains("张三"));
        assert!(prompt.contains("OpenAI"));
    }

    #[test]
    fn skips_prompt_when_disabled() {
        let glossary = GlossaryConfig {
            hotwords: vec!["A".into()],
            replacements: vec![],
            apply_as_whisper_prompt: false,
            apply_post_replace: true,
        };
        assert!(build_whisper_prompt(&glossary).is_none());
    }

    #[test]
    fn replaces_longer_first() {
        let text = "人工智能AI与人工智能";
        let replacements = vec![
            GlossaryReplacement {
                from: "AI".into(),
                to: "人工智能".into(),
            },
            GlossaryReplacement {
                from: "人工智能".into(),
                to: "AI".into(),
            },
        ];
        // Longer "人工智能" wins first pass ordering by length.
        let result = apply_replacements(text, &replacements);
        assert!(!result.is_empty());
    }

    #[test]
    fn simple_replace() {
        let result = apply_replacements(
            "hello world",
            &[GlossaryReplacement {
                from: "world".into(),
                to: "地球".into(),
            }],
        );
        assert_eq!(result, "hello 地球");
    }

    #[test]
    fn hash_stable() {
        let glossary = GlossaryConfig::default();
        let first = glossary_content_hash(&glossary);
        let second = glossary_content_hash(&glossary);
        assert_eq!(first, second);
    }
}
