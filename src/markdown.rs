use pulldown_cmark::{html, Options, Parser};

pub fn convert_markdown_to_html(markdown: &str) -> String {
    let normalized = markdown.replace("\r\n", "\n");
    let preprocessed = preprocess_custom_blocks(&normalized);
    let body = render_markdown_fragment(&preprocessed);

    format!(
        "<!DOCTYPE html><html><head><meta charset=\"utf-8\"></head><body class=\"markdown-body\">{body}</body></html>"
    )
}

pub fn markdown_default_css() -> &'static str {
    MARKDOWN_DEFAULT_CSS
}

fn render_markdown_fragment(input: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);
    options.insert(Options::ENABLE_FOOTNOTES);

    let parser = Parser::new_ext(input, options);
    let mut output = String::new();
    html::push_html(&mut output, parser);
    output
}

fn preprocess_custom_blocks(markdown: &str) -> String {
    let lines = markdown.lines().collect::<Vec<_>>();
    let mut output = String::new();
    let mut index = 0usize;

    while index < lines.len() {
        let trimmed = lines[index].trim();

        if let Some((kind, title)) = parse_container_open(trimmed) {
            let mut end = index + 1;
            while end < lines.len() && lines[end].trim() != ":::" {
                end += 1;
            }

            if end < lines.len() {
                let body_md = lines[index + 1..end].join("\n");
                output.push_str(&render_container_block(&kind, &title, &body_md));
                output.push('\n');
                index = end + 1;
                continue;
            }
        }

        if let Some((kind, inline_title)) = parse_github_alert_open(trimmed) {
            let mut body_lines = Vec::new();
            if !inline_title.is_empty() {
                body_lines.push(inline_title);
            }

            let mut end = index + 1;
            while end < lines.len() {
                let line = lines[end].trim_start();
                if !line.starts_with('>') {
                    break;
                }
                body_lines.push(line.trim_start_matches('>').trim_start().to_string());
                end += 1;
            }

            output.push_str(&render_container_block(&kind, "", &body_lines.join("\n")));
            output.push('\n');
            index = end;
            continue;
        }

        output.push_str(lines[index]);
        output.push('\n');
        index += 1;
    }

    output
}

fn parse_container_open(line: &str) -> Option<(String, String)> {
    let payload = line.strip_prefix(":::")?.trim();
    if payload.is_empty() {
        return None;
    }

    let mut parts = payload.splitn(2, char::is_whitespace);
    let kind = parts.next()?.trim().to_ascii_lowercase();
    if kind.is_empty() {
        return None;
    }
    let title = parts.next().unwrap_or("").trim().to_string();
    Some((kind, title))
}

fn parse_github_alert_open(line: &str) -> Option<(String, String)> {
    let payload = line.strip_prefix("> [!")?;
    let close = payload.find(']')?;
    let kind = payload[..close].trim().to_ascii_lowercase();
    if kind.is_empty() {
        return None;
    }
    let title = payload[close + 1..].trim().to_string();
    Some((kind, title))
}

fn render_container_block(kind: &str, title: &str, body_md: &str) -> String {
    let class_kind = sanitize_class_fragment(kind);
    let title_text = if title.trim().is_empty() {
        default_container_title(kind)
    } else {
        title.trim().to_string()
    };
    let body_html = render_markdown_fragment(body_md.trim());

    format!(
        "<div class=\"md-container md-{class_kind}\"><p class=\"md-container-title\">{}</p>{body_html}</div>",
        escape_html(&title_text)
    )
}

fn sanitize_class_fragment(value: &str) -> String {
    let mut sanitized = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>();
    while sanitized.contains("--") {
        sanitized = sanitized.replace("--", "-");
    }
    sanitized.trim_matches('-').to_string()
}

fn default_container_title(kind: &str) -> String {
    match kind.to_ascii_lowercase().as_str() {
        "tip" => "Tip".to_string(),
        "warning" => "Warning".to_string(),
        "danger" => "Danger".to_string(),
        "note" => "Note".to_string(),
        "important" => "Important".to_string(),
        "caution" => "Caution".to_string(),
        "card" => "Card".to_string(),
        other => {
            let mut chars = other.chars();
            let Some(first) = chars.next() else {
                return "Info".to_string();
            };
            format!(
                "{}{}",
                first.to_ascii_uppercase(),
                chars.collect::<String>()
            )
        }
    }
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

const MARKDOWN_DEFAULT_CSS: &str = r#"
@page {
  margin: 2cm;
}

body {
  font-family: Sarabun, sans-serif;
  font-size: 14px;
  line-height: 1.6;
  color: #24292f;
}

h1 {
  font-size: 28px;
  margin: 20px 0 14px;
  border-bottom: 1px solid #d0d7de;
  padding-bottom: 4px;
}

h2 {
  font-size: 22px;
  margin: 18px 0 12px;
  border-bottom: 1px solid #d0d7de;
  padding-bottom: 3px;
}

h3 {
  font-size: 18px;
  margin: 16px 0 10px;
}

h4 {
  font-size: 15px;
  margin: 14px 0 8px;
}

p {
  margin: 0 0 10px;
}

a {
  color: #0969da;
  text-decoration: underline;
}

blockquote {
  margin: 0 0 10px;
  padding-left: 12px;
  border-left: 3px solid #d0d7de;
  color: #59636e;
}

pre {
  margin: 0 0 12px;
  padding: 10px 12px;
  border: 1px solid #d0d7de;
  background: #f6f8fa;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
  word-break: break-word;
}

code {
  font-family: monospace;
  font-size: 12px;
}

pre code {
  display: block;
  white-space: pre-wrap;
  overflow-wrap: anywhere;
  word-break: break-word;
}

ul,
ol {
  margin: 0 0 10px;
  padding-left: 24px;
}

li {
  margin-bottom: 4px;
}

table {
  border-collapse: collapse;
  margin: 0 0 10px;
}

th,
td {
  border: 1px solid #d0d7de;
  padding: 6px 10px;
  font-size: 12px;
}

th {
  background: #f6f8fa;
}

hr {
  border: none;
  height: 0;
  margin: 0;
  page-break-after: always;
}

.md-container {
  border-left: 4px solid #d0d7de;
  border-radius: 6px;
  background: #f6f8fa;
  padding: 10px 12px;
  margin: 0 0 10px;
}

.md-container-title {
  font-weight: bold;
  margin: 0 0 6px;
}

.md-info,
.md-note {
  border-left-color: #0969da;
  background: #ddf4ff;
}

.md-tip,
.md-success,
.md-box-green {
  border-left-color: #1a7f37;
  background: #dafbe1;
}

.md-warning,
.md-caution,
.md-box-yellow,
.md-box-orange {
  border-left-color: #9a6700;
  background: #fff8c5;
}

.md-danger,
.md-box-red,
.md-error {
  border-left-color: #cf222e;
  background: #ffebe9;
}

.md-important,
.md-box-purple,
.md-special {
  border-left-color: #8250df;
  background: #fbefff;
}

.md-card,
.md-box-gray,
.md-box-blue {
  border-left-color: #57606a;
  background: #f3f4f6;
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_markdown_links_and_tables() {
        let html = convert_markdown_to_html(
            "[link](https://example.com)\n\n| a | b |\n| - | - |\n| 1 | 2 |",
        );
        assert!(html.contains("href=\"https://example.com\""));
        assert!(html.contains("<table>"));
    }

    #[test]
    fn converts_custom_containers() {
        let html = convert_markdown_to_html(":::warning ระวัง\nข้อความ\n:::");
        assert!(html.contains("md-container md-warning"));
        assert!(html.contains("ระวัง"));
    }
}
