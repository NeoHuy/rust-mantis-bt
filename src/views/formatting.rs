use std::collections::{HashMap, HashSet};

pub use crate::models::{
    collect_mentions, combine_context, diff_new_mentions, extract_mentions, scan_issue_refs,
    scan_note_refs,
};
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

const MAX_BLOCKQUOTE_DEPTH: usize = 16;

/// Sanitizes a Markdown fence info string into a safe CSS `language-{lang}` suffix,
/// retaining only ASCII alphanumeric characters, `-`, `_`, and `+`.
pub fn sanitize_lang_class(lang: &str) -> String {
    lang.split(|c: char| c.is_whitespace() || c == ',')
        .next()
        .unwrap_or("")
        .chars()
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '+'))
        .take(64)
        .collect()
}

fn push_hl_span(out: &mut String, cls: &str, raw_slice: &str) {
    out.push_str("<span class=\"");
    out.push_str(cls);
    out.push_str("\">");
    out.push_str(&html_escape(raw_slice));
    out.push_str("</span>");
}

struct LangSpec {
    line_comments: &'static [&'static str],
    block_comment: bool,
    triple_quotes: bool,
    backtick_strings: bool,
    single_quote_strings: bool,
    dollar_vars: bool,
    rust_attrs: bool,
    c_preproc: bool,
    at_attr: bool,
    case_insensitive_kw: bool,
    pascal_case_types: bool,
    keywords: &'static [&'static str],
    types: &'static [&'static str],
}

const RUST_SPEC: LangSpec = LangSpec {
    line_comments: &["//"],
    block_comment: true,
    triple_quotes: false,
    backtick_strings: false,
    single_quote_strings: false,
    dollar_vars: false,
    rust_attrs: true,
    c_preproc: false,
    at_attr: false,
    case_insensitive_kw: false,
    pascal_case_types: true,
    keywords: &[
        "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum",
        "extern", "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move",
        "mut", "pub", "ref", "return", "self", "Self", "static", "struct", "super", "trait",
        "true", "type", "unsafe", "use", "where", "while",
    ],
    types: &[
        "bool", "char", "f32", "f64", "i8", "i16", "i32", "i64", "i128", "isize", "str", "u8",
        "u16", "u32", "u64", "u128", "usize", "String", "Vec", "Option", "Result", "Box",
        "HashMap", "HashSet", "BTreeMap", "BTreeSet",
    ],
};

const PYTHON_SPEC: LangSpec = LangSpec {
    line_comments: &["#"],
    block_comment: false,
    triple_quotes: true,
    backtick_strings: false,
    single_quote_strings: true,
    dollar_vars: false,
    rust_attrs: false,
    c_preproc: false,
    at_attr: true,
    case_insensitive_kw: false,
    pascal_case_types: true,
    keywords: &[
        "and", "as", "assert", "async", "await", "break", "class", "continue", "def", "del",
        "elif", "else", "except", "False", "finally", "for", "from", "global", "if", "import",
        "in", "is", "lambda", "None", "nonlocal", "not", "or", "pass", "raise", "return", "True",
        "try", "while", "with", "yield",
    ],
    types: &[
        "int", "float", "str", "bool", "list", "dict", "tuple", "set", "bytes", "object", "Any",
        "Optional", "List", "Dict", "Tuple", "Set", "Union",
    ],
};

const JS_TS_SPEC: LangSpec = LangSpec {
    line_comments: &["//"],
    block_comment: true,
    triple_quotes: false,
    backtick_strings: true,
    single_quote_strings: true,
    dollar_vars: false,
    rust_attrs: false,
    c_preproc: false,
    at_attr: true,
    case_insensitive_kw: false,
    pascal_case_types: true,
    keywords: &[
        "as",
        "async",
        "await",
        "break",
        "case",
        "catch",
        "class",
        "const",
        "continue",
        "debugger",
        "default",
        "delete",
        "do",
        "else",
        "enum",
        "export",
        "extends",
        "false",
        "finally",
        "for",
        "from",
        "function",
        "if",
        "implements",
        "import",
        "in",
        "instanceof",
        "interface",
        "let",
        "new",
        "null",
        "of",
        "package",
        "private",
        "protected",
        "public",
        "readonly",
        "return",
        "static",
        "super",
        "switch",
        "this",
        "throw",
        "true",
        "try",
        "type",
        "typeof",
        "undefined",
        "var",
        "void",
        "while",
        "with",
        "yield",
    ],
    types: &[
        "any", "boolean", "bigint", "never", "number", "object", "string", "symbol", "unknown",
        "Array", "Promise", "Record", "Partial", "Map", "Set", "Date", "Error",
    ],
};

