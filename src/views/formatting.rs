use std::collections::{HashMap, HashSet};

use crate::views::bugs::status_label;
use crate::views::html_escape;

/// Information about a referenced issue used when rendering `#123` links.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IssueRefInfo {
    pub id: i64,
    pub status: i32,
    pub summary: String,
}

/// Rendering context for MantisCoreFormatting passed from controllers to views.
#[derive(Debug, Clone, Default)]
pub struct FormatContext {
    pub enabled: bool,
    pub issue_refs: HashMap<i64, IssueRefInfo>,
    pub note_refs: HashMap<i64, i64>,
    pub valid_usernames: HashSet<String>,
}

/// Helper that strips fenced code blocks and inline code from text,
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
                    // Valid closed inline code span; replace content with spaces
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

/// Scans raw text for referenced issue IDs (`#123`), ignoring code blocks and inline code.
pub fn scan_issue_refs(raw: &str) -> Vec<i64> {
    let text = extract_non_code_text(raw);
    let mut ids = Vec::new();
    let mut seen = HashSet::new();

    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if chars[i] == '#' {
            let prev_ok = if i == 0 {
                true
            } else {
                let prev = chars[i - 1];
                !prev.is_alphanumeric() && prev != '&'
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

/// Scans raw text for referenced bugnote IDs (`~123`), ignoring code blocks and inline code.
pub fn scan_note_refs(raw: &str) -> Vec<i64> {
    let text = extract_non_code_text(raw);
    let mut ids = Vec::new();
    let mut seen = HashSet::new();

    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if chars[i] == '~' {
            let prev_ok = if i == 0 {
                true
            } else {
                let prev = chars[i - 1];
                !prev.is_alphanumeric() && prev != '~'
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
        if chars[i] == '@' {
            let prev_ok = if i == 0 {
                true
            } else {
                let prev = chars[i - 1];
                !prev.is_alphanumeric()
                    && prev != '_'
                    && prev != '.'
                    && prev != '-'
                    && prev != '+'
                    && prev != '/'
                    && prev != ':'
                    && prev != '='
                    && prev != '?'
                    && prev != '&'
            };

            if prev_ok {
                let start = i + 1;
                let mut end = start;
                while end < len {
                    let c = chars[end];
                    if c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.' {
                        end += 1;
                    } else {
                        break;
                    }
                }
                if end > start {
                    let raw_uname: String = chars[start..end].iter().collect();
                    let trimmed = raw_uname.trim_end_matches(['.', ',', '!', '?', ':', ';']);
                    if !trimmed.is_empty() {
                        let lower = trimmed.to_lowercase();
                        if seen.insert(lower) {
                            mentions.push(trimmed.to_string());
                        }
                    }
                }
            }
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

/// Formats text according to MantisCoreFormatting rules with XSS protection.
///
/// When disabled, returns plain HTML-escaped text.
/// When enabled, renders safe Markdown, issue/bugnote links, auto-links, and mentions.
pub fn format_text(raw: &str, ctx: &FormatContext) -> String {
    if !ctx.enabled {
        return html_escape(raw);
    }

    // Strip internal sentinel delimiters \x01 and \x02 to prevent any placeholder injection
    let sanitized_raw: String = raw
        .chars()
        .filter(|&c| c != '\x01' && c != '\x02')
        .collect();
    let raw = &sanitized_raw;

    if raw.trim().is_empty() {
        return String::new();
    }

    let mut placeholders: Vec<String> = Vec::new();

    // 1. Separate fenced code blocks from regular Markdown lines
    enum LineItem {
        Normal(String),
        FencePlaceholder(usize),
    }

    let mut line_items: Vec<LineItem> = Vec::new();
    let mut in_fence = false;
    let mut fence_lang = String::new();
    let mut fence_code = String::new();

    for line in raw.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            if in_fence {
                // Closing fence
                let escaped_code = html_escape(&fence_code);
                let html = if fence_lang.is_empty() {
                    format!(
                        r#"<pre class="code-block"><code>{}</code></pre>"#,
                        escaped_code
                    )
                } else {
                    format!(
                        r#"<pre class="code-block"><code class="language-{}">{}</code></pre>"#,
                        html_escape(&fence_lang),
                        escaped_code
                    )
                };
                let idx = placeholders.len();
                placeholders.push(html);
                line_items.push(LineItem::FencePlaceholder(idx));
                in_fence = false;
                fence_lang.clear();
                fence_code.clear();
            } else {
                // Opening fence
                in_fence = true;
                let lang = trimmed.trim_start_matches('`').trim();
                fence_lang = lang.to_string();
                fence_code.clear();
            }
            continue;
        }

        if in_fence {
            if !fence_code.is_empty() {
                fence_code.push('\n');
            }
            fence_code.push_str(line);
        } else {
            line_items.push(LineItem::Normal(line.to_string()));
        }
    }

    // Unclosed fence fallback
    if in_fence {
        let escaped_code = html_escape(&fence_code);
        let html = if fence_lang.is_empty() {
            format!(
                r#"<pre class="code-block"><code>{}</code></pre>"#,
                escaped_code
            )
        } else {
            format!(
                r#"<pre class="code-block"><code class="language-{}">{}</code></pre>"#,
                html_escape(&fence_lang),
                escaped_code
            )
        };
        let idx = placeholders.len();
        placeholders.push(html);
        line_items.push(LineItem::FencePlaceholder(idx));
    }

    // 2. Parse Markdown block structures outside of fenced blocks
    enum Block {
        Fence(usize),
        Heading { level: usize, text: String },
        Hr,
        Blockquote(Vec<String>),
        Ul(Vec<String>),
        Ol { start: u64, items: Vec<String> },
        Paragraph(Vec<String>),
    }

    let mut blocks: Vec<Block> = Vec::new();
    let mut current_para: Vec<String> = Vec::new();
    let mut current_quote: Vec<String> = Vec::new();
    let mut current_ul: Vec<String> = Vec::new();
    let mut current_ol: Vec<String> = Vec::new();
    let mut current_ol_start: u64 = 1;

    macro_rules! flush_para {
        () => {
            if !current_para.is_empty() {
                blocks.push(Block::Paragraph(std::mem::take(&mut current_para)));
            }
        };
    }
    macro_rules! flush_quote {
        () => {
            if !current_quote.is_empty() {
                blocks.push(Block::Blockquote(std::mem::take(&mut current_quote)));
            }
        };
    }
    macro_rules! flush_ul {
        () => {
            if !current_ul.is_empty() {
                blocks.push(Block::Ul(std::mem::take(&mut current_ul)));
            }
        };
    }
    macro_rules! flush_ol {
        () => {
            if !current_ol.is_empty() {
                blocks.push(Block::Ol {
                    start: std::mem::replace(&mut current_ol_start, 1),
                    items: std::mem::take(&mut current_ol),
                });
            }
        };
    }
    macro_rules! flush_all {
        () => {
            flush_para!();
            flush_quote!();
            flush_ul!();
            flush_ol!();
        };
    }

    for item in line_items {
        match item {
            LineItem::FencePlaceholder(idx) => {
                flush_all!();
                blocks.push(Block::Fence(idx));
            }
            LineItem::Normal(line) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    flush_all!();
                    continue;
                }

                // Check Horizontal Rule: `---`, `***`, `___`
                if trimmed == "---" || trimmed == "***" || trimmed == "___" {
                    flush_all!();
                    blocks.push(Block::Hr);
                    continue;
                }

                // Check Headings: `## ` through `###### ` (strictly NOT turning `#123` into an `<h1>`)
                if let Some((level, text)) = parse_heading(&line) {
                    flush_all!();
                    blocks.push(Block::Heading { level, text });
                    continue;
                }

                // Check Blockquote: lines starting with `>`
                if let Some(rest) = parse_blockquote(&line) {
                    flush_para!();
                    flush_ul!();
                    flush_ol!();
                    current_quote.push(rest);
                    continue;
                } else {
                    flush_quote!();
                }

                // Check Unordered List: `- `, `* `, `+ `
                if let Some(rest) = parse_ul_item(&line) {
                    flush_para!();
                    flush_ol!();
                    current_ul.push(rest);
                    continue;
                } else {
                    flush_ul!();
                }

                // Check Ordered List: `1. `, `2. `, etc.
                if let Some((num, rest)) = parse_ol_item(&line) {
                    if !current_para.is_empty() && current_ol.is_empty() && num != 1 {
                        // CommonMark: A new ordered list can only interrupt a paragraph if the start number is 1
                        current_para.push(line);
                        continue;
                    }
                    if current_ol.is_empty() && num > 999 {
                        // Prevent accidental years/dates (e.g. "2024. was a great year") from starting a list
                        current_para.push(line);
                        continue;
                    }
                    if current_ol.is_empty() {
                        current_ol_start = num;
                    }
                    flush_para!();
                    flush_ul!();
                    current_ol.push(rest);
                    continue;
                } else {
                    flush_ol!();
                }

                // Default: regular paragraph line
                current_para.push(line);
            }
        }
    }
    flush_all!();

    // 3. Render blocks and format inline elements
    let mut out = String::from(r#"<div class="formatted-text">"#);

    for block in blocks {
        match block {
            Block::Fence(idx) => {
                if idx < placeholders.len() {
                    out.push_str(&placeholders[idx]);
                }
            }
            Block::Heading { level, text } => {
                let formatted = format_inline(&text, ctx, &mut placeholders);
                out.push_str(&format!("<h{}>{}</h{}>", level, formatted, level));
            }
            Block::Hr => {
                out.push_str("<hr>");
            }
            Block::Blockquote(lines) => {
                let mut inner = String::new();
                for (i, l) in lines.iter().enumerate() {
                    if i > 0 {
                        inner.push_str("<br>");
                    }
                    inner.push_str(&format_inline(l, ctx, &mut placeholders));
                }
                out.push_str(&format!("<blockquote>{}</blockquote>", inner));
            }
            Block::Ul(items) => {
                out.push_str("<ul>");
                for item in items {
                    let formatted = format_inline(&item, ctx, &mut placeholders);
                    out.push_str(&format!("<li>{}</li>", formatted));
                }
                out.push_str("</ul>");
            }
            Block::Ol { start, items } => {
                if start != 1 {
                    out.push_str(&format!(r#"<ol start="{}">"#, start));
                } else {
                    out.push_str("<ol>");
                }
                for item in items {
                    let formatted = format_inline(&item, ctx, &mut placeholders);
                    out.push_str(&format!("<li>{}</li>", formatted));
                }
                out.push_str("</ol>");
            }
            Block::Paragraph(lines) => {
                let mut inner = String::new();
                for (i, l) in lines.iter().enumerate() {
                    if i > 0 {
                        inner.push_str("<br>");
                    }
                    inner.push_str(&format_inline(l, ctx, &mut placeholders));
                }
                out.push_str(&format!("<p>{}</p>", inner));
            }
        }
    }

    out.push_str("</div>");

    // Expand all remaining placeholders
    expand_placeholders(&out, &placeholders)
}

fn parse_heading(line: &str) -> Option<(usize, String)> {
    let trimmed = line.trim_start();
    // Support headings `# ` through `###### ` (requires space after hashes, so `#123` is never an h1)
    let hashes = trimmed.chars().take_while(|&c| c == '#').count();
    if (1..=6).contains(&hashes) {
        let rest = &trimmed[hashes..];
        if rest.starts_with(' ') {
            let heading_text = rest.trim();
            if !heading_text.is_empty() {
                return Some((hashes, heading_text.to_string()));
            }
        }
    }
    None
}

fn parse_blockquote(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    trimmed
        .strip_prefix('>')
        .map(|stripped| stripped.strip_prefix(' ').unwrap_or(stripped).to_string())
}

fn parse_ul_item(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    for prefix in &["- ", "* ", "+ "] {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            return Some(rest.to_string());
        }
    }
    None
}

fn parse_ol_item(line: &str) -> Option<(u64, String)> {
    let trimmed = line.trim_start();
    let num_digits = trimmed.chars().take_while(|c| c.is_ascii_digit()).count();
    if (1..=9).contains(&num_digits) {
        let digits_str = &trimmed[..num_digits];
        if let Ok(num) = digits_str.parse::<u64>() {
            let after_digits = &trimmed[num_digits..];
            if let Some(rest) = after_digits.strip_prefix(". ") {
                return Some((num, rest.to_string()));
            }
        }
    }
    None
}

/// Formats inline elements: inline code, markdown links, bare URLs, issue refs, note refs,
/// mentions, bold, italic, and strikethrough.
fn format_inline(raw_line: &str, ctx: &FormatContext, placeholders: &mut Vec<String>) -> String {
    // Step 1: Protect inline code `...`
    let mut protected_line = String::with_capacity(raw_line.len());
    let chars: Vec<char> = raw_line.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if chars[i] == '`' {
            // Find closing backtick
            let start = i + 1;
            let mut close = start;
            while close < len && chars[close] != '`' {
                close += 1;
            }
            if close < len {
                let code_content: String = chars[start..close].iter().collect();
                let html = format!(
                    r#"<code class="inline-code">{}</code>"#,
                    html_escape(&code_content)
                );
                let idx = placeholders.len();
                placeholders.push(html);
                protected_line.push_str(&format!("\x01P{}X\x02", idx));
                i = close + 1;
                continue;
            }
        }
        protected_line.push(chars[i]);
        i += 1;
    }

    // Step 2: HTML-escape the line so raw user HTML (<script>, <img>, etc.) can never execute
    let escaped_line = html_escape(&protected_line);

    // Step 3: Markdown links [label](http...) - strictly only http:// and https://
    let mut with_md_links = String::with_capacity(escaped_line.len());
    let mut cursor = 0;
    let mut last = 0;
    let b = escaped_line.as_bytes();
    let blen = b.len();

    // Pre-index all `](` locations in a single O(N) pass to guarantee strictly linear O(N) bracket scanning
    let mid_indices: Vec<usize> = escaped_line
        .match_indices("](")
        .map(|(idx, _)| idx)
        .collect();
    let mut mid_ptr = 0;

    // Pre-index all `)` locations in a single O(N) pass to guarantee strictly linear O(N) paren scanning
    let close_paren_indices: Vec<usize> = escaped_line
        .match_indices(')')
        .map(|(idx, _)| idx)
        .collect();
    let mut paren_ptr = 0;

    while cursor < blen {
        if b[cursor] == b'[' {
            // Advance pointer to the next `](` that appears strictly after cursor
            while mid_ptr < mid_indices.len() && mid_indices[mid_ptr] <= cursor {
                mid_ptr += 1;
            }

            // If a `](` exists within 1024 bytes of `[`, inspect it
            if mid_ptr < mid_indices.len() && mid_indices[mid_ptr] - cursor <= 1024 {
                let close_bracket = mid_indices[mid_ptr];
                let url_start = close_bracket + 2;

                while paren_ptr < close_paren_indices.len()
                    && close_paren_indices[paren_ptr] < url_start
                {
                    paren_ptr += 1;
                }

                if paren_ptr < close_paren_indices.len()
                    && close_paren_indices[paren_ptr] - url_start <= 2048
                {
                    let close_paren = close_paren_indices[paren_ptr];
                    let label = &escaped_line[cursor + 1..close_bracket];
                    let url = escaped_line[url_start..close_paren].trim();
                    let has_scheme = url
                        .get(..7)
                        .is_some_and(|p| p.eq_ignore_ascii_case("http://"))
                        || url
                            .get(..8)
                            .is_some_and(|p| p.eq_ignore_ascii_case("https://"));

                    if has_scheme
                        && !url.contains(char::is_whitespace)
                        && !url.contains('\x01')
                        && !url.contains('\x02')
                    {
                        // Expand any inline code placeholders inside the link label immediately
                        let expanded_label = expand_placeholders(label, placeholders);
                        let safe_url = url.replace('"', "&quot;").replace('\'', "&#x27;");
                        let link_html = format!(
                            r#"<a href="{}" target="_blank" rel="noopener noreferrer nofollow">{}</a>"#,
                            safe_url, expanded_label
                        );
                        let idx = placeholders.len();
                        placeholders.push(link_html);
                        with_md_links.push_str(&escaped_line[last..cursor]);
                        with_md_links.push_str(&format!("\x01P{}X\x02", idx));
                        cursor = close_paren + 1;
                        last = cursor;
                        continue;
                    } else {
                        // The URL target for this `](` is invalid; advance `mid_ptr` so preceding `[` do not re-inspect it
                        mid_ptr += 1;
                    }
                } else {
                    // No valid closing paren within 2048 bytes; advance `mid_ptr`
                    mid_ptr += 1;
                }
            }
        }
        cursor += 1;
    }
    with_md_links.push_str(&escaped_line[last..]);

    // Step 4: Auto-link bare URLs (http:// and https://)
    let with_bare_urls = autolink_bare_urls(&with_md_links, placeholders);

    // Step 5: Auto-link issue references `#123`
    let with_issue_refs = link_issue_refs(&with_bare_urls, ctx, placeholders);

    // Step 6: Auto-link bugnote references `~123`
    let with_note_refs = link_note_refs(&with_issue_refs, ctx, placeholders);

    // Step 7: Highlight `@username` mentions
    let with_mentions = highlight_mentions(&with_note_refs, ctx, placeholders);

    // Step 8: Inline formatting: **bold**, ~~strikethrough~~, *italic*
    format_text_decorations(&with_mentions)
}

fn autolink_bare_urls(text: &str, placeholders: &mut Vec<String>) -> String {
    let mut out = String::with_capacity(text.len());
    let mut i = 0;
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();

    while i < len {
        // Skip placeholders \x01...\x02
        if chars[i] == '\x01' {
            let start = i;
            while i < len && chars[i] != '\x02' {
                i += 1;
            }
            if i < len {
                i += 1;
            }
            let ph: String = chars[start..i].iter().collect();
            out.push_str(&ph);
            continue;
        }

        // Check if starts with http:// or https://
        let rem: String = chars[i..chars.len().min(i + 8)].iter().collect();
        let rem_lower = rem.to_lowercase();
        let is_http = rem_lower.starts_with("http://");
        let is_https = rem_lower.starts_with("https://");

        if is_http || is_https {
            // Preceding character must not be alphanumeric or @
            let prev_ok = if i == 0 {
                true
            } else {
                let prev = chars[i - 1];
                !prev.is_alphanumeric() && prev != '@'
            };

            if prev_ok {
                let prefix_len = if is_http { 7 } else { 8 };
                // Fast-check host start: character immediately after "://" must exist and be alphanumeric
                let has_valid_host = chars
                    .get(i + prefix_len)
                    .map(|c| c.is_ascii_alphanumeric())
                    .unwrap_or(false);
                if !has_valid_host {
                    // Not a valid URL start (e.g. "http://.", "http://#", bare "http://").
                    // Jump past the scheme so we never re-scan or trigger quadratic scanning
                    for k in 0..prefix_len {
                        if i + k < len {
                            out.push(chars[i + k]);
                        }
                    }
                    i += prefix_len;
                    continue;
                }

                let url_start = i;
                let mut url_end = url_start;
                while url_end < len {
                    let c = chars[url_end];
                    if c.is_whitespace()
                        || c == '<'
                        || c == '>'
                        || c == '"'
                        || c == '\''
                        || c == '\x01'
                    {
                        break;
                    }
                    // Stop before escaped HTML entity delimiters: &quot;, &gt;, &lt;, &#x27;
                    let remaining_chars = &chars[url_end..];
                    if remaining_chars.starts_with(&['&', 'q', 'u', 'o', 't', ';'])
                        || remaining_chars.starts_with(&['&', 'g', 't', ';'])
                        || remaining_chars.starts_with(&['&', 'l', 't', ';'])
                        || remaining_chars.starts_with(&['&', '#', 'x', '2', '7', ';'])
                    {
                        break;
                    }
                    url_end += 1;
                }

                // Balance parentheses and trailing punctuation outside the URL in a strictly linear O(N) scan
                let mut candidate_url: String = chars[url_start..url_end].iter().collect();
                let open_p = candidate_url.chars().filter(|&c| c == '(').count();
                let mut close_p = candidate_url.chars().filter(|&c| c == ')').count();

                let mut cut = candidate_url.len();
                loop {
                    let s = &candidate_url[..cut];
                    if s.ends_with(|c| ".,!?:;]".contains(c)) && !s.ends_with("&amp;") {
                        cut -= 1;
                        continue;
                    }
                    if s.ends_with(')') && close_p > open_p {
                        cut -= 1;
                        close_p -= 1;
                        continue;
                    }
                    break;
                }
                let trailing = candidate_url.split_off(cut);

                if !candidate_url.is_empty() {
                    let safe_url = candidate_url.replace('"', "&quot;").replace('\'', "&#x27;");
                    let link_html = format!(
                        r#"<a href="{}" target="_blank" rel="noopener noreferrer nofollow">{}</a>"#,
                        safe_url, safe_url
                    );
                    let idx = placeholders.len();
                    placeholders.push(link_html);
                    out.push_str(&format!("\x01P{}X\x02", idx));
                    out.push_str(&trailing);
                    i = url_end;
                    continue;
                }
            }
        }

        out.push(chars[i]);
        i += 1;
    }

    out
}

fn link_issue_refs(text: &str, ctx: &FormatContext, placeholders: &mut Vec<String>) -> String {
    let mut out = String::with_capacity(text.len());
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if chars[i] == '\x01' {
            let start = i;
            while i < len && chars[i] != '\x02' {
                i += 1;
            }
            if i < len {
                i += 1;
            }
            let ph: String = chars[start..i].iter().collect();
            out.push_str(&ph);
            continue;
        }

        if chars[i] == '#' {
            let prev_ok = if i == 0 {
                true
            } else {
                let prev = chars[i - 1];
                !prev.is_alphanumeric() && prev != '&'
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
                            if let Some(info) = ctx.issue_refs.get(&id) {
                                let st = status_label(info.status);
                                let clean_summary: String = info
                                    .summary
                                    .chars()
                                    .filter(|&c| c != '\x01' && c != '\x02')
                                    .collect();
                                let title = format!("#{:07}: {} [{}]", info.id, clean_summary, st);
                                let escaped_title = html_escape(&title);

                                let is_resolved = info.status >= crate::models::STATUS_RESOLVED;
                                let link_html = if is_resolved {
                                    format!(
                                        r#"<a href="/view_bug_page/{}" class="issue-link-resolved" title="{}"><del>#{:07}</del></a>"#,
                                        id, escaped_title, id
                                    )
                                } else {
                                    format!(
                                        r#"<a href="/view_bug_page/{}" title="{}">#{:07}</a>"#,
                                        id, escaped_title, id
                                    )
                                };

                                let idx = placeholders.len();
                                placeholders.push(link_html);
                                out.push_str(&format!("\x01P{}X\x02", idx));
                                i = end;
                                continue;
                            }
                        }
                    }
                }
            }
        }

        out.push(chars[i]);
        i += 1;
    }

    out
}

