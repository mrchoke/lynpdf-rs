use crate::{render_html_to_pdf, PageSize, PdfDocument, RenderOptions, RenderRequest, Result};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CertificateOrientation {
    Portrait,
    Landscape,
}

impl CertificateOrientation {
    fn page_size(self) -> PageSize {
        match self {
            CertificateOrientation::Portrait => PageSize::A4,
            CertificateOrientation::Landscape => PageSize {
                width_pt: PageSize::A4.height_pt,
                height_pt: PageSize::A4.width_pt,
            },
        }
    }

    fn css_page_size(self) -> &'static str {
        match self {
            CertificateOrientation::Portrait => "A4 portrait",
            CertificateOrientation::Landscape => "A4 landscape",
        }
    }
}

#[derive(Debug, Clone)]
pub struct CertificateTemplateOptions {
    pub orientation: CertificateOrientation,
    pub title: String,
    pub subtitle: String,
    pub recipient_name: String,
    pub statement: String,
    pub issuer_name: String,
    pub issuer_role: String,
    pub issue_date: String,
    pub certificate_id: String,
    pub verification_url: String,
}

impl Default for CertificateTemplateOptions {
    fn default() -> Self {
        Self {
            orientation: CertificateOrientation::Landscape,
            title: "Certificate of Achievement".to_string(),
            subtitle: "LynPDF RS Professional Program".to_string(),
            recipient_name: "Recipient Name".to_string(),
            statement:
                "for successfully completing the practical PDF engineering and document automation track"
                    .to_string(),
            issuer_name: "LynPDF RS Team".to_string(),
            issuer_role: "Program Director".to_string(),
            issue_date: "2026-05-03".to_string(),
            certificate_id: "LPR-REAL-0001".to_string(),
            verification_url: "https://example.com/verify/LPR-REAL-0001".to_string(),
        }
    }
}