const SQL_SPEC: LangSpec = LangSpec {
    line_comments: &["--"],
    block_comment: true,
    triple_quotes: false,
    backtick_strings: false,
    single_quote_strings: true,
    dollar_vars: false,
    rust_attrs: false,
    c_preproc: false,
    at_attr: false,
    case_insensitive_kw: true,
    pascal_case_types: false,
    keywords: &[
        "ADD",
        "ALL",
        "ALTER",
        "AND",
        "AS",
        "ASC",
        "BEGIN",
        "BETWEEN",
        "BY",
        "CASE",
        "CHECK",
        "COLUMN",
        "COMMIT",
        "CONSTRAINT",
        "CREATE",
        "CROSS",
        "DEFAULT",
        "DELETE",
        "DESC",
        "DISTINCT",
        "DROP",
        "ELSE",
        "END",
        "EXISTS",
        "FALSE",
        "FOREIGN",
        "FROM",
        "FULL",
        "GROUP",
        "HAVING",
        "IF",
        "IN",
        "INDEX",
        "INNER",
        "INSERT",
        "INTO",
        "IS",
        "JOIN",
        "KEY",
        "LEFT",
        "LIKE",
        "LIMIT",
        "NOT",
        "NULL",
        "OFFSET",
        "ON",
        "OR",
        "ORDER",
        "OUTER",
        "PRIMARY",
        "REFERENCES",
        "RETURNING",
        "RIGHT",
        "ROLLBACK",
        "SELECT",
        "SET",
        "TABLE",
        "THEN",
        "TRUE",
        "UNION",
        "UNIQUE",
        "UPDATE",
        "VALUES",
        "WHEN",
        "WHERE",
        "WITH",
    ],
    types: &[
        "BIGINT",
        "BLOB",
        "BOOLEAN",
        "CHAR",
        "DATE",
        "DATETIME",
        "DECIMAL",
        "DOUBLE",
        "FLOAT",
        "INT",
        "INTEGER",
        "JSON",
        "NUMERIC",
        "REAL",
        "SMALLINT",
        "TEXT",
        "TIME",
        "TIMESTAMP",
        "VARCHAR",
    ],
};

const BASH_SPEC: LangSpec = LangSpec {
    line_comments: &["#"],
    block_comment: false,
    triple_quotes: false,
    backtick_strings: false,
    single_quote_strings: true,
    dollar_vars: true,
    rust_attrs: false,
    c_preproc: false,
    at_attr: false,
    case_insensitive_kw: false,
    pascal_case_types: false,
    keywords: &[
        "break", "case", "continue", "do", "done", "elif", "else", "esac", "exit", "export", "fi",
        "for", "function", "if", "in", "local", "readonly", "return", "select", "then", "until",
        "while", "true", "false",
    ],
    types: &[
        "echo", "printf", "cd", "pwd", "ls", "cat", "grep", "sed", "awk", "curl", "cargo", "git",
        "sudo", "source", "unset", "shift", "test",
    ],
};

const PHP_SPEC: LangSpec = LangSpec {
    line_comments: &["//", "#"],
    block_comment: true,
    triple_quotes: false,
    backtick_strings: false,
    single_quote_strings: true,
    dollar_vars: true,
    rust_attrs: false,
    c_preproc: false,
    at_attr: false,
    case_insensitive_kw: false,
    pascal_case_types: true,
    keywords: &[
        "abstract",
        "and",
        "array",
        "as",
        "break",
        "callable",
        "case",
        "catch",
        "class",
        "clone",
        "const",
        "continue",
        "declare",
        "default",
        "do",
        "echo",
        "else",
        "elseif",
        "empty",
        "enddeclare",
        "endfor",
        "endforeach",
        "endif",
        "endswitch",
        "endwhile",
        "extends",
        "false",
        "final",
        "finally",
        "fn",
        "for",
        "foreach",
        "function",
        "global",
        "goto",
        "if",
        "implements",
        "include",
        "include_once",
        "instanceof",
        "interface",
        "isset",
        "list",
        "match",
        "namespace",
        "new",
        "null",
        "or",
        "print",
        "private",
        "protected",
        "public",
        "readonly",
        "require",
        "require_once",
        "return",
        "static",
        "switch",
        "throw",
        "trait",
        "true",
        "try",
        "unset",
        "use",
        "var",
        "while",
        "xor",
        "yield",
    ],
    types: &[
        "int", "float", "bool", "string", "iterable", "object", "mixed", "void", "never", "self",
        "parent",
    ],
};

const GO_SPEC: LangSpec = LangSpec {
    line_comments: &["//"],
    block_comment: true,
    triple_quotes: false,
    backtick_strings: true,
    single_quote_strings: false,
    dollar_vars: false,
    rust_attrs: false,
    c_preproc: false,
    at_attr: false,
    case_insensitive_kw: false,
    pascal_case_types: true,
    keywords: &[
        "break",
        "case",
        "chan",
        "const",
        "continue",
        "default",
        "defer",
        "else",
        "fallthrough",
        "for",
        "func",
        "go",
        "goto",
        "if",
        "import",
        "interface",
        "map",
        "package",
        "range",
        "return",
        "select",
        "struct",
        "switch",
        "type",
        "var",
        "true",
        "false",
        "nil",
        "iota",
    ],
    types: &[
        "bool",
        "byte",
        "complex64",
        "complex128",
        "error",
        "float32",
        "float64",
        "int",
        "int8",
        "int16",
        "int32",
        "int64",
        "rune",
        "string",
        "uint",
        "uint8",
        "uint16",
        "uint32",
        "uint64",
        "uintptr",
        "any",
    ],
};