fn link_note_refs(text: &str, ctx: &FormatContext, placeholders: &mut Vec<String>) -> String {
    let mut out = String::with_capacity(text.len());
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if chars[i] == '\x01' {
            let start = i;
            while i < len && chars[i] != '\x02' {
                i += 1;
            }
            if i < len {
                i += 1;
            }
            let ph: String = chars[start..i].iter().collect();
            out.push_str(&ph);
            continue;
        }

        if chars[i] == '~' {
            let prev_ok = if i == 0 {
                true
            } else {
                let prev = chars[i - 1];
                !prev.is_alphanumeric() && prev != '~'
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
                            if let Some(&bug_id) = ctx.note_refs.get(&id) {
                                let link_html = format!(
                                    r#"<a href="/view_bug_page/{}#c{}">~{:07}</a>"#,
                                    bug_id, id, id
                                );
                                let idx = placeholders.len();
                                placeholders.push(link_html);
                                out.push_str(&format!("\x01P{}X\x02", idx));
                                i = end;
                                continue;
                            }
                        }
                    }
                }
            }
        }

        out.push(chars[i]);
        i += 1;
    }

    out
}

fn highlight_mentions(text: &str, ctx: &FormatContext, placeholders: &mut Vec<String>) -> String {
    let mut out = String::with_capacity(text.len());
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if chars[i] == '\x01' {
            let start = i;
            while i < len && chars[i] != '\x02' {
                i += 1;
            }
            if i < len {
                i += 1;
            }
            let ph: String = chars[start..i].iter().collect();
            out.push_str(&ph);
            continue;
        }

        if chars[i] == '@' {
            let prev_ok = if i == 0 {
                true
            } else {
                let prev = chars[i - 1];
                !prev.is_alphanumeric()
                    && prev != '_'
                    && prev != '.'
                    && prev != '-'
                    && prev != '+'
                    && prev != '/'
                    && prev != ':'
                    && prev != '='
                    && prev != '?'
                    && prev != '&'
            };

            if prev_ok {
                let start = i + 1;
                let mut end = start;
                while end < len {
                    let c = chars[end];
                    if c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.' {
                        end += 1;
                    } else {
                        break;
                    }
                }
                if end > start {
                    let raw_uname: String = chars[start..end].iter().collect();
                    // First check exact candidate against valid_usernames (supports usernames ending in '_')
                    let matched_user = if ctx.valid_usernames.contains(&raw_uname)
                        || ctx
                            .valid_usernames
                            .contains(&raw_uname.to_ascii_lowercase())
                    {
                        Some((raw_uname.clone(), end))
                    } else {
                        // Otherwise try trimming trailing punctuation (e.g. '@alice.' at end of sentence)
                        let trimmed = raw_uname.trim_end_matches(['.', ',', '!', '?', ':', ';']);
                        let trailing_len = raw_uname.len() - trimmed.len();
                        let actual_end = end - trailing_len;
                        if !trimmed.is_empty()
                            && (ctx.valid_usernames.contains(trimmed)
                                || ctx.valid_usernames.contains(&trimmed.to_ascii_lowercase()))
                        {
                            Some((trimmed.to_string(), actual_end))
                        } else {
                            None
                        }
                    };

                    if let Some((uname, actual_end)) = matched_user {
                        let span_html =
                            format!(r#"<span class="mention">@{}</span>"#, html_escape(&uname));
                        let idx = placeholders.len();
                        placeholders.push(span_html);
                        out.push_str(&format!("\x01P{}X\x02", idx));
                        i = actual_end;
                        continue;
                    }
                }
            }
        }

        out.push(chars[i]);
        i += 1;
    }

    out
}

