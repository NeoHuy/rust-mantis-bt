use std::collections::{HashMap, HashSet};

use super::{
    collect_mentions, combine_context, diff_new_mentions, extract_mentions, format_text,
    scan_issue_refs, scan_note_refs, FormatContext, IssueRefInfo,
};

fn sample_context(enabled: bool) -> FormatContext {
    let mut issue_refs = HashMap::new();
    issue_refs.insert(
        101,
        IssueRefInfo {
            id: 101,
            status: 10, // new
            summary: "Crash on startup".to_string(),
        },
    );
    issue_refs.insert(
        102,
        IssueRefInfo {
            id: 102,
            status: 80, // resolved
            summary: "Fix memory leak".to_string(),
        },
    );
    issue_refs.insert(
        103,
        IssueRefInfo {
            id: 103,
            status: 90, // closed
            summary: "Deprecate old API".to_string(),
        },
    );

    let mut note_refs = HashMap::new();
    note_refs.insert(501, 101); // note 501 on bug 101
    note_refs.insert(502, 102); // note 502 on bug 102

    let mut valid_usernames = HashSet::new();
    valid_usernames.insert("admin".to_string());
    valid_usernames.insert("alice".to_string());
    valid_usernames.insert("bob_builder".to_string());

    FormatContext {
        enabled,
        issue_refs,
        note_refs,
        valid_usernames,
    }
}