const C_CPP_JAVA_SPEC: LangSpec = LangSpec {
    line_comments: &["//"],
    block_comment: true,
    triple_quotes: false,
    backtick_strings: false,
    single_quote_strings: false,
    dollar_vars: false,
    rust_attrs: false,
    c_preproc: true,
    at_attr: true,
    case_insensitive_kw: false,
    pascal_case_types: true,
    keywords: &[
        "abstract",
        "auto",
        "break",
        "case",
        "catch",
        "class",
        "const",
        "constexpr",
        "continue",
        "default",
        "delete",
        "do",
        "else",
        "enum",
        "explicit",
        "export",
        "extends",
        "extern",
        "false",
        "final",
        "finally",
        "for",
        "friend",
        "goto",
        "if",
        "implements",
        "import",
        "inline",
        "instanceof",
        "interface",
        "mutable",
        "namespace",
        "native",
        "new",
        "noexcept",
        "null",
        "nullptr",
        "operator",
        "override",
        "package",
        "private",
        "protected",
        "public",
        "register",
        "return",
        "sizeof",
        "static",
        "struct",
        "super",
        "switch",
        "synchronized",
        "template",
        "this",
        "throw",
        "throws",
        "transient",
        "true",
        "try",
        "typedef",
        "typename",
        "union",
        "using",
        "virtual",
        "volatile",
        "while",
    ],
    types: &[
        "bool", "boolean", "byte", "char", "double", "float", "int", "long", "short", "signed",
        "size_t", "unsigned", "void", "String", "List", "Map", "Set", "std", "string", "vector",
    ],
};

fn scan_quoted_string(bytes: &[u8], start: usize, quote: u8, sql_doubled_quote: bool) -> usize {
    let len = bytes.len();
    let mut i = start + 1;
    while i < len {
        if bytes[i] == b'\\' && i + 1 < len {
            i += 2;
            continue;
        }
        if bytes[i] == quote {
            if sql_doubled_quote && i + 1 < len && bytes[i + 1] == quote {
                i += 2;
                continue;
            }
            return i + 1;
        }
        i += 1;
    }
    len
}

fn scan_number_end(bytes: &[u8], start: usize) -> usize {
    let len = bytes.len();
    let mut i = start;
    if i + 1 < len
        && bytes[i] == b'0'
        && matches!(bytes[i + 1], b'x' | b'X' | b'b' | b'B' | b'o' | b'O')
    {
        i += 2;
        while i < len && (bytes[i].is_ascii_hexdigit() || bytes[i] == b'_') {
            i += 1;
        }
    } else {
        while i < len && (bytes[i].is_ascii_digit() || bytes[i] == b'_') {
            i += 1;
        }
        if i + 1 < len && bytes[i] == b'.' && bytes[i + 1].is_ascii_digit() {
            i += 1;
            while i < len && (bytes[i].is_ascii_digit() || bytes[i] == b'_') {
                i += 1;
            }
        }
        if i < len && matches!(bytes[i], b'e' | b'E') {
            let mut j = i + 1;
            if j < len && matches!(bytes[j], b'+' | b'-') {
                j += 1;
            }
            if j < len && bytes[j].is_ascii_digit() {
                i = j;
                while i < len && (bytes[i].is_ascii_digit() || bytes[i] == b'_') {
                    i += 1;
                }
            }
        }
    }
    while i < len && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
        i += 1;
    }
    i
}

fn is_pascal_case(ident: &str) -> bool {
    let mut bytes = ident.bytes();
    match bytes.next() {
        Some(b) if b.is_ascii_uppercase() => bytes.any(|c| c.is_ascii_lowercase()),
        _ => false,
    }
}

fn match_word(word: &str, list: &[&str], case_insensitive: bool) -> bool {
    if case_insensitive {
        list.iter().any(|kw| word.eq_ignore_ascii_case(kw))
    } else {
        list.contains(&word)
    }
}

