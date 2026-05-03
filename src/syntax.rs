use crate::types::RenderOptions;
use std::fmt::Write as _;
use std::sync::OnceLock;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style as SyntectStyle, Theme, ThemeSet};
use syntect::parsing::{SyntaxReference, SyntaxSet};
use syntect::util::LinesWithEndings;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LanguageFamily {
    PlainText,
    Rust,
    JsTs,
    Python,
    Go,
    CLike,
    Sql,
    Json,
    Yaml,
    Html,
    Css,
    Bash,
    Markdown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DetectedLanguage {
    token: String,
    family: LanguageFamily,
}

#[derive(Debug, Clone, Copy)]
struct SyntaxPalette {
    keyword: &'static str,
    string: &'static str,
    number: &'static str,
    comment: &'static str,
    builtin: &'static str,
}

static SYNTECT_SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();
static SYNTECT_THEME_SET: OnceLock<ThemeSet> = OnceLock::new();

pub fn apply_syntax_highlighting_to_html(html: &str, options: &RenderOptions) -> String {
    if !options.enable_syntax_highlighting {
        return html.to_string();
    }

    let palette = palette_for_theme(&options.syntax_highlight_theme);
    let mut output = String::with_capacity(html.len() + html.len() / 4);
    let mut cursor = 0usize;

    while let Some(pre_rel) = html[cursor..].find("<pre") {
        let pre_start = cursor + pre_rel;
        output.push_str(&html[cursor..pre_start]);

        let Some(pre_open_rel_end) = html[pre_start..].find('>') else {
            output.push_str(&html[pre_start..]);
            return output;
        };
        let pre_open_end = pre_start + pre_open_rel_end;
        let pre_attrs = html[pre_start + "<pre".len()..pre_open_end].trim();
        let pre_body_start = pre_open_end + 1;

        let Some(pre_close_rel) = html[pre_body_start..].find("</pre>") else {
            output.push_str(&html[pre_start..]);
            return output;
        };
        let pre_close_start = pre_body_start + pre_close_rel;
        let pre_end = pre_close_start + "</pre>".len();
        let pre_inner = &html[pre_body_start..pre_close_start];

        let Some(code_open_rel) = pre_inner.find("<code") else {
            output.push_str(&html[pre_start..pre_end]);
            cursor = pre_end;
            continue;
        };

        let code_open_start = pre_body_start + code_open_rel;
        let Some(code_open_rel_end) = html[code_open_start..].find('>') else {
            output.push_str(&html[pre_start..pre_end]);
            cursor = pre_end;
            continue;
        };

        let code_open_end = code_open_start + code_open_rel_end;
        let code_attrs = html[code_open_start + "<code".len()..code_open_end].trim();
        let code_body_start = code_open_end + 1;

        let Some(code_close_rel) = html[code_body_start..pre_close_start].find("</code>") else {
            output.push_str(&html[pre_start..pre_end]);
            cursor = pre_end;
            continue;
        };

        let code_close_start = code_body_start + code_close_rel;
        let code_close_end = code_close_start + "</code>".len();

        let leading = &html[pre_body_start..code_open_start];
        let trailing = &html[code_close_end..pre_close_start];

        if !leading.trim().is_empty() || !trailing.trim().is_empty() {
            output.push_str(&html[pre_start..pre_end]);
            cursor = pre_end;
            continue;
        }

        if code_attrs.contains("data-lynpdf-highlighted") {
            output.push_str(&html[pre_start..pre_end]);
            cursor = pre_end;
            continue;
        }

        let code_body_raw = &html[code_body_start..code_close_start];
        if code_body_raw.contains("<span") || code_body_raw.contains("<div") {
            output.push_str(&html[pre_start..pre_end]);
            cursor = pre_end;
            continue;
        }

        let language = detect_language_from_attrs(code_attrs);
        let decoded = decode_html_entities_basic(code_body_raw);
        let highlighted = highlight_source_code(
            &decoded,
            language.as_ref(),
            palette,
            &options.syntax_highlight_theme,
        );
        let pre_style = "margin:0 0 12px;padding:10px 12px;border:1px solid #d0d7de;background:#f6f8fa;white-space:pre-wrap;overflow-wrap:anywhere;word-break:break-word;";
        let code_style = "display:block;white-space:pre-wrap;overflow-wrap:anywhere;word-break:break-word;font-family:monospace;font-size:12px;line-height:1.5;";

        let pre_open = if pre_attrs.is_empty() {
            format!("<pre style=\"{pre_style}\">")
        } else if pre_attrs.to_ascii_lowercase().contains("style=") {
            format!("<pre {}>", pre_attrs.trim())
        } else {
            format!("<pre {} style=\"{pre_style}\">", pre_attrs.trim())
        };

        let code_open = if code_attrs.is_empty() {
            format!("<code data-lynpdf-highlighted=\"1\" style=\"{code_style}\">")
        } else if code_attrs.to_ascii_lowercase().contains("style=") {
            format!("<code {} data-lynpdf-highlighted=\"1\">", code_attrs.trim())
        } else {
            format!(
                "<code {} data-lynpdf-highlighted=\"1\" style=\"{code_style}\">",
                code_attrs.trim()
            )
        };

        output.push_str(&pre_open);
        output.push_str(&code_open);
        output.push_str(&highlighted);
        output.push_str("</code></pre>");

        cursor = pre_end;
    }

    output.push_str(&html[cursor..]);
    output
}

fn highlight_source_code(
    source: &str,
    language: Option<&DetectedLanguage>,
    palette: SyntaxPalette,
    theme_name: &str,
) -> String {
    if let Some(rendered) = language
        .and_then(|detected| highlight_source_code_with_syntect(source, detected, theme_name))
    {
        return rendered;
    }

    let lang = language
        .map(|detected| detected.family)
        .unwrap_or(LanguageFamily::PlainText);

    let mut out = String::with_capacity(source.len() * 2);
    for line in source.split_inclusive('\n') {
        out.push_str(&highlight_line(line, lang, palette));
    }

    out
}

fn highlight_source_code_with_syntect(
    source: &str,
    language: &DetectedLanguage,
    theme_name: &str,
) -> Option<String> {
    let syntax_set = syntect_syntax_set();
    let syntax = syntect_syntax_for_token(&language.token)?;
    let theme = syntect_theme(theme_name)?;
    let mut highlighter = HighlightLines::new(syntax, theme);
    let mut out = String::with_capacity(source.len() * 3);

    for line in LinesWithEndings::from(source) {
        let ranges = highlighter.highlight_line(line, syntax_set).ok()?;
        for (style, content) in ranges {
            push_syntect_span(&mut out, style, content);
        }
    }

    Some(out)
}

fn syntect_syntax_set() -> &'static SyntaxSet {
    SYNTECT_SYNTAX_SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

fn syntect_theme_set() -> &'static ThemeSet {
    SYNTECT_THEME_SET.get_or_init(ThemeSet::load_defaults)
}

