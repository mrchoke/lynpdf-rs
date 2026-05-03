use lynpdf_rs::{render_file_to_pdf, RenderOptions};
use std::path::{Path, PathBuf};

fn has_font_fixture(root: &Path) -> bool {
    root.join("fonts/Sarabun-Regular.ttf").exists()
}

fn render_fixture(name: &str) -> lynpdf_rs::Result<lynpdf_rs::PdfDocument> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let html = root.join("tests/fixtures").join(name);
    let css = root.join("tests/fixtures/test-styles.css");
    let output = root
        .join("tests/output")
        .join(format!("{}-rust.pdf", name.trim_end_matches(".html")));

    std::fs::create_dir_all(root.join("tests/output")).unwrap();
    render_file_to_pdf(html, Some(css), output, RenderOptions::default())
}

#[test]
fn svg_transform_chain_fixture_emits_gradient_and_opacity_resources() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if !has_font_fixture(&root) {
        return;
    }

    let result = render_fixture("test-21-svg-transform-chain.html").unwrap();
    let pdf_text = String::from_utf8_lossy(&result.bytes);

    assert!(result.pages >= 1);
    assert!(result.bytes.starts_with(b"%PDF-1.7"));
    assert!(pdf_text.contains("/Pattern <<"));
    assert!(pdf_text.contains("/ShadingType 2"));
    assert!(pdf_text.contains(" scn\n"));
    assert!(pdf_text.contains("/ExtGState <<"));
    assert!(pdf_text.contains(" gs\n"));
}

#[test]
fn svg_gradient_stroke_fixture_uses_pattern_stroke_paint() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    if !has_font_fixture(&root) {
        return;
    }

    let result = render_fixture("test-22-svg-gradient-stroke.html").unwrap();
    let pdf_text = String::from_utf8_lossy(&result.bytes);

    assert!(result.pages >= 1);
    assert!(result.bytes.starts_with(b"%PDF-1.7"));
    assert!(pdf_text.contains("/Pattern <<"));
    assert!(pdf_text.contains("/ShadingType 2"));
    assert!(pdf_text.contains(" SCN\n"));
    assert!(pdf_text.contains("/FunctionType 3"));
    assert!(pdf_text.contains("/ExtGState <<"));
}