fn highlight_code_spec(raw_code: &str, spec: &LangSpec) -> String {
    let bytes = raw_code.as_bytes();
    let len = bytes.len();
    let mut out = String::with_capacity(len + len / 4);
    let mut plain_start = 0usize;
    let mut i = 0usize;

    while i < len {
        // Non-ASCII bytes cannot start ASCII syntax tokens; skip full UTF-8 sequence safely
        if bytes[i] >= 0x80 {
            i += 1;
            while i < len && bytes[i] >= 0x80 {
                i += 1;
            }
            continue;
        }

        // Line comments
        let mut matched_line_cmt = false;
        for &prefix in spec.line_comments {
            if bytes[i..].starts_with(prefix.as_bytes()) {
                if i > plain_start {
                    out.push_str(&html_escape(&raw_code[plain_start..i]));
                }
                let start = i;
                i += prefix.len();
                while i < len && bytes[i] != b'\n' {
                    i += 1;
                }
                push_hl_span(&mut out, "hl-cmt", &raw_code[start..i]);
                plain_start = i;
                matched_line_cmt = true;
                break;
            }
        }
        if matched_line_cmt {
            continue;
        }

        // Block comments /* ... */
        if spec.block_comment && bytes[i..].starts_with(b"/*") {
            if i > plain_start {
                out.push_str(&html_escape(&raw_code[plain_start..i]));
            }
            let start = i;
            i += 2;
            while i + 1 < len && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            i = if i + 1 < len { i + 2 } else { len };
            push_hl_span(&mut out, "hl-cmt", &raw_code[start..i]);
            plain_start = i;
            continue;
        }

        // Triple-quoted strings (Python)
        if spec.triple_quotes
            && (bytes[i..].starts_with(b"\"\"\"") || bytes[i..].starts_with(b"'''"))
        {
            if i > plain_start {
                out.push_str(&html_escape(&raw_code[plain_start..i]));
            }
            let quote = bytes[i];
            let delim = [quote, quote, quote];
            let start = i;
            i += 3;
            while i < len {
                if bytes[i] == b'\\' && i + 1 < len {
                    i += 2;
                    continue;
                }
                if bytes[i..].starts_with(&delim) {
                    i += 3;
                    break;
                }
                i += 1;
            }
            push_hl_span(&mut out, "hl-str", &raw_code[start..i]);
            plain_start = i;
            continue;
        }

        // Double-quoted strings
        if bytes[i] == b'"' {
            if i > plain_start {
                out.push_str(&html_escape(&raw_code[plain_start..i]));
            }
            let end = scan_quoted_string(bytes, i, b'"', false);
            push_hl_span(&mut out, "hl-str", &raw_code[i..end]);
            i = end;
            plain_start = i;
            continue;
        }

        // Backtick strings (JS/TS/Go)
        if spec.backtick_strings && bytes[i] == b'`' {
            if i > plain_start {
                out.push_str(&html_escape(&raw_code[plain_start..i]));
            }
            let end = scan_quoted_string(bytes, i, b'`', false);
            push_hl_span(&mut out, "hl-str", &raw_code[i..end]);
            i = end;
            plain_start = i;
            continue;
        }

        // Single-quoted strings or char literals
        if bytes[i] == b'\'' {
            if spec.single_quote_strings {
                if i > plain_start {
                    out.push_str(&html_escape(&raw_code[plain_start..i]));
                }
                let end = scan_quoted_string(bytes, i, b'\'', spec.case_insensitive_kw);
                push_hl_span(&mut out, "hl-str", &raw_code[i..end]);
                i = end;
                plain_start = i;
                continue;
            } else {
                // Bounded char literal check (e.g. 'a', '\n', '\x7f') vs Rust lifetime ('static)
                let mut k = i + 1;
                let limit = (i + 12).min(len);
                let mut found_close = None;
                while k < limit {
                    if bytes[k] == b'\n' || bytes[k] == b'\r' {
                        break;
                    }
                    if bytes[k] == b'\\' && k + 1 < limit {
                        k += 2;
                        continue;
                    }
                    if bytes[k] == b'\'' {
                        found_close = Some(k + 1);
                        break;
                    }
                    k += 1;
                }
                if let Some(end) = found_close {
                    if i > plain_start {
                        out.push_str(&html_escape(&raw_code[plain_start..i]));
                    }
                    push_hl_span(&mut out, "hl-str", &raw_code[i..end]);
                    i = end;
                    plain_start = i;
                    continue;
                }
                i += 1;
                continue;
            }
        }

        // Rust attributes: #[...] or #![...]
        if spec.rust_attrs && (bytes[i..].starts_with(b"#[") || bytes[i..].starts_with(b"#![")) {
            if i > plain_start {
                out.push_str(&html_escape(&raw_code[plain_start..i]));
            }
            let start = i;
            while i < len && bytes[i] != b']' && bytes[i] != b'\n' {
                i += 1;
            }
            if i < len && bytes[i] == b']' {
                i += 1;
            }
            push_hl_span(&mut out, "hl-attr", &raw_code[start..i]);
            plain_start = i;
            continue;
        }

        // C/C++ preprocessor directives (#include, #define, ...)
        if spec.c_preproc && bytes[i] == b'#' {
            let line_start_ok = {
                let mut p = i;
                let mut only_ws = true;
                while p > 0 && bytes[p - 1] != b'\n' {
                    p -= 1;
                    if !matches!(bytes[p], b' ' | b'\t') {
                        only_ws = false;
                        break;
                    }
                }
                only_ws
            };
            if line_start_ok {
                if i > plain_start {
                    out.push_str(&html_escape(&raw_code[plain_start..i]));
                }
                let start = i;
                i += 1;
                while i < len && matches!(bytes[i], b' ' | b'\t') {
                    i += 1;
                }
                while i < len && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
                push_hl_span(&mut out, "hl-attr", &raw_code[start..i]);
                plain_start = i;
                continue;
            }
        }

        // Decorators / annotations (@override, @dataclass, ...)
        if spec.at_attr
            && bytes[i] == b'@'
            && i + 1 < len
            && (bytes[i + 1].is_ascii_alphabetic() || bytes[i + 1] == b'_')
        {
            if i > plain_start {
                out.push_str(&html_escape(&raw_code[plain_start..i]));
            }
            let start = i;
            i += 1;
            while i < len
                && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_' || bytes[i] == b'.')
            {
                i += 1;
            }
            push_hl_span(&mut out, "hl-attr", &raw_code[start..i]);
            plain_start = i;
            continue;
        }

        // Shell / PHP $variables
        if spec.dollar_vars && bytes[i] == b'$' && i + 1 < len {
            let next = bytes[i + 1];
            if next == b'{' {
                if i > plain_start {
                    out.push_str(&html_escape(&raw_code[plain_start..i]));
                }
                let start = i;
                i += 2;
                while i < len && bytes[i] != b'}' && bytes[i] != b'\n' {
                    i += 1;
                }
                if i < len && bytes[i] == b'}' {
                    i += 1;
                }
                push_hl_span(&mut out, "hl-attr", &raw_code[start..i]);
                plain_start = i;
                continue;
            } else if next.is_ascii_alphanumeric() || next == b'_' {
                if i > plain_start {
                    out.push_str(&html_escape(&raw_code[plain_start..i]));
                }
                let start = i;
                i += 1;
                while i < len && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
                push_hl_span(&mut out, "hl-attr", &raw_code[start..i]);
                plain_start = i;
                continue;
            }
        }

        // Numbers
        if bytes[i].is_ascii_digit() {
            if i > plain_start {
                out.push_str(&html_escape(&raw_code[plain_start..i]));
            }
            let end = scan_number_end(bytes, i);
            push_hl_span(&mut out, "hl-num", &raw_code[i..end]);
            i = end;
            plain_start = i;
            continue;
        }

        // Identifiers, keywords, and types
        if bytes[i].is_ascii_alphabetic() || bytes[i] == b'_' {
            let start = i;
            i += 1;
            while i < len && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            let ident = &raw_code[start..i];
            let cls_opt = if match_word(ident, spec.keywords, spec.case_insensitive_kw) {
                Some("hl-kw")
            } else if match_word(ident, spec.types, spec.case_insensitive_kw)
                || (spec.pascal_case_types && is_pascal_case(ident))
            {
                Some("hl-ty")
            } else {
                None
            };
            if let Some(cls) = cls_opt {
                if start > plain_start {
                    out.push_str(&html_escape(&raw_code[plain_start..start]));
                }
                push_hl_span(&mut out, cls, ident);
                plain_start = i;
            }
            continue;
        }

        i += 1;
    }

    if plain_start < len {
        out.push_str(&html_escape(&raw_code[plain_start..]));
    }
    out
}