fn syntect_syntax_for_token(token: &str) -> Option<&'static SyntaxReference> {
    let syntax_set = syntect_syntax_set();
    for candidate in syntect_token_candidates(token) {
        if let Some(syntax) = syntax_set.find_syntax_by_token(&candidate) {
            return Some(syntax);
        }
        if let Some(syntax) = syntax_set.find_syntax_by_extension(&candidate) {
            return Some(syntax);
        }
        if let Some(syntax) = syntax_set.find_syntax_by_name(&candidate) {
            return Some(syntax);
        }
    }
    None
}

fn syntect_token_candidates(token: &str) -> Vec<String> {
    let normalized = token.trim().trim_start_matches('.').to_ascii_lowercase();
    match normalized.as_str() {
        "rs" | "rust" => vec!["Rust".to_string(), "rs".to_string()],
        "ts" | "tsx" | "typescript" => vec!["TypeScript".to_string(), "ts".to_string()],
        "js" | "jsx" | "javascript" => vec!["JavaScript".to_string(), "js".to_string()],
        "py" | "python" => vec!["Python".to_string(), "py".to_string()],
        "go" | "golang" => vec!["Go".to_string(), "go".to_string()],
        "c" | "h" => vec!["C".to_string(), normalized],
        "cpp" | "cxx" | "hpp" => vec!["C++".to_string(), normalized],
        "java" => vec!["Java".to_string(), "java".to_string()],
        "kt" | "kotlin" => vec!["Kotlin".to_string(), "kt".to_string()],
        "sql" => vec!["SQL".to_string(), "sql".to_string()],
        "json" => vec!["JSON".to_string(), "json".to_string()],
        "yaml" | "yml" => vec!["YAML".to_string(), normalized],
        "html" | "xml" => vec!["HTML".to_string(), normalized],
        "svg" => vec!["XML".to_string(), "svg".to_string(), "xml".to_string()],
        "css" | "scss" => vec!["CSS".to_string(), normalized],
        "bash" | "sh" | "zsh" | "shell" => {
            vec!["Bourne Again Shell (bash)".to_string(), "sh".to_string()]
        }
        "md" | "markdown" => vec!["Markdown".to_string(), "md".to_string()],
        _ => vec![normalized],
    }
}