fn format_text_decorations(text: &str) -> String {
    // 1. ***bold italic*** (properly nested)
    let s = replace_delimiter_pair(text, "***", "<strong><em>", "</em></strong>");
    // 2. **bold**
    let s = replace_delimiter_pair(&s, "**", "<strong>", "</strong>");
    // 3. ~~strikethrough~~
    let s = replace_delimiter_pair(&s, "~~", "<del>", "</del>");
    // 4. *italic*
    replace_delimiter_pair(&s, "*", "<em>", "</em>")
}

fn replace_delimiter_pair(text: &str, delim: &str, open_tag: &str, close_tag: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut remaining = text;

    while let Some(open_idx) = remaining.find(delim) {
        // Push text before opening delim
        out.push_str(&remaining[..open_idx]);
        let after_open = &remaining[open_idx + delim.len()..];

        // Flanking rule: opening delimiter must NOT be followed by whitespace
        if after_open.starts_with(|c: char| c.is_whitespace()) {
            out.push_str(delim);
            remaining = after_open;
            continue;
        }

        // Find closing delim (must not be empty and must be before any newline)
        if let Some(close_idx) = after_open.find(delim) {
            let inside = &after_open[..close_idx];
            // Flanking rule: closing delimiter must NOT be preceded by whitespace
            if !inside.is_empty()
                && !inside.contains('\n')
                && !inside.contains(delim)
                && !inside.ends_with(|c: char| c.is_whitespace())
            {
                out.push_str(open_tag);
                out.push_str(inside);
                out.push_str(close_tag);
                remaining = &after_open[close_idx + delim.len()..];
                continue;
            }
        }

        // No valid closing delimiter found, treat as literal
        out.push_str(delim);
        remaining = after_open;
    }

    out.push_str(remaining);
    out
}

fn expand_placeholders(text: &str, placeholders: &[String]) -> String {
    let mut out = String::with_capacity(text.len());
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        if chars[i] == '\x01' {
            let start = i;
            while i < len && chars[i] != '\x02' {
                i += 1;
            }
            if i < len {
                i += 1;
            }
            let ph: String = chars[start..i].iter().collect();
            // Placeholder format is `\x01P{idx}X\x02`
            if ph.starts_with("\x01P") && ph.ends_with("X\x02") {
                let num_str = &ph[2..ph.len() - 2];
                if let Ok(idx) = num_str.parse::<usize>() {
                    if idx < placeholders.len() {
                        out.push_str(&placeholders[idx]);
                        continue;
                    }
                }
            }
            out.push_str(&ph);
            continue;
        }
        out.push(chars[i]);
        i += 1;
    }

    out
}

#[cfg(test)]
#[path = "formatting_test.rs"]
mod tests;