fn highlight_json(raw_code: &str) -> String {
    let bytes = raw_code.as_bytes();
    let len = bytes.len();
    let mut out = String::with_capacity(len + len / 4);
    let mut plain_start = 0usize;
    let mut i = 0usize;

    while i < len {
        if bytes[i] >= 0x80 {
            i += 1;
            while i < len && bytes[i] >= 0x80 {
                i += 1;
            }
            continue;
        }

        if bytes[i] == b'"' {
            if i > plain_start {
                out.push_str(&html_escape(&raw_code[plain_start..i]));
            }
            let end = scan_quoted_string(bytes, i, b'"', false);
            // Bounded lookahead (max 64 bytes) for ':' so adversarial whitespace stays O(N)
            let mut look = end;
            let look_limit = (end + 64).min(len);
            while look < look_limit && bytes[look].is_ascii_whitespace() {
                look += 1;
            }
            let cls = if look < len && bytes[look] == b':' {
                "hl-attr"
            } else {
                "hl-str"
            };
            push_hl_span(&mut out, cls, &raw_code[i..end]);
            i = end;
            plain_start = i;
            continue;
        }

        if bytes[i].is_ascii_digit()
            || (bytes[i] == b'-' && i + 1 < len && bytes[i + 1].is_ascii_digit())
        {
            if i > plain_start {
                out.push_str(&html_escape(&raw_code[plain_start..i]));
            }
            let start = i;
            if bytes[i] == b'-' {
                i += 1;
            }
            let end = scan_number_end(bytes, i);
            push_hl_span(&mut out, "hl-num", &raw_code[start..end]);
            i = end;
            plain_start = i;
            continue;
        }

        if bytes[i].is_ascii_alphabetic() {
            let start = i;
            i += 1;
            while i < len && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                i += 1;
            }
            let word = &raw_code[start..i];
            if matches!(word, "true" | "false" | "null") {
                if start > plain_start {
                    out.push_str(&html_escape(&raw_code[plain_start..start]));
                }
                push_hl_span(&mut out, "hl-kw", word);
                plain_start = i;
            }
            continue;
        }

        i += 1;
    }

    if plain_start < len {
        out.push_str(&html_escape(&raw_code[plain_start..]));
    }
    out
}