fn syntect_theme(theme_name: &str) -> Option<&'static Theme> {
    let theme_set = syntect_theme_set();
    let lower = theme_name.to_ascii_lowercase();
    let candidates = if lower.contains("dark") {
        ["base16-ocean.dark", "Solarized (dark)", "InspiredGitHub"]
    } else {
        ["InspiredGitHub", "base16-ocean.light", "Solarized (light)"]
    };

    candidates
        .iter()
        .find_map(|name| theme_set.themes.get(*name))
        .or_else(|| theme_set.themes.values().next())
}

fn push_syntect_span(out: &mut String, style: SyntectStyle, content: &str) {
    if content.is_empty() {
        return;
    }
    out.push_str("<span style=\"color:");
    let _ = write!(
        out,
        "#{:02x}{:02x}{:02x}",
        style.foreground.r, style.foreground.g, style.foreground.b
    );
    out.push_str(";\">");
    out.push_str(&html_escape(content));
    out.push_str("</span>");
}

fn highlight_line(line: &str, lang: LanguageFamily, palette: SyntaxPalette) -> String {
    if matches!(lang, LanguageFamily::PlainText) {
        return html_escape(line);
    }

    let mut out = String::with_capacity(line.len() * 2);
    let mut idx = 0usize;

    while idx < line.len() {
        let rest = &line[idx..];

        if comment_marker(lang, rest).is_some() {
            let comment = &line[idx..];
            push_span(&mut out, palette.comment, &html_escape(comment));
            break;
        }

        let Some(ch) = rest.chars().next() else {
            break;
        };

        if is_string_delimiter(lang, ch) {
            let end = consume_string(line, idx, ch);
            push_span(&mut out, palette.string, &html_escape(&line[idx..end]));
            idx = end;
            continue;
        }

        if ch.is_ascii_digit() {
            let end = consume_number(line, idx);
            push_span(&mut out, palette.number, &html_escape(&line[idx..end]));
            idx = end;
            continue;
        }

        if is_ident_start(ch) {
            let end = consume_identifier(line, idx);
            let word = &line[idx..end];
            let word_lower = word.to_ascii_lowercase();
            if is_keyword(lang, &word_lower) {
                push_span(&mut out, palette.keyword, &html_escape(word));
            } else if is_builtin(lang, &word_lower) {
                push_span(&mut out, palette.builtin, &html_escape(word));
            } else {
                out.push_str(&html_escape(word));
            }
            idx = end;
            continue;
        }

        out.push_str(&html_escape_char(ch));
        idx += ch.len_utf8();
    }

    out
}

fn comment_marker(lang: LanguageFamily, rest: &str) -> Option<&'static str> {
    match lang {
        LanguageFamily::Python
        | LanguageFamily::Yaml
        | LanguageFamily::Bash
        | LanguageFamily::Markdown => rest.starts_with('#').then_some("#"),
        LanguageFamily::Sql => rest.starts_with("--").then_some("--"),
        LanguageFamily::Html => rest.starts_with("<!--").then_some("<!--"),
        LanguageFamily::Json => None,
        _ => rest.starts_with("//").then_some("//"),
    }
}

fn is_string_delimiter(lang: LanguageFamily, ch: char) -> bool {
    match lang {
        LanguageFamily::Json | LanguageFamily::Yaml => matches!(ch, '"' | '\''),
        _ => matches!(ch, '"' | '\'' | '`'),
    }
}

fn consume_string(line: &str, start: usize, quote: char) -> usize {
    let mut idx = start + quote.len_utf8();
    let mut escaped = false;

    while idx < line.len() {
        let Some(ch) = line[idx..].chars().next() else {
            break;
        };

        if escaped {
            escaped = false;
            idx += ch.len_utf8();
            continue;
        }

        if ch == '\\' {
            escaped = true;
            idx += ch.len_utf8();
            continue;
        }

        idx += ch.len_utf8();
        if ch == quote {
            break;
        }
    }

    idx
}