pub fn build_fullpage_certificate_html(options: &CertificateTemplateOptions) -> String {
    let page_size = options.orientation.page_size();
    let width = page_size.width_pt;
    let height = page_size.height_pt;

    let is_landscape = matches!(options.orientation, CertificateOrientation::Landscape);

    let title_lines = wrap_text_lines(&options.title, if is_landscape { 34 } else { 26 }, 2);
    let subtitle_lines = wrap_text_lines(&options.subtitle, if is_landscape { 56 } else { 42 }, 2);
    let recipient_lines = wrap_text_lines(
        &options.recipient_name,
        if is_landscape { 28 } else { 24 },
        2,
    );
    let statement_lines = wrap_text_lines(
        &options.statement,
        if is_landscape { 76 } else { 56 },
        if is_landscape { 3 } else { 4 },
    );

    let mut title_font = if is_landscape { 42.0 } else { 38.0 };
    let mut subtitle_font = if is_landscape { 18.0 } else { 16.0 };
    let mut recipient_font = if is_landscape { 38.0 } else { 34.0 };
    let mut statement_font = if is_landscape { 16.0 } else { 15.0 };

    if title_lines.len() > 1 {
        title_font *= 0.84;
    }
    if subtitle_lines.len() > 1 {
        subtitle_font *= 0.92;
    }
    if recipient_lines.len() > 1 {
        recipient_font *= 0.84;
    }
    if statement_lines.len() > 2 {
        statement_font *= 0.94;
    }

    let title_line_height = title_font * 1.12;
    let subtitle_line_height = subtitle_font * 1.18;
    let recipient_line_height = recipient_font * 1.1;
    let statement_line_height = statement_font * 1.26;

    let center_x = width * 0.5;
    let title_y = height * if is_landscape { 0.22 } else { 0.23 };
    let subtitle_y = title_y + title_line_height * title_lines.len() as f32 + 14.0;
    let lead_y = subtitle_y + subtitle_line_height * subtitle_lines.len() as f32 + 38.0;
    let recipient_y = lead_y + 47.0;
    let recipient_bottom = recipient_y + recipient_line_height * recipient_lines.len() as f32;
    let rule_y = recipient_bottom + 8.0;
    let statement_y = rule_y + 34.0;
    let statement_bottom = statement_y + statement_line_height * statement_lines.len() as f32;

    let preferred_footer_y = height * if is_landscape { 0.84 } else { 0.845 };
    let footer_y = preferred_footer_y
        .max(statement_bottom + if is_landscape { 64.0 } else { 70.0 })
        .min(height - 64.0);
    let issuer_role_y = footer_y + 18.0;
    let meta_date_y = footer_y;
    let meta_id_y = footer_y + 18.0;
    let meta_verify_y = footer_y + 36.0;

    let title_svg = build_centered_svg_text_lines(
        &title_lines,
        center_x,
        title_y,
        title_font,
        title_line_height,
        "#b46f23",
        true,
    );
    let subtitle_svg = build_centered_svg_text_lines(
        &subtitle_lines,
        center_x,
        subtitle_y,
        subtitle_font,
        subtitle_line_height,
        "#32556f",
        false,
    );
    let recipient_svg = build_centered_svg_text_lines(
        &recipient_lines,
        center_x,
        recipient_y,
        recipient_font,
        recipient_line_height,
        "#0f2a45",
        true,
    );
    let statement_svg = build_centered_svg_text_lines(
        &statement_lines,
        center_x,
        statement_y,
        statement_font,
        statement_line_height,
        "#415e74",
        false,
    );

    let outer = 10.0;
    let inner = 26.0;

    format!(
        r##"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <title>{title}</title>
  <style>
    @page {{ size: {page_size_css}; margin: 0; }}
    html, body {{ margin: 0; padding: 0; }}
        body {{ background: #ffffff; }}
  </style>
</head>
<body>
  <svg width="{width:.3}pt" height="{height:.3}pt" viewBox="0 0 {width:.3} {height:.3}">
        <rect x="0" y="0" width="{width:.3}" height="{height:.3}" fill="#fafcff" />
    <rect x="{outer:.3}" y="{outer:.3}" width="{outer_w:.3}" height="{outer_h:.3}" fill="none" stroke="#0b2f4a" stroke-width="5" />
    <rect x="{inner:.3}" y="{inner:.3}" width="{inner_w:.3}" height="{inner_h:.3}" fill="none" stroke="#c8893d" stroke-width="1.5" />

{title_svg}    {subtitle_svg}

    <text x="{center_x:.3}" y="{lead_y:.3}" text-anchor="middle" font-size="17" fill="#4a6478">This certifies that</text>
{recipient_svg}    
    <line x1="{rule_left:.3}" y1="{rule_y:.3}" x2="{rule_right:.3}" y2="{rule_y:.3}" stroke="#c8893d" stroke-width="2" />

{statement_svg}    

    <text x="{issuer_x:.3}" y="{footer_y:.3}" text-anchor="middle" font-size="15" font-weight="bold" fill="#0f2a45">{issuer}</text>
    <text x="{issuer_x:.3}" y="{issuer_role_y:.3}" text-anchor="middle" font-size="12" fill="#506f86">{issuer_role}</text>

    <text x="{meta_x:.3}" y="{meta_date_y:.3}" text-anchor="end" font-size="12" fill="#314b60">Issued: {issue_date}</text>
    <text x="{meta_x:.3}" y="{meta_id_y:.3}" text-anchor="end" font-size="12" fill="#314b60">Certificate ID: {cert_id}</text>
    <text x="{meta_x:.3}" y="{meta_verify_y:.3}" text-anchor="end" font-size="11" fill="#5f778a">Verify: {verify_url}</text>
  </svg>
</body>
</html>
"##,
        page_size_css = options.orientation.css_page_size(),
        width = width,
        height = height,
        outer = outer,
        outer_w = width - outer * 2.0,
        outer_h = height - outer * 2.0,
        inner = inner,
        inner_w = width - inner * 2.0,
        inner_h = height - inner * 2.0,
        center_x = center_x,
        lead_y = lead_y,
        rule_left = width * 0.23,
        rule_right = width * 0.77,
        rule_y = rule_y,
        issuer_x = width * 0.28,
        footer_y = footer_y,
        issuer_role_y = issuer_role_y,
        meta_x = width * 0.92,
        meta_date_y = meta_date_y,
        meta_id_y = meta_id_y,
        meta_verify_y = meta_verify_y,
        title = escape_html(&options.title),
        title_svg = title_svg,
        subtitle_svg = subtitle_svg,
        recipient_svg = recipient_svg,
        statement_svg = statement_svg,
        issuer = escape_html(&options.issuer_name),
        issuer_role = escape_html(&options.issuer_role),
        issue_date = escape_html(&options.issue_date),
        cert_id = escape_html(&options.certificate_id),
        verify_url = escape_html(&options.verification_url),
    )
}

pub fn render_fullpage_certificate_pdf(
    options: &CertificateTemplateOptions,
) -> Result<PdfDocument> {
    let html = build_fullpage_certificate_html(options);
    let mut render_options = RenderOptions::default();
    render_options.page_size = options.orientation.page_size();
    render_options.margin_pt = 0.0;

    let base_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let request = RenderRequest {
        html,
        css: String::new(),
        base_dir,
        css_base_dir: None,
        options: render_options,
    };

    render_html_to_pdf(request)
}

pub fn render_fullpage_certificate_pdf_to_file(
    options: &CertificateTemplateOptions,
    output_path: impl AsRef<Path>,
) -> Result<PdfDocument> {
    let pdf = render_fullpage_certificate_pdf(options)?;
    fs::write(output_path.as_ref(), &pdf.bytes)?;
    Ok(pdf)
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn build_centered_svg_text_lines(
    lines: &[String],
    center_x: f32,
    start_y: f32,
    font_size: f32,
    line_height: f32,
    fill: &str,
    bold: bool,
) -> String {
    let mut out = String::new();
    let weight_attr = if bold { " font-weight=\"bold\"" } else { "" };
    for (index, line) in lines.iter().enumerate() {
        let y = start_y + line_height * index as f32;
        out.push_str(&format!(
            "    <text x=\"{center_x:.3}\" y=\"{y:.3}\" text-anchor=\"middle\" font-size=\"{font_size:.1}\"{weight_attr} fill=\"{fill}\">{}</text>\n",
            escape_html(line)
        ));
    }
    out
}

fn wrap_text_lines(text: &str, max_chars: usize, max_lines: usize) -> Vec<String> {
    let normalized = text
        .split_whitespace()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    if normalized.is_empty() {
        return vec![String::new()];
    }

    let max_chars = max_chars.max(8);
    let max_lines = max_lines.max(1);

    let mut lines = Vec::new();
    let mut current = String::new();
    for word in normalized.split(' ') {
        for chunk in split_word_chunks(word, max_chars) {
            if current.is_empty() {
                current = chunk;
                continue;
            }

            let projected = char_count(&current) + 1 + char_count(&chunk);
            if projected <= max_chars {
                current.push(' ');
                current.push_str(&chunk);
            } else {
                lines.push(current);
                current = chunk;
            }
        }
    }

    if !current.is_empty() {
        lines.push(current);
    }

    if lines.len() <= max_lines {
        return lines;
    }

    let mut limited = lines.into_iter().take(max_lines).collect::<Vec<_>>();
    if let Some(last) = limited.last_mut() {
        let max_visible = max_chars.saturating_sub(3).max(1);
        while char_count(last) > max_visible {
            last.pop();
        }
        while last.ends_with(' ') {
            last.pop();
        }
        last.push_str("...");
    }
    limited
}

fn split_word_chunks(word: &str, max_chars: usize) -> Vec<String> {
    if char_count(word) <= max_chars {
        return vec![word.to_string()];
    }

    let mut chunks = Vec::new();
    let mut current = String::new();
    for ch in word.chars() {
        if char_count(&current) >= max_chars {
            chunks.push(current);
            current = String::new();
        }
        current.push(ch);
    }

    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}

fn char_count(value: &str) -> usize {
    value.chars().count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generates_landscape_certificate_html() {
        let options = CertificateTemplateOptions::default();
        let html = build_fullpage_certificate_html(&options);
        assert!(html.contains("@page { size: A4 landscape; margin: 0; }"));
        assert!(html.contains("Certificate ID"));
    }

    #[test]
    fn generates_portrait_certificate_html() {
        let mut options = CertificateTemplateOptions::default();
        options.orientation = CertificateOrientation::Portrait;
        let html = build_fullpage_certificate_html(&options);
        assert!(html.contains("@page { size: A4 portrait; margin: 0; }"));
    }
}