fn highlight_html_xml(raw_code: &str) -> String {
    let bytes = raw_code.as_bytes();
    let len = bytes.len();
    let mut out = String::with_capacity(len + len / 4);
    let mut plain_start = 0usize;
    let mut i = 0usize;

    while i < len {
        if bytes[i] >= 0x80 {
            i += 1;
            while i < len && bytes[i] >= 0x80 {
                i += 1;
            }
            continue;
        }

        if bytes[i..].starts_with(b"<!--") {
            if i > plain_start {
                out.push_str(&html_escape(&raw_code[plain_start..i]));
            }
            let start = i;
            i += 4;
            while i + 2 < len && !bytes[i..].starts_with(b"-->") {
                i += 1;
            }
            i = if i + 2 < len { i + 3 } else { len };
            push_hl_span(&mut out, "hl-cmt", &raw_code[start..i]);
            plain_start = i;
            continue;
        }

        if bytes[i] == b'<'
            && i + 1 < len
            && (bytes[i + 1].is_ascii_alphabetic() || matches!(bytes[i + 1], b'/' | b'!' | b'?'))
        {
            if i > plain_start {
                out.push_str(&html_escape(&raw_code[plain_start..i]));
            }
            let prefix_end = if matches!(bytes[i + 1], b'/' | b'!' | b'?') {
                i + 2
            } else {
                i + 1
            };
            out.push_str(&html_escape(&raw_code[i..prefix_end]));
            i = prefix_end;

            // Tag name
            let tag_start = i;
            while i < len
                && (bytes[i].is_ascii_alphanumeric() || matches!(bytes[i], b'-' | b'_' | b':'))
            {
                i += 1;
            }
            if i > tag_start {
                push_hl_span(&mut out, "hl-kw", &raw_code[tag_start..i]);
            }

            // Inside tag attributes until '>'
            let mut tag_plain = i;
            while i < len {
                if bytes[i] >= 0x80 {
                    i += 1;
                    while i < len && bytes[i] >= 0x80 {
                        i += 1;
                    }
                    continue;
                }
                if bytes[i] == b'>' {
                    i += 1;
                    if i > tag_plain {
                        out.push_str(&html_escape(&raw_code[tag_plain..i]));
                    }
                    tag_plain = i;
                    break;
                }
                if bytes[i] == b'"' || bytes[i] == b'\'' {
                    if i > tag_plain {
                        out.push_str(&html_escape(&raw_code[tag_plain..i]));
                    }
                    let end = scan_quoted_string(bytes, i, bytes[i], false);
                    push_hl_span(&mut out, "hl-str", &raw_code[i..end]);
                    i = end;
                    tag_plain = i;
                    continue;
                }
                if bytes[i].is_ascii_alphabetic() || bytes[i] == b'_' {
                    let attr_start = i;
                    i += 1;
                    while i < len
                        && (bytes[i].is_ascii_alphanumeric()
                            || matches!(bytes[i], b'-' | b'_' | b':'))
                    {
                        i += 1;
                    }
                    if attr_start > tag_plain {
                        out.push_str(&html_escape(&raw_code[tag_plain..attr_start]));
                    }
                    push_hl_span(&mut out, "hl-attr", &raw_code[attr_start..i]);
                    tag_plain = i;
                    continue;
                }
                i += 1;
            }
            if i > tag_plain {
                out.push_str(&html_escape(&raw_code[tag_plain..i]));
            }
            plain_start = i;
            continue;
        }

        i += 1;
    }

    if plain_start < len {
        out.push_str(&html_escape(&raw_code[plain_start..]));
    }
    out
}