fn consume_number(line: &str, start: usize) -> usize {
    let mut idx = start;
    while idx < line.len() {
        let Some(ch) = line[idx..].chars().next() else {
            break;
        };
        if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | 'x' | 'X') {
            idx += ch.len_utf8();
        } else {
            break;
        }
    }
    idx
}

fn is_ident_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_'
}

fn consume_identifier(line: &str, start: usize) -> usize {
    let mut idx = start;
    while idx < line.len() {
        let Some(ch) = line[idx..].chars().next() else {
            break;
        };
        if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-') {
            idx += ch.len_utf8();
        } else {
            break;
        }
    }
    idx
}

fn detect_language_from_attrs(attrs: &str) -> Option<DetectedLanguage> {
    let class_attr = extract_attr_value(attrs, "class");

    if let Some(class_attr) = class_attr {
        for token in class_attr.split_whitespace() {
            let lower = token.to_ascii_lowercase();
            if let Some(language) = lower
                .strip_prefix("language-")
                .or_else(|| lower.strip_prefix("lang-"))
                .and_then(detect_language_token)
            {
                return Some(language);
            }
            if let Some(language) = detect_language_token(&lower) {
                return Some(language);
            }
        }
    }

    extract_attr_value(attrs, "data-language")
        .as_deref()
        .and_then(detect_language_token)
}

fn detect_language_token(token: &str) -> Option<DetectedLanguage> {
    let normalized = token.trim().trim_start_matches('.').to_ascii_lowercase();
    if normalized.is_empty() {
        return None;
    }

    let family = language_from_token(&normalized);
    if family.is_none() && syntect_syntax_for_token(&normalized).is_none() {
        return None;
    }

    Some(DetectedLanguage {
        token: normalized,
        family: family.unwrap_or(LanguageFamily::PlainText),
    })
}

fn extract_attr_value(attrs: &str, attr_name: &str) -> Option<String> {
    let lower = attrs.to_ascii_lowercase();
    let needle = format!("{attr_name}=");
    let pos = lower.find(&needle)?;
    let raw = attrs[pos + needle.len()..].trim_start();

    if let Some(rest) = raw.strip_prefix('"') {
        let end = rest.find('"')?;
        return Some(rest[..end].to_string());
    }

    if let Some(rest) = raw.strip_prefix('\'') {
        let end = rest.find('\'')?;
        return Some(rest[..end].to_string());
    }

    let end = raw.find(char::is_whitespace).unwrap_or(raw.len());
    Some(raw[..end].to_string())
}

fn language_from_token(token: &str) -> Option<LanguageFamily> {
    match token.to_ascii_lowercase().as_str() {
        "rs" | "rust" => Some(LanguageFamily::Rust),
        "ts" | "tsx" | "js" | "jsx" | "typescript" | "javascript" => Some(LanguageFamily::JsTs),
        "py" | "python" => Some(LanguageFamily::Python),
        "go" | "golang" => Some(LanguageFamily::Go),
        "c" | "h" | "hpp" | "cpp" | "cxx" | "java" | "kt" | "kotlin" => Some(LanguageFamily::CLike),
        "sql" => Some(LanguageFamily::Sql),
        "json" => Some(LanguageFamily::Json),
        "yaml" | "yml" => Some(LanguageFamily::Yaml),
        "html" | "xml" | "svg" => Some(LanguageFamily::Html),
        "css" | "scss" => Some(LanguageFamily::Css),
        "bash" | "sh" | "zsh" | "shell" => Some(LanguageFamily::Bash),
        "md" | "markdown" => Some(LanguageFamily::Markdown),
        _ => None,
    }
}

