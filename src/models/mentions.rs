use std::collections::HashSet;

/// Strips fenced code blocks and inline code spans from text,
/// returning only candidate text for reference and mention scanning.
fn extract_non_code_text(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut in_fence = false;

    for line in raw.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            in_fence = !in_fence;
            out.push('\n');
            continue;
        }
        if in_fence {
            out.push('\n');
            continue;
        }

        // Inside non-fenced line, strip inline code spans (`...`) matching pairs
        let line_chars: Vec<char> = line.chars().collect();
        let llen = line_chars.len();
        let mut li = 0;
        while li < llen {
            if line_chars[li] == '`' {
                let start = li + 1;
                let mut close = start;
                while close < llen && line_chars[close] != '`' {
                    close += 1;
                }
                if close < llen {
                    for _ in li..=close {
                        out.push(' ');
                    }
                    li = close + 1;
                    continue;
                }
            }
            out.push(line_chars[li]);
            li += 1;
        }
        out.push('\n');
    }

    out
}

/// Scans `#123` or `~123` numeric references in raw text, ignoring code blocks and inline code.
fn scan_numeric_refs(raw: &str, sigil: char, disallowed_prev: char) -> Vec<i64> {
    let text = extract_non_code_text(raw);
    let mut ids = Vec::new();
    let mut seen = HashSet::new();

    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if chars[i] == sigil {
            let prev_ok = if i == 0 {
                true
            } else {
                let prev = chars[i - 1];
                !prev.is_alphanumeric() && prev != disallowed_prev
            };

            if prev_ok {
                let start = i + 1;
                let mut end = start;
                while end < len && chars[end].is_ascii_digit() && end - start < 10 {
                    end += 1;
                }
                if end > start {
                    let next_ok = if end == len {
                        true
                    } else {
                        !chars[end].is_alphanumeric()
                    };

                    if next_ok {
                        let digits: String = chars[start..end].iter().collect();
                        if let Ok(id) = digits.parse::<i64>() {
                            if id > 0 && seen.insert(id) {
                                ids.push(id);
                            }
                        }
                    }
                }
            }
        }
        i += 1;
    }

    ids
}

/// Scans raw text for referenced issue IDs (`#123`), ignoring code blocks and inline code.
pub fn scan_issue_refs(raw: &str) -> Vec<i64> {
    scan_numeric_refs(raw, '#', '&')
}

/// Scans raw text for referenced bugnote IDs (`~123`), ignoring code blocks and inline code.
pub fn scan_note_refs(raw: &str) -> Vec<i64> {
    scan_numeric_refs(raw, '~', '~')
}

/// Parses a single `@username` mention candidate at `chars[at_idx]`, returning
/// `Some((username, end_char_index))` when valid.
///
/// Shared by both `extract_mentions` and view highlighting so trailing punctuation
/// is handled identically across notifications and HTML rendering.
pub fn parse_mention_at(chars: &[char], at_idx: usize) -> Option<(String, usize)> {
    let len = chars.len();
    if at_idx >= len || chars[at_idx] != '@' {
        return None;
    }

    let prev_ok = if at_idx == 0 {
        true
    } else {
        let prev = chars[at_idx - 1];
        !prev.is_alphanumeric()
            && !matches!(prev, '_' | '.' | '-' | '+' | '/' | ':' | '=' | '?' | '&')
    };
    if !prev_ok {
        return None;
    }

    let start = at_idx + 1;
    let mut end = start;
    while end < len {
        let c = chars[end];
        if c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.' {
            end += 1;
        } else {
            break;
        }
    }
    if end == start {
        return None;
    }

    let raw_uname: String = chars[start..end].iter().collect();
    let trimmed = raw_uname.trim_end_matches(['.', ',', '!', '?', ':', ';']);
    if trimmed.is_empty() {
        return None;
    }
    let trailing_chars = raw_uname.chars().count() - trimmed.chars().count();
    let actual_end = end - trailing_chars;
    Some((trimmed.to_string(), actual_end))
}

/// Scans raw text for mentioned usernames (`@username`), ignoring code blocks, inline code,
/// and email addresses like `foo@bar.com`.
pub fn extract_mentions(raw: &str) -> Vec<String> {
    let text = extract_non_code_text(raw);
    let mut mentions = Vec::new();
    let mut seen = HashSet::new();

    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if let Some((uname, next_idx)) = parse_mention_at(&chars, i) {
            let lower = uname.to_ascii_lowercase();
            if seen.insert(lower) {
                mentions.push(uname);
            }
            i = next_idx;
            continue;
        }
        i += 1;
    }

    mentions
}

/// Collects and deduplicates user mentions from multiple text slices case-insensitively,
/// capped at 30 unique mentions.
pub fn collect_mentions(texts: &[&str]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut deduped = Vec::new();
    for &t in texts {
        for m in extract_mentions(t) {
            if seen.insert(m.to_ascii_lowercase()) {
                deduped.push(m);
                if deduped.len() >= 30 {
                    return deduped;
                }
            }
        }
    }
    deduped
}

/// Combines non-empty text parts into a unified context string for notifications,
/// falling back to the summary if all parts are empty.
pub fn combine_context(summary: &str, parts: &[&str]) -> String {
    let non_empty: Vec<&str> = parts
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    if non_empty.is_empty() {
        summary.trim().to_string()
    } else {
        non_empty.join("\n\n")
    }
}

/// Returns the newly added mentions in `new_mentions` that were not in `old_mentions`,
/// deduplicated case-insensitively and capped at 30 entries.
pub fn diff_new_mentions(old_mentions: &HashSet<String>, new_mentions: &[String]) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut fresh = Vec::new();
    for m in new_mentions {
        let lower = m.to_ascii_lowercase();
        if !old_mentions.contains(&lower) && seen.insert(lower) {
            fresh.push(m.clone());
            if fresh.len() >= 30 {
                break;
            }
        }
    }
    fresh
}