fn highlight_css(raw_code: &str) -> String {
    let bytes = raw_code.as_bytes();
    let len = bytes.len();
    let mut out = String::with_capacity(len + len / 4);
    let mut plain_start = 0usize;
    let mut i = 0usize;
    let mut brace_depth = 0usize;

    while i < len {
        if bytes[i] >= 0x80 {
            i += 1;
            while i < len && bytes[i] >= 0x80 {
                i += 1;
            }
            continue;
        }

        if bytes[i..].starts_with(b"/*") {
            if i > plain_start {
                out.push_str(&html_escape(&raw_code[plain_start..i]));
            }
            let start = i;
            i += 2;
            while i + 1 < len && !(bytes[i] == b'*' && bytes[i + 1] == b'/') {
                i += 1;
            }
            i = if i + 1 < len { i + 2 } else { len };
            push_hl_span(&mut out, "hl-cmt", &raw_code[start..i]);
            plain_start = i;
            continue;
        }

        if bytes[i] == b'"' || bytes[i] == b'\'' {
            if i > plain_start {
                out.push_str(&html_escape(&raw_code[plain_start..i]));
            }
            let end = scan_quoted_string(bytes, i, bytes[i], false);
            push_hl_span(&mut out, "hl-str", &raw_code[i..end]);
            i = end;
            plain_start = i;
            continue;
        }

        if bytes[i] == b'{' {
            brace_depth += 1;
            i += 1;
            continue;
        }
        if bytes[i] == b'}' {
            brace_depth = brace_depth.saturating_sub(1);
            i += 1;
            continue;
        }

        // @at-rules or !important
        if matches!(bytes[i], b'@' | b'!')
            && i + 1 < len
            && (bytes[i + 1].is_ascii_alphabetic() || bytes[i + 1] == b'-')
        {
            if i > plain_start {
                out.push_str(&html_escape(&raw_code[plain_start..i]));
            }
            let start = i;
            i += 1;
            while i < len && (bytes[i].is_ascii_alphanumeric() || matches!(bytes[i], b'-' | b'_')) {
                i += 1;
            }
            push_hl_span(&mut out, "hl-kw", &raw_code[start..i]);
            plain_start = i;
            continue;
        }

        if brace_depth > 0 {
            if bytes[i] == b'#' && i + 1 < len && bytes[i + 1].is_ascii_hexdigit() {
                if i > plain_start {
                    out.push_str(&html_escape(&raw_code[plain_start..i]));
                }
                let start = i;
                i += 1;
                while i < len && bytes[i].is_ascii_hexdigit() {
                    i += 1;
                }
                push_hl_span(&mut out, "hl-num", &raw_code[start..i]);
                plain_start = i;
                continue;
            }

            if bytes[i].is_ascii_digit() {
                if i > plain_start {
                    out.push_str(&html_escape(&raw_code[plain_start..i]));
                }
                let start = i;
                while i < len && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                    i += 1;
                }
                while i < len && (bytes[i].is_ascii_alphabetic() || bytes[i] == b'%') {
                    i += 1;
                }
                push_hl_span(&mut out, "hl-num", &raw_code[start..i]);
                plain_start = i;
                continue;
            }
        }

        if bytes[i].is_ascii_alphabetic() || bytes[i] == b'-' {
            let start = i;
            i += 1;
            while i < len && (bytes[i].is_ascii_alphanumeric() || matches!(bytes[i], b'-' | b'_')) {
                i += 1;
            }
            if brace_depth > 0 {
                // Bounded lookahead for ':' property separator
                let mut look = i;
                let look_limit = (i + 64).min(len);
                while look < look_limit && matches!(bytes[look], b' ' | b'\t') {
                    look += 1;
                }
                if look < len && bytes[look] == b':' {
                    if start > plain_start {
                        out.push_str(&html_escape(&raw_code[plain_start..start]));
                    }
                    push_hl_span(&mut out, "hl-attr", &raw_code[start..i]);
                    plain_start = i;
                }
            }
            continue;
        }

        i += 1;
    }

    if plain_start < len {
        out.push_str(&html_escape(&raw_code[plain_start..]));
    }
    out
}

/// Highlights a fenced code block server-side with zero external dependencies and
/// strict HTML escaping of every emitted slice for CSP & XSS safety.
pub fn highlight_code_block(lang: &str, raw_code: &str) -> String {
    let norm = sanitize_lang_class(lang).to_ascii_lowercase();
    match norm.as_str() {
        "rust" | "rs" => highlight_code_spec(raw_code, &RUST_SPEC),
        "python" | "py" => highlight_code_spec(raw_code, &PYTHON_SPEC),
        "javascript" | "js" | "typescript" | "ts" | "jsx" | "tsx" => {
            highlight_code_spec(raw_code, &JS_TS_SPEC)
        }
        "sql" => highlight_code_spec(raw_code, &SQL_SPEC),
        "json" => highlight_json(raw_code),
        "bash" | "sh" | "shell" | "zsh" => highlight_code_spec(raw_code, &BASH_SPEC),
        "php" => highlight_code_spec(raw_code, &PHP_SPEC),
        "go" | "golang" => highlight_code_spec(raw_code, &GO_SPEC),
        "c" | "cpp" | "c++" | "java" => highlight_code_spec(raw_code, &C_CPP_JAVA_SPEC),
        "html" | "xml" | "xhtml" | "svg" => highlight_html_xml(raw_code),
        "css" => highlight_css(raw_code),
        _ => html_escape(raw_code),
    }
}