fn is_keyword(lang: LanguageFamily, word: &str) -> bool {
    match lang {
        LanguageFamily::Rust => matches!(
            word,
            "as" | "async"
                | "await"
                | "break"
                | "const"
                | "continue"
                | "crate"
                | "else"
                | "enum"
                | "extern"
                | "fn"
                | "for"
                | "if"
                | "impl"
                | "in"
                | "let"
                | "loop"
                | "match"
                | "mod"
                | "move"
                | "mut"
                | "pub"
                | "return"
                | "self"
                | "static"
                | "struct"
                | "super"
                | "trait"
                | "type"
                | "unsafe"
                | "use"
                | "where"
                | "while"
        ),
        LanguageFamily::JsTs => matches!(
            word,
            "as" | "async"
                | "await"
                | "break"
                | "case"
                | "catch"
                | "class"
                | "const"
                | "continue"
                | "default"
                | "else"
                | "export"
                | "extends"
                | "finally"
                | "for"
                | "from"
                | "function"
                | "if"
                | "import"
                | "implements"
                | "interface"
                | "let"
                | "new"
                | "return"
                | "switch"
                | "throw"
                | "try"
                | "type"
                | "var"
                | "while"
        ),
        LanguageFamily::Python => matches!(
            word,
            "and"
                | "as"
                | "break"
                | "class"
                | "continue"
                | "def"
                | "elif"
                | "else"
                | "except"
                | "finally"
                | "for"
                | "from"
                | "if"
                | "import"
                | "in"
                | "is"
                | "lambda"
                | "not"
                | "or"
                | "pass"
                | "raise"
                | "return"
                | "try"
                | "while"
                | "with"
                | "yield"
        ),
        LanguageFamily::Go => matches!(
            word,
            "break"
                | "case"
                | "chan"
                | "const"
                | "continue"
                | "default"
                | "defer"
                | "else"
                | "fallthrough"
                | "for"
                | "func"
                | "go"
                | "if"
                | "import"
                | "interface"
                | "map"
                | "package"
                | "range"
                | "return"
                | "select"
                | "struct"
                | "switch"
                | "type"
                | "var"
        ),
        LanguageFamily::CLike => matches!(
            word,
            "break"
                | "case"
                | "catch"
                | "class"
                | "const"
                | "continue"
                | "default"
                | "do"
                | "else"
                | "enum"
                | "extends"
                | "for"
                | "if"
                | "implements"
                | "import"
                | "interface"
                | "namespace"
                | "new"
                | "private"
                | "protected"
                | "public"
                | "return"
                | "static"
                | "struct"
                | "switch"
                | "template"
                | "throw"
                | "try"
                | "typedef"
                | "using"
                | "void"
                | "while"
        ),
        LanguageFamily::Sql => matches!(
            word,
            "select"
                | "from"
                | "where"
                | "join"
                | "left"
                | "right"
                | "inner"
                | "outer"
                | "on"
                | "group"
                | "by"
                | "order"
                | "insert"
                | "into"
                | "values"
                | "update"
                | "set"
                | "delete"
                | "create"
                | "table"
                | "alter"
                | "drop"
                | "and"
                | "or"
                | "not"
                | "as"
                | "distinct"
                | "limit"
        ),
        LanguageFamily::Json => matches!(word, "true" | "false" | "null"),
        LanguageFamily::Yaml => matches!(word, "true" | "false" | "null"),
        LanguageFamily::Html => matches!(
            word,
            "html"
                | "head"
                | "body"
                | "div"
                | "span"
                | "a"
                | "img"
                | "svg"
                | "path"
                | "rect"
                | "circle"
        ),
        LanguageFamily::Css => matches!(
            word,
            "display"
                | "color"
                | "background"
                | "font-size"
                | "font-family"
                | "margin"
                | "padding"
                | "border"
                | "width"
                | "height"
                | "position"
                | "absolute"
                | "relative"
                | "flex"
                | "grid"
                | "justify-content"
                | "align-items"
        ),
        LanguageFamily::Bash => matches!(
            word,
            "if" | "then"
                | "else"
                | "fi"
                | "for"
                | "while"
                | "do"
                | "done"
                | "case"
                | "esac"
                | "function"
                | "in"
        ),
        LanguageFamily::Markdown => matches!(word, "```" | "#" | "##" | "###"),
        LanguageFamily::PlainText => false,
    }
}

fn is_builtin(lang: LanguageFamily, word: &str) -> bool {
    match lang {
        LanguageFamily::Rust => matches!(
            word,
            "string"
                | "str"
                | "vec"
                | "option"
                | "result"
                | "some"
                | "none"
                | "ok"
                | "err"
                | "println"
                | "format"
        ),
        LanguageFamily::JsTs => matches!(
            word,
            "console"
                | "promise"
                | "string"
                | "number"
                | "boolean"
                | "array"
                | "object"
                | "undefined"
                | "null"
        ),
        LanguageFamily::Python => matches!(
            word,
            "self" | "none" | "true" | "false" | "print" | "dict" | "list" | "tuple"
        ),
        LanguageFamily::Go => matches!(word, "string" | "int" | "bool" | "error" | "nil"),
        LanguageFamily::CLike => matches!(
            word,
            "string"
                | "int"
                | "float"
                | "double"
                | "bool"
                | "boolean"
                | "char"
                | "nullptr"
                | "null"
                | "this"
        ),
        LanguageFamily::Sql => matches!(word, "count" | "sum" | "avg" | "min" | "max"),
        LanguageFamily::Json | LanguageFamily::Yaml => matches!(word, "true" | "false" | "null"),
        _ => false,
    }
}

