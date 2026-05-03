use crate::error::Result;
use crate::fonts::FontRegistry;
use crate::html::{extract_document_metadata, extract_style_blocks, find_body, parse_html};
use crate::layout::layout_document;
use crate::markdown::{convert_markdown_to_html, markdown_default_css};
use crate::pdf::emit_pdf;
use crate::style::StyleSheet;
use crate::syntax::apply_syntax_highlighting_to_html;
use crate::types::{PdfDocument, RenderOptions, RenderRequest};
use std::fs;
use std::path::Path;

pub fn render_html_to_pdf(request: RenderRequest) -> Result<PdfDocument> {
    let highlighted_html = apply_syntax_highlighting_to_html(&request.html, &request.options);
    let root = parse_html(&highlighted_html)?;
    let metadata = extract_document_metadata(&root);
    let inline_css = extract_style_blocks(&root);
    let combined_css = format!("{}\n{}", request.css, inline_css);
    let stylesheet = StyleSheet::parse(&combined_css);
    let mut diagnostics = stylesheet.diagnostics.clone();
    diagnostics.push(crate::types::Diagnostic::info(
        "LYNPDF_VERSION",
        format!("LynPDF RS {}", crate::LYNPDF_RS_VERSION),
    ));
    let mut fonts = FontRegistry::discover(&request.options.default_font_family)?;
    let css_base_dir = request.css_base_dir.as_deref().unwrap_or(&request.base_dir);
    fonts.register_font_faces(
        &stylesheet.font_faces,
        css_base_dir,
        &request.base_dir,
        &mut diagnostics,
    );
    fonts.register_user_fonts(
        &request.options.user_font_dirs,
        &request.options.user_font_mappings,
        &request.base_dir,
        &mut diagnostics,
    );
    fonts.finalize_default_font(&request.options.default_font_family)?;
    let body = find_body(&root);
    let (layout, resolved_scale_percent) = layout_document_with_fit(
        body,
        &stylesheet,
        &fonts,
        &request.options,
        &request.base_dir,
    )?;
    let bytes = emit_pdf(&layout, &fonts, &metadata)?;

    if request.options.fit_to_page {
        if layout.pages.len() <= 1 {
            diagnostics.push(crate::types::Diagnostic::info(
                "LYNPDF_FIT_TO_PAGE",
                format!(
                    "fit-to-page active; rendered {} page(s) at {:.2}% scale",
                    layout.pages.len(),
                    resolved_scale_percent
                ),
            ));
        } else {
            diagnostics.push(crate::types::Diagnostic::warning(
                "LYNPDF_FIT_TO_PAGE_UNSATISFIED",
                format!(
                    "fit-to-page requested but content cannot be reduced to one page; kept {} page(s) at {:.2}% scale",
                    layout.pages.len(),
                    resolved_scale_percent
                ),
            ));
        }
    }

    diagnostics.push(crate::types::Diagnostic::info(
        "LYNPDF_RENDERED",
        format!(
            "rendered {} page(s) with {} bytes",
            layout.pages.len(),
            bytes.len()
        ),
    ));

    Ok(PdfDocument {
        bytes,
        pages: layout.pages.len(),
        diagnostics,
    })
}

fn layout_document_with_fit(
    body: &crate::html::HtmlNode,
    stylesheet: &crate::style::StyleSheet,
    fonts: &crate::fonts::FontRegistry,
    options: &RenderOptions,
    base_dir: &Path,
) -> Result<(crate::layout::LayoutDocument, f32)> {
    let max_scale_percent = options.page_scale_percent.clamp(10.0, 400.0);

    let mut initial_options = options.clone();
    initial_options.fit_to_page = false;
    initial_options.page_scale_percent = max_scale_percent;
    let initial_layout = layout_document(body, stylesheet, fonts, &initial_options, base_dir)?;

    if !options.fit_to_page || initial_layout.pages.len() <= 1 {
        return Ok((initial_layout, max_scale_percent));
    }

    let min_scale_percent = 10.0f32;
    let mut min_probe_options = initial_options.clone();
    min_probe_options.page_scale_percent = min_scale_percent;
    let min_layout = layout_document(body, stylesheet, fonts, &min_probe_options, base_dir)?;
    if min_layout.pages.len() > 1 {
        let initial_pages = initial_layout.pages.len();
        let minimum_pages = min_layout.pages.len();
        if minimum_pages >= initial_pages {
            return Ok((initial_layout, max_scale_percent));
        }

        let mut low = min_scale_percent;
        let mut high = max_scale_percent;
        let mut best_scale = min_scale_percent;
        let mut best_layout = min_layout;

        for _ in 0..12 {
            let mid = (low + high) * 0.5;
            let mut probe_options = initial_options.clone();
            probe_options.page_scale_percent = mid;
            let layout = layout_document(body, stylesheet, fonts, &probe_options, base_dir)?;
            if layout.pages.len() <= minimum_pages {
                best_scale = mid;
                best_layout = layout;
                low = mid;
            } else {
                high = mid;
            }
        }

        return Ok((best_layout, best_scale));
    }

    let mut low = min_scale_percent;
    let mut high = max_scale_percent;
    let mut best_scale = min_scale_percent;
    let mut best_layout = min_layout;

    for _ in 0..12 {
        let mid = (low + high) * 0.5;
        let mut probe_options = initial_options.clone();
        probe_options.page_scale_percent = mid;
        let layout = layout_document(body, stylesheet, fonts, &probe_options, base_dir)?;
        if layout.pages.len() <= 1 {
            best_scale = mid;
            best_layout = layout;
            low = mid;
        } else {
            high = mid;
        }
    }

    Ok((best_layout, best_scale))
}

pub fn render_file_to_pdf(
    html_path: impl AsRef<Path>,
    css_path: Option<impl AsRef<Path>>,
    output_path: impl AsRef<Path>,
    options: RenderOptions,
) -> Result<PdfDocument> {
    let html_path = html_path.as_ref();
    let mut html = fs::read_to_string(html_path)?;
    let mut css = String::new();
    let mut css_base_dir = None;
    if let Some(css_path) = css_path {
        let css_path = css_path.as_ref();
        css.push_str(&fs::read_to_string(css_path)?);
        css_base_dir = css_path.parent().map(Path::to_path_buf);
    }

    if html_path
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| value.eq_ignore_ascii_case("md"))
    {
        html = convert_markdown_to_html(&html);
        css = if css.trim().is_empty() {
            markdown_default_css().to_string()
        } else {
            format!("{}\n{}", markdown_default_css(), css)
        };
    }

    let base_dir = html_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    let result = render_html_to_pdf(RenderRequest {
        html,
        css,
        base_dir,
        css_base_dir,
        options,
    })?;
    fs::write(output_path, &result.bytes)?;
    Ok(result)
}