#[test]
fn test_disabled_returns_escaped_plain_text() {
    let ctx = sample_context(false);
    let input = "Hello #101 ~501 @alice **bold** <script>alert(1)</script>";
    let result = format_text(input, &ctx);

    // When disabled, no HTML formatting tags should be produced
    assert!(!result.contains("<a href="));
    assert!(!result.contains("<span class=\"mention\">"));
    assert!(!result.contains("<strong>"));
    assert!(!result.contains("<script>"));
    // Raw HTML must be safely escaped
    assert!(result.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
    assert!(result.contains("#101"));
}

#[test]
fn test_markdown_headings_and_heading_non_collision() {
    let ctx = sample_context(true);

    // H1 through H6 are supported when followed by space
    let h1 = format_text("# Level 1 Heading", &ctx);
    assert!(h1.contains("<h1>Level 1 Heading</h1>"));

    let h2 = format_text("## Level 2 Heading", &ctx);
    assert!(h2.contains("<h2>Level 2 Heading</h2>"));

    let h3 = format_text("### Level 3 Heading", &ctx);
    assert!(h3.contains("<h3>Level 3 Heading</h3>"));

    let h6 = format_text("###### Level 6 Heading", &ctx);
    assert!(h6.contains("<h6>Level 6 Heading</h6>"));

    // CRITICAL: A line starting with #101 must NOT become <h1>!
    let issue_line = format_text("#101 is the top priority", &ctx);
    assert!(!issue_line.contains("<h1>"));
    assert!(issue_line.contains("<a href=\"/view_bug_page/101\""));
    assert!(issue_line.contains("#0000101"));
}

#[test]
fn test_markdown_lists_and_quotes_and_hr() {
    let ctx = sample_context(true);

    let text = "- Item 1\n- Item 2\n\n1. First\n2. Second\n\n---\n\n> Quoted text\n> Line two";
    let formatted = format_text(text, &ctx);

    assert!(formatted.contains("<ul><li>Item 1</li><li>Item 2</li></ul>"));
    assert!(formatted.contains("<ol><li>First</li><li>Second</li></ol>"));
    assert!(formatted.contains("<hr>"));
    assert!(formatted.contains("<blockquote>Quoted text<br>Line two</blockquote>"));
}

#[test]
fn test_code_blocks_and_inline_code_protection() {
    let ctx = sample_context(true);

    let code_input =
        "```rust\nfn main() {\n    let x = \"#101 ~501 @alice\";\n}\n```\n\nUse `cargo test #101`";
    let formatted = format_text(code_input, &ctx);

    // Fenced code block with language
    assert!(formatted.contains(r#"<pre class="code-block"><code class="language-rust">"#));
    assert!(formatted.contains("let x = &quot;#101 ~501 @alice&quot;;"));
    // Tokens inside code block must NOT be linked or highlighted
    assert!(!formatted.contains("/view_bug_page/101"));
    assert!(!formatted.contains("class=\"mention\""));

    // Inline code
    assert!(formatted.contains(r#"<code class="inline-code">cargo test #101</code>"#));
}

#[test]
fn test_inline_styles() {
    let ctx = sample_context(true);

    let input = "Text with **bold**, *italic*, and ~~deleted~~ words.";
    let formatted = format_text(input, &ctx);

    assert!(formatted.contains("<strong>bold</strong>"));
    assert!(formatted.contains("<em>italic</em>"));
    assert!(formatted.contains("<del>deleted</del>"));
}

#[test]
fn test_issue_refs_open_vs_resolved_and_unauthorized_leakage() {
    let ctx = sample_context(true);

    // Open bug #101 (status 10)
    let open_res = format_text("Check #101 now", &ctx);
    assert!(open_res.contains(
        r##"<a href="/view_bug_page/101" title="#0000101: Crash on startup [new]">#0000101</a>"##
    ));
    assert!(!open_res.contains("issue-link-resolved"));

    // Resolved bug #102 (status 80) -> strikethrough styling
    let res_res = format_text("Check #102 now", &ctx);
    assert!(res_res.contains(
        r##"<a href="/view_bug_page/102" class="issue-link-resolved" title="#0000102: Fix memory leak [resolved]"><del>#0000102</del></a>"##
    ));

    // Closed bug #103 (status 90) -> strikethrough styling
    let closed_res = format_text("Check #103 now", &ctx);
    assert!(closed_res.contains("issue-link-resolved"));
    assert!(closed_res.contains("<del>#0000103</del>"));

    // Missing / inaccessible bug #999 -> NO title leak, NO link!
    let missing_res = format_text("Check #999 secret issue", &ctx);
    assert!(!missing_res.contains("/view_bug_page/999"));
    assert!(!missing_res.contains("title="));
    assert!(missing_res.contains("#999"));
}

#[test]
fn test_note_refs_linking() {
    let ctx = sample_context(true);

    // Visible note 501 on bug 101
    let res = format_text("See note ~501 for details", &ctx);
    assert!(res.contains(r#"<a href="/view_bug_page/101#c501">~0000501</a>"#));

    // Inaccessible / nonexistent note 999
    let missing = format_text("See note ~999 for details", &ctx);
    assert!(!missing.contains("/view_bug_page/"));
    assert!(!missing.contains("#c999"));
    assert!(missing.contains("~999"));
}

#[test]
fn test_autolink_bare_urls_and_markdown_links() {
    let ctx = sample_context(true);

    // Bare URL with trailing punctuation
    let bare = format_text("Visit https://example.com/test?a=1&b=2.", &ctx);
    assert!(bare.contains(
        r#"<a href="https://example.com/test?a=1&amp;b=2" target="_blank" rel="noopener noreferrer nofollow">https://example.com/test?a=1&amp;b=2</a>."#
    ));

    // Bare "http://" or "https://" without host MUST NOT be autolinked
    let bare_no_host = format_text("Just http:// or https:// alone without host.", &ctx);
    assert!(!bare_no_host.contains("<a href="));

    // Markdown link with safe https scheme
    let md = format_text("[Doc site](https://docs.mantisbt.org)", &ctx);
    assert!(md.contains(
        r#"<a href="https://docs.mantisbt.org" target="_blank" rel="noopener noreferrer nofollow">Doc site</a>"#
    ));

    // Markdown link with dangerous javascript: scheme MUST be blocked
    let dangerous = format_text("[Bad Link](javascript:alert(document.cookie))", &ctx);
    assert!(!dangerous.contains("<a href="));
    assert!(dangerous.contains("[Bad Link](javascript:alert(document.cookie))"));

    // Markdown link with dangerous data: scheme MUST be blocked
    let data_scheme = format_text("[Data URI](data:text/html,<script>alert(1)</script>)", &ctx);
    assert!(!data_scheme.contains("<a href="));
}

#[test]
fn test_mentions_highlight_and_email_non_collision() {
    let ctx = sample_context(true);

    // Valid mention
    let res = format_text("cc @alice and @admin.", &ctx);
    assert!(res.contains(r#"<span class="mention">@alice</span>"#));
    assert!(res.contains(r#"<span class="mention">@admin</span>"#));

    // Valid mention with underscore
    let res2 = format_text("cc @bob_builder!", &ctx);
    assert!(res2.contains(r#"<span class="mention">@bob_builder</span>"#));

    // Non-existent username
    let unknown = format_text("cc @unknown_person", &ctx);
    assert!(!unknown.contains("class=\"mention\""));
    assert!(unknown.contains("@unknown_person"));

    // Email addresses MUST NOT be treated as mentions
    let email_text = "Contact support@example.com or alice@corp.local for help.";
    let res_email = format_text(email_text, &ctx);
    assert!(!res_email.contains("class=\"mention\""));
}

#[test]
fn test_exhaustive_xss_protection() {
    let ctx = sample_context(true);

    let attack_vectors = vec![
        "<script>alert('XSS')</script>",
        "<img src=x onerror=alert('img')>",
        "<svg/onload=alert('svg')>",
        "javascript:/*--></title></style></textarea></script></xmp><svg/onload='+/'/+/onmouseover=1/+/[*/[]/+alert(1)//'>",
        "[Click](javascript:alert(1))",
        "[Click](data:text/html;base64,PHNjcmlwdD5hbGVydCgxKTwvc2NyaXB0Pg==)",
        "[Click](vbscript:msgbox(1))",
        "https://example.com/\"onmouseover=\"alert(1)",
        "## Heading <script>alert(1)</script>",
        "- List item <img src=x onerror=alert(1)>",
        "> Quote <svg onload=alert(1)>",
    ];

    for attack in attack_vectors {
        let output = format_text(attack, &ctx);
        assert!(
            !output.contains("<script>"),
            "Failed XSS check for <script>: {}",
            output
        );
        assert!(
            !output.contains("<img"),
            "Failed XSS check for <img: {}",
            output
        );
        assert!(
            !output.contains("<svg"),
            "Failed XSS check for <svg: {}",
            output
        );
        assert!(
            !output.contains("href=\"javascript:"),
            "Failed XSS check for javascript: href: {}",
            output
        );
        assert!(
            !output.contains("href=\"data:"),
            "Failed XSS check for data: href: {}",
            output
        );
        assert!(
            !output.contains("href=\"vbscript:"),
            "Failed XSS check for vbscript: href: {}",
            output
        );
    }
}

#[test]
fn test_attribute_breakout_in_issue_summary_is_escaped() {
    let mut issue_refs = HashMap::new();
    issue_refs.insert(
        201,
        IssueRefInfo {
            id: 201,
            status: 10,
            summary: "\"><script>alert('pwned')</script><span attr=\"".to_string(),
        },
    );
    let ctx = FormatContext {
        enabled: true,
        issue_refs,
        note_refs: HashMap::new(),
        valid_usernames: HashSet::new(),
    };

    let result = format_text("Check out #201 now", &ctx);
    // Attribute breakout must be prevented
    assert!(!result.contains("<script>"));
    assert!(!result.contains("<span attr="));
    assert!(result.contains("&quot;&gt;&lt;script&gt;alert(&#x27;pwned&#x27;)&lt;/script&gt;"));
    assert!(result.contains("<a href=\"/view_bug_page/201\""));
}

#[test]
fn test_plugin_toggle_on_vs_off_identical_input() {
    let text = "Issue #101 has **bold** and `code` with @alice on https://example.com";
    let ctx_off = sample_context(false);
    let ctx_on = sample_context(true);

    let res_off = format_text(text, &ctx_off);
    let res_on = format_text(text, &ctx_on);

    // When off: exact html_escape of raw string, no tags synthesized
    assert_eq!(res_off, text); // no special chars to escape
    assert!(!res_off.contains("<div"));
    assert!(!res_off.contains("<a href="));
    assert!(!res_off.contains("<strong>"));

    // When on: full markup synthesized
    assert!(res_on.contains("<div class=\"formatted-text\">"));
    assert!(res_on.contains("<a href=\"/view_bug_page/101\""));
    assert!(res_on.contains("<strong>bold</strong>"));
    assert!(res_on.contains("<code class=\"inline-code\">code</code>"));
    assert!(res_on.contains("<span class=\"mention\">@alice</span>"));
    assert!(res_on.contains("<a href=\"https://example.com\""));
}

#[test]
fn test_scanning_helpers() {
    let raw = r#"
Here is issue #101 and #102.
Also note ~501 and ~502.
Please ask @alice and @bob_builder.
Ignore email support@example.com and user.name+tag@sub.domain.org.

```rust
// In code block: #999, ~999, @fake_user, dev@corp.com
let x = 1;
```

Inline `code #888 ~888 @inline_fake` should also be ignored.
"#;

    let issues = scan_issue_refs(raw);
    assert_eq!(issues, vec![101, 102]);

    let notes = scan_note_refs(raw);
    assert_eq!(notes, vec![501, 502]);

    let mentions = extract_mentions(raw);
    assert_eq!(
        mentions,
        vec!["alice".to_string(), "bob_builder".to_string()]
    );
}

#[test]
fn test_unicode_vietnamese_markdown_links() {
    let ctx = sample_context(true);
    let raw = "Lỗi [tài liệu](https://example.com/docs) xem thêm chi tiết.";
    let res = format_text(raw, &ctx);

    assert!(res.contains("Lỗi <a href=\"https://example.com/docs\" target=\"_blank\" rel=\"noopener noreferrer nofollow\">tài liệu</a> xem thêm chi tiết."));

    // Multi-byte text with emoji and without links
    let no_links = "Văn bản có dấu tiếng Việt: Thử nghiệm không có link và emoji 🎉.";
    let res_no_links = format_text(no_links, &ctx);
    assert!(
        res_no_links.contains("Văn bản có dấu tiếng Việt: Thử nghiệm không có link và emoji 🎉.")
    );
}

#[test]
fn test_autolink_edge_cases() {
    let ctx = sample_context(true);

    // 1. Quoted URL
    let quoted = r#"Check "https://example.com" please."#;
    let res_quoted = format_text(quoted, &ctx);
    assert!(res_quoted.contains("&quot;<a href=\"https://example.com\" target=\"_blank\" rel=\"noopener noreferrer nofollow\">https://example.com</a>&quot;"));

    // 2. Angle brackets
    let angle = "Check <https://example.com> please.";
    let res_angle = format_text(angle, &ctx);
    assert!(res_angle.contains("&lt;<a href=\"https://example.com\" target=\"_blank\" rel=\"noopener noreferrer nofollow\">https://example.com</a>&gt;"));

    // 3. Parentheses
    let paren = "Parentheses (https://example.com) test.";
    let res_paren = format_text(paren, &ctx);
    assert!(res_paren.contains("(<a href=\"https://example.com\" target=\"_blank\" rel=\"noopener noreferrer nofollow\">https://example.com</a>)"));

    // 4. Wikipedia URL with balanced parentheses
    let wiki = "Wikipedia: https://en.wikipedia.org/wiki/Rust_(programming_language) is great.";
    let res_wiki = format_text(wiki, &ctx);
    assert!(
        res_wiki.contains(r#"href="https://en.wikipedia.org/wiki/Rust_(programming_language)""#)
    );
    assert!(res_wiki
        .contains(r#">https://en.wikipedia.org/wiki/Rust_(programming_language)</a> is great."#));

    // 5. Trailing sentence punctuation
    let trailing = "Visit https://example.com.";
    let res_trailing = format_text(trailing, &ctx);
    assert!(res_trailing.contains(r#"<a href="https://example.com" target="_blank" rel="noopener noreferrer nofollow">https://example.com</a>."#));

    // 6. Query string with ampersands
    let query = "API https://example.com/api?user=1&flag=2 test";
    let res_query = format_text(query, &ctx);
    assert!(res_query.contains(r#"href="https://example.com/api?user=1&amp;flag=2""#));

    // 7. Parentheses AND trailing period: (see https://x.com/foo).
    let paren_period = "Reference (see https://example.com/foo).";
    let res_paren_period = format_text(paren_period, &ctx);
    assert!(res_paren_period.contains(r#"(see <a href="https://example.com/foo" target="_blank" rel="noopener noreferrer nofollow">https://example.com/foo</a>)."#));

    // 8. Wikipedia URL with enclosing parentheses AND trailing period
    let wiki_paren_period = "(Check https://en.wikipedia.org/wiki/Rust_(programming_language)).";
    let res_wiki_paren_period = format_text(wiki_paren_period, &ctx);
    assert!(res_wiki_paren_period.contains(r#"(Check <a href="https://en.wikipedia.org/wiki/Rust_(programming_language)" target="_blank" rel="noopener noreferrer nofollow">https://en.wikipedia.org/wiki/Rust_(programming_language)</a>)."#));
}

#[test]
fn test_nested_placeholders_in_markdown_links() {
    let ctx = sample_context(true);
    let raw = "Link with inline code: [`my_code` documentation](https://example.com/doc)";
    let res = format_text(raw, &ctx);
    assert!(res.contains(r#"<a href="https://example.com/doc" target="_blank" rel="noopener noreferrer nofollow"><code class="inline-code">my_code</code> documentation</a>"#));
}

#[test]
fn test_sentinel_sanitization() {
    let ctx = sample_context(true);
    let malicious = "Attempted sentinel injection: \x01P0X\x02 test.";
    let res = format_text(malicious, &ctx);
    assert!(!res.contains("\x01"));
    assert!(!res.contains("\x02"));
    assert!(res.contains("Attempted sentinel injection: P0X test."));
}

#[test]
fn test_usernames_with_trailing_underscore_and_punctuation() {
    let mut ctx = sample_context(true);
    ctx.valid_usernames.insert("bob_".to_string());
    ctx.valid_usernames.insert("alice".to_string());

    // 1. Username ending with underscore
    let raw_under = "Hello @bob_ please review.";
    let res_under = format_text(raw_under, &ctx);
    assert!(res_under.contains(r#"<span class="mention">@bob_</span>"#));

    // 2. Mention at end of sentence with trailing period
    let raw_period = "Contact @alice.";
    let res_period = format_text(raw_period, &ctx);
    assert!(res_period.contains(r#"<span class="mention">@alice</span>."#));
}

#[test]
fn test_unclosed_backtick_consistency() {
    let raw = "Unclosed backtick ` with mention @alice and issue #101";
    let mentions = extract_mentions(raw);
    assert_eq!(mentions, vec!["alice".to_string()]);

    let issues = scan_issue_refs(raw);
    assert_eq!(issues, vec![101]);
}

#[test]
fn test_issue_summary_sentinel_does_not_break_out_or_expand() {
    let mut ctx = sample_context(true);
    ctx.issue_refs.insert(
        101,
        IssueRefInfo {
            id: 101,
            status: 50,
            summary: "Malicious \x01P0X\x02 summary \" onmouseover=\"alert(1)".to_string(),
        },
    );

    let raw = "Check issue #101 for details";
    let res = format_text(raw, &ctx);
    // Sentinel bytes must be stripped
    assert!(!res.contains("\x01"));
    assert!(!res.contains("\x02"));
    // Title attribute must be safely escaped and not contain executable breakout
    assert!(res.contains(
        r##"title="#0000101: Malicious P0X summary &quot; onmouseover=&quot;alert(1) [assigned]""##
    ));
    assert!(res.contains(r#"<a href="/view_bug_page/101""#));
}

#[test]
fn test_empty_heading_not_rendered_as_h1() {
    let ctx = sample_context(true);
    let raw = "#\n# \n## \n###   \nValid text";
    let res = format_text(raw, &ctx);
    assert!(!res.contains("<h1"));
    assert!(!res.contains("<h2"));
    assert!(!res.contains("<h3"));
    assert!(res.contains("Valid text"));
}

#[test]
fn test_bare_http_or_fragment_not_autolinked() {
    let ctx = sample_context(true);
    let raw = "Visit http:// or http://# or https://?foo or https://example.com/ok";
    let res = format_text(raw, &ctx);
    assert!(!res.contains(r#"href="http://""#));
    assert!(!res.contains(r#"href="http://#""#));
    assert!(!res.contains(r#"href="https://?foo""#));
    assert!(res.contains(r#"<a href="https://example.com/ok" target="_blank" rel="noopener noreferrer nofollow">https://example.com/ok</a>"#));
}

#[test]
fn test_markdown_link_with_space_in_url_rejected() {
    let ctx = sample_context(true);
    let raw = "Bad link: [Click here](https://example.com/foo bar) test";
    let res = format_text(raw, &ctx);
    assert!(!res.contains(r#"href="https://example.com/foo bar""#));
    assert!(res.contains("[Click here]("));
    assert!(res.contains(" bar)"));
}

#[test]
fn test_markdown_link_with_inline_code_in_url_rejected() {
    let ctx = sample_context(true);
    let raw = "Link with code in URL: [docs](https://example.com/`x`) test";
    let res = format_text(raw, &ctx);
    // Sentinel markers must NOT end up in href or anywhere in the output
    assert!(!res.contains("\x01"));
    assert!(!res.contains("\x02"));
    // The markdown link [docs](...) must NOT be constructed with placeholder in href
    assert!(!res.contains(r#"<a href="https://example.com/&#x1;P"#));
    assert!(!res.contains(r#"<a href="https://example.com/\x01"#));
    assert!(!res.contains(r#">docs</a>"#));
    assert!(res.contains("[docs]("));
    assert!(res.contains(r#"<code class="inline-code">x</code>"#));
}

#[test]
fn test_markdown_bracket_spam_performance() {
    let ctx = sample_context(true);
    // 200 KB of '[' followed by a single '](https://example.com)' at the end
    let mut input = "[".repeat(200_000);
    input.push_str("](https://example.com)");

    let start = std::time::Instant::now();
    let res = format_text(&input, &ctx);
    let elapsed = start.elapsed();

    assert!(
        elapsed < std::time::Duration::from_millis(500),
        "200KB bracket spam took {:?}, must be under 500ms (O(N) linear time)",
        elapsed
    );
    assert!(res.len() > 100_000);
}

#[test]
fn test_bare_url_autolink_scheme_spam_performance() {
    let ctx = sample_context(true);
    // 200 KB of 'http://.' repeated
    let input = "http://.".repeat(25_000);

    let start = std::time::Instant::now();
    let res = format_text(&input, &ctx);
    let elapsed = start.elapsed();

    assert!(
        elapsed < std::time::Duration::from_millis(500),
        "200KB scheme spam took {:?}, must be under 500ms (O(N) linear time)",
        elapsed
    );
    assert!(!res.contains(r#"href="http://.""#));
}

#[test]
fn test_mentions_in_urls_not_extracted_or_highlighted() {
    let mut ctx = sample_context(true);
    ctx.valid_usernames.insert("alice".to_string());
    ctx.valid_usernames.insert("bob".to_string());

    let raw = "Check https://medium.com/@alice/post and http://example.com/api?user=@bob.";
    let extracted = extract_mentions(raw);
    assert!(
        extracted.is_empty(),
        "Mentions inside URL paths and query strings must not be extracted: {:?}",
        extracted
    );

    let res = format_text(raw, &ctx);
    assert!(
        !res.contains(r#"<span class="mention">@alice</span>"#),
        "URL path @alice must not be highlighted as a user mention"
    );
    assert!(
        !res.contains(r#"<span class="mention">@bob</span>"#),
        "URL query @bob must not be highlighted as a user mention"
    );
}

#[test]
fn test_unclosed_markdown_link_performance() {
    let ctx = sample_context(true);
    // 200 KB of '[](' repeated with no closing ')'
    let input = "[](".repeat(68_000);

    let start = std::time::Instant::now();
    let res = format_text(&input, &ctx);
    let elapsed = start.elapsed();

    assert!(
        elapsed < std::time::Duration::from_millis(500),
        "200KB unclosed link spam took {:?}, must be under 500ms (O(N) linear time)",
        elapsed
    );
    assert!(res.len() > 100_000);
}

#[test]
fn test_collect_mentions_deduplication_and_cap() {
    let text1 = "Ping @alice and @bob and @ALICE";
    let text2 = "Also notify @charlie and @alice";
    let res = collect_mentions(&[text1, text2]);
    assert_eq!(res.len(), 3);
    assert_eq!(res, vec!["alice", "bob", "charlie"]);
}

#[test]
fn test_combine_context_helper() {
    let combined = combine_context(
        "Default Summary",
        &["First paragraph", "", "Second paragraph"],
    );
    assert_eq!(combined, "First paragraph\n\nSecond paragraph");

    let fallback = combine_context("Fallback Summary", &["", "   ", "\n"]);
    assert_eq!(fallback, "Fallback Summary");
}

#[test]
fn test_ordered_list_year_non_collision() {
    let ctx = sample_context(true);

    // 1. Standalone 4-digit year line must not become an ordered list
    let input_standalone = "2024. was an important milestone year.";
    let res_standalone = format_text(input_standalone, &ctx);
    assert!(
        !res_standalone.contains("<ol>"),
        "Standalone year 2024. must not become an ordered list: {}",
        res_standalone
    );
    assert!(res_standalone.contains("<p>2024. was an important milestone year.</p>"));

    // 2. Year line inside a paragraph must not interrupt the paragraph
    let input_para = "Project history:\n2024. was an important milestone year.";
    let res_para = format_text(input_para, &ctx);
    assert!(
        !res_para.contains("<ol>"),
        "Year inside paragraph must not interrupt into an ordered list: {}",
        res_para
    );
    assert!(res_para.contains("Project history:<br>2024. was an important milestone year."));

    // 3. Legitimate ordered list starting with 1. works correctly
    let input_valid = "1. First step\n2. Second step\n3. Third step";
    let res_valid = format_text(input_valid, &ctx);
    assert!(
        res_valid.contains("<ol><li>First step</li><li>Second step</li><li>Third step</li></ol>"),
        "Valid ordered list must be rendered: {}",
        res_valid
    );
}

#[test]
fn test_arithmetic_asterisk_non_collision() {
    let ctx = sample_context(true);

    // 1. Arithmetic expressions must not turn into <em>
    let input = "Calculate 2 * 3 * 4 = 24 and 5 * 10 = 50.";
    let res = format_text(input, &ctx);
    assert!(
        !res.contains("<em>"),
        "Arithmetic with asterisks must not be parsed as italic: {}",
        res
    );
    assert!(res.contains("Calculate 2 * 3 * 4 = 24 and 5 * 10 = 50."));

    // 2. Spaces adjacent to delimiters must not activate emphasis
    let input_spaces = "This * not italic * and ** not bold ** text.";
    let res_spaces = format_text(input_spaces, &ctx);
    assert!(
        !res_spaces.contains("<em>") && !res_spaces.contains("<strong>"),
        "Spaced delimiters must not trigger formatting: {}",
        res_spaces
    );

    // 3. Valid emphasis adjacent to arithmetic works as expected
    let input_mixed = "Formula: 2 * 3 equals *six* and **bold result**.";
    let res_mixed = format_text(input_mixed, &ctx);
    assert!(res_mixed.contains("2 * 3 equals <em>six</em> and <strong>bold result</strong>."));
}

#[test]
fn test_autolink_trailing_dots_dos_performance() {
    let ctx = sample_context(true);
    // 200,000 trailing periods after a URL: must run in O(N) linear time without quadratic buffer shifting
    let input = format!("https://a{}", ".".repeat(200_000));

    let start = std::time::Instant::now();
    let res = format_text(&input, &ctx);
    let elapsed = start.elapsed();

    assert!(
        elapsed < std::time::Duration::from_millis(500),
        "200K trailing dots autolink took {:?}, must be under 500ms (O(N) linear time)",
        elapsed
    );
    assert!(res.contains(r#"href="https://a""#));
}

#[test]
fn test_triple_asterisk_bold_italic_proper_nesting() {
    let ctx = sample_context(true);
    let input = "***bold and italic***";
    let res = format_text(input, &ctx);
    assert!(
        res.contains("<strong><em>bold and italic</em></strong>"),
        "Triple asterisk must produce properly nested tags: {}",
        res
    );
}

#[test]
fn test_ordered_list_custom_start_number() {
    let ctx = sample_context(true);
    let input = "3. Third step\n4. Fourth step";
    let res = format_text(input, &ctx);
    assert!(
        res.contains(r#"<ol start="3"><li>Third step</li><li>Fourth step</li></ol>"#),
        "Ordered list starting with 3 must set start attribute: {}",
        res
    );
}

#[test]
fn test_diff_new_mentions_helper() {
    let mut old_mentions = HashSet::new();
    old_mentions.insert("alice".to_string());
    old_mentions.insert("bob".to_string());
    let new_mentions = vec![
        "ALICE".to_string(),
        "charlie".to_string(),
        "BOB".to_string(),
        "david".to_string(),
        "charlie".to_string(),
    ];
    let diff = diff_new_mentions(&old_mentions, &new_mentions);
    assert_eq!(diff, vec!["charlie", "david"]);
}

#[test]
fn test_markdown_bracket_spam_with_2kb_url_zero_alloc_performance() {
    let ctx = sample_context(true);
    // 1000 '[' followed by '](', a 2KB non-http string, and ')'
    let input = format!("{}]({})", "[".repeat(1000), "x".repeat(2000));

    let start = std::time::Instant::now();
    let res = format_text(&input, &ctx);
    let elapsed = start.elapsed();

    assert!(
        elapsed < std::time::Duration::from_millis(200),
        "1000 brackets before 2KB url took {:?}, must be under 200ms (zero redundant allocs)",
        elapsed
    );
    assert!(!res.contains("<a href"));
}