fn render_code_block(fence_lang: &str, fence_code: &str) -> String {
    let safe_lang = sanitize_lang_class(fence_lang);
    let highlighted = highlight_code_block(fence_lang, fence_code);
    if safe_lang.is_empty() {
        format!(
            r#"<pre class="code-block"><code>{}</code></pre>"#,
            highlighted
        )
    } else {
        format!(
            r#"<pre class="code-block"><code class="language-{}">{}</code></pre>"#,
            safe_lang, highlighted
        )
    }
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
                let html = render_code_block(&fence_lang, &fence_code);
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
        let html = render_code_block(&fence_lang, &fence_code);
        let idx = placeholders.len();
        placeholders.push(html);
        line_items.push(LineItem::FencePlaceholder(idx));
    }

    // 2. Parse Markdown block structures outside of fenced blocks
    enum Block {
        Fence(usize),
        Heading { level: usize, text: String },
        Hr,
        Blockquote(Vec<(usize, String)>),
        Ul(Vec<String>),
        Ol { start: u64, items: Vec<String> },
        Paragraph(Vec<String>),
    }

    let mut blocks: Vec<Block> = Vec::new();
    let mut current_para: Vec<String> = Vec::new();
    let mut current_quote: Vec<(usize, String)> = Vec::new();
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

                // Check Blockquote: lines starting with `>` (supports nested `> >`)
                if let Some((depth, rest)) = parse_blockquote(&line) {
                    flush_para!();
                    flush_ul!();
                    flush_ol!();
                    current_quote.push((depth, rest));
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
                out.push_str(&render_blockquote_lines(&lines, ctx, &mut placeholders));
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

fn render_blockquote_lines(
    lines: &[(usize, String)],
    ctx: &FormatContext,
    placeholders: &mut Vec<String>,
) -> String {
    let mut out = String::new();
    let mut current_depth = 0usize;
    let mut prev_depth = 0usize;

    for (depth, text) in lines {
        let target_depth = (*depth).min(MAX_BLOCKQUOTE_DEPTH);
        let opened_new = target_depth > current_depth;
        let closed_inner = target_depth < current_depth;
        while current_depth < target_depth {
            out.push_str("<blockquote>");
            current_depth += 1;
        }
        while current_depth > target_depth {
            out.push_str("</blockquote>");
            current_depth -= 1;
        }
        if !opened_new && !closed_inner && prev_depth == current_depth {
            out.push_str("<br>");
        }
        out.push_str(&format_inline(text, ctx, placeholders));
        prev_depth = current_depth;
    }

    while current_depth > 0 {
        out.push_str("</blockquote>");
        current_depth -= 1;
    }
    out
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

fn parse_blockquote(line: &str) -> Option<(usize, String)> {
    let mut rest = line.trim_start();
    let mut depth = 0usize;
    while let Some(after) = rest.strip_prefix('>') {
        depth += 1;
        let after_space = after.strip_prefix(' ').unwrap_or(after);
        let peek = after_space.trim_start();
        if peek.starts_with('>') {
            rest = peek;
        } else {
            rest = after_space;
            break;
        }
    }
    if depth > 0 {
        Some((depth, rest.to_string()))
    } else {
        None
    }
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

                // Fast-reject non-http(s) targets before scanning parens to preserve O(N) time
                let mut scheme_start = url_start;
                let ws_limit = (url_start + 16).min(blen);
                while scheme_start < ws_limit && b[scheme_start].is_ascii_whitespace() {
                    scheme_start += 1;
                }
                let maybe_scheme = escaped_line.get(scheme_start..);
                let starts_http = maybe_scheme.is_some_and(|s| {
                    s.get(..7)
                        .is_some_and(|p| p.eq_ignore_ascii_case("http://"))
                        || s.get(..8)
                            .is_some_and(|p| p.eq_ignore_ascii_case("https://"))
                });

                if starts_http {
                    // Track balanced parentheses inside the URL up to 2048 bytes
                    let scan_limit = (url_start + 2048).min(blen);
                    let mut depth = 1usize;
                    let mut p = url_start;
                    let mut close_paren_opt = None;
                    while p < scan_limit {
                        if b[p] == b'(' {
                            depth += 1;
                        } else if b[p] == b')' {
                            depth -= 1;
                            if depth == 0 {
                                close_paren_opt = Some(p);
                                break;
                            }
                        }
                        p += 1;
                    }

                    if let Some(close_paren) = close_paren_opt {
                        let label = &escaped_line[cursor + 1..close_bracket];
                        let url = escaped_line[url_start..close_paren].trim();

                        if !url.contains(char::is_whitespace)
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
                        }
                    }
                }
                // Target for this `](` is invalid or unclosed; advance `mid_ptr` so preceding `[` do not re-inspect it
                mid_ptr += 1;
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

                // Balance parentheses and trailing punctuation/markdown delimiters outside the URL in a strictly linear O(N) scan
                let mut candidate_url: String = chars[url_start..url_end].iter().collect();
                let open_p = candidate_url.chars().filter(|&c| c == '(').count();
                let mut close_p = candidate_url.chars().filter(|&c| c == ')').count();

                let mut cut = candidate_url.len();
                loop {
                    let s = &candidate_url[..cut];
                    if s.ends_with(|c| ".,!?:;]*~".contains(c)) && !s.ends_with("&amp;") {
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
                                        r#"<a href="/view_bug_page/{}" class="issue-link-resolved" title="{}">#{:07}</a>"#,
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

        if let Some((uname, actual_end)) = crate::models::parse_mention_at(&chars, i) {
            if ctx.valid_usernames.contains(&uname)
                || ctx.valid_usernames.contains(&uname.to_ascii_lowercase())
            {
                let span_html = format!(r#"<span class="mention">@{}</span>"#, html_escape(&uname));
                let idx = placeholders.len();
                placeholders.push(span_html);
                out.push_str(&format!("\x01P{}X\x02", idx));
                i = actual_end;
                continue;
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