fn push_span(out: &mut String, color: &str, content: &str) {
    if content.is_empty() {
        return;
    }
    out.push_str("<span style=\"");
    out.push_str("color:");
    out.push_str(color);
    out.push_str(";\">");
    out.push_str(content);
    out.push_str("</span>");
}

fn palette_for_theme(theme: &str) -> SyntaxPalette {
    if theme.to_ascii_lowercase().contains("dark") {
        SyntaxPalette {
            keyword: "#ff7b72",
            string: "#a5d6ff",
            number: "#79c0ff",
            comment: "#8b949e",
            builtin: "#d2a8ff",
        }
    } else {
        SyntaxPalette {
            keyword: "#cf222e",
            string: "#0a7f3f",
            number: "#0550ae",
            comment: "#6e7781",
            builtin: "#8250df",
        }
    }
}

fn decode_html_entities_basic(input: &str) -> String {
    input
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
}

fn html_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn html_escape_char(ch: char) -> String {
    match ch {
        '&' => "&amp;".to_string(),
        '<' => "&lt;".to_string(),
        '>' => "&gt;".to_string(),
        '"' => "&quot;".to_string(),
        '\'' => "&#39;".to_string(),
        _ => ch.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn highlights_html_code_block_when_enabled() {
        let options = RenderOptions::default();
        let html = "<pre><code class=\"language-rust\">let x = 42; // comment\n</code></pre>";
        let rendered = apply_syntax_highlighting_to_html(html, &options);

        assert!(rendered.contains("data-lynpdf-highlighted=\"1\""));
        assert!(rendered.contains("color:#"));
        assert!(strip_html_tags(&rendered).contains("let x = 42; // comment\n"));
    }

    #[test]
    fn keeps_html_untouched_when_disabled() {
        let options = RenderOptions::default().with_syntax_highlighting(false);
        let html = "<pre><code class=\"language-rust\">let x = 42;\n</code></pre>";
        let rendered = apply_syntax_highlighting_to_html(html, &options);
        assert_eq!(rendered, html);
    }

    #[test]
    fn highlighting_preserves_code_block_newlines_and_indentation() {
        let options = RenderOptions::default();
        let html = "<pre><code class=\"language-python\">def run():\n    return 1\n</code></pre>";
        let rendered = apply_syntax_highlighting_to_html(html, &options);

        assert!(rendered.contains("color:#"));
        assert!(strip_html_tags(&rendered).contains("def run():\n    return 1\n"));
    }

    #[test]
    fn injects_default_pre_and_code_styles_when_missing() {
        let options = RenderOptions::default();
        let html = "<pre><code class=\"language-rust\">let x = 1;\n</code></pre>";
        let rendered = apply_syntax_highlighting_to_html(html, &options);

        assert!(
            rendered.contains("<pre style=\"margin:0 0 12px;padding:10px 12px;border:1px solid #d0d7de;background:#f6f8fa;")
        );
        assert!(rendered.contains("<code class=\"language-rust\" data-lynpdf-highlighted=\"1\" style=\"display:block;white-space:pre-wrap;"));
    }

    #[test]
    fn preserves_existing_pre_style() {
        let options = RenderOptions::default();
        let html = "<pre style=\"padding:0\"><code class=\"language-rust\">let x = 1;\n</code></pre>";
        let rendered = apply_syntax_highlighting_to_html(html, &options);

        assert!(rendered.contains("<pre style=\"padding:0\">"));
        assert!(!rendered.contains("<pre style=\"margin:0 0 12px;"));
    }

    fn strip_html_tags(html: &str) -> String {
        let mut output = String::new();
        let mut inside_tag = false;
        for ch in html.chars() {
            match ch {
                '<' => inside_tag = true,
                '>' => inside_tag = false,
                _ if !inside_tag => output.push(ch),
                _ => {}
            }
        }
        output
    }
}
