use lynpdf_rs::{render_file_to_pdf, LynPdfError, RenderOptions};
use std::path::PathBuf;

#[test]
fn renders_thai_typography_fixture_to_pdf() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let html = root.join("tests/fixtures/test-01-thai-typography.html");
    let css = root.join("tests/fixtures/test-styles.css");
    if !root.join("fonts/Sarabun-Regular.ttf").exists() {
        return;
    }

    let output_dir = root.join("tests/output");
    std::fs::create_dir_all(&output_dir).unwrap();
    let output = output_dir.join("test-01-thai-typography-rust.pdf");
    let result = render_file_to_pdf(html, Some(css), &output, RenderOptions::default()).unwrap();

    assert!(result.pages >= 1);
    assert!(result.bytes.starts_with(b"%PDF-1.7"));
    assert!(result.bytes.len() > 10_000);
    assert!(output.exists());
}

#[test]
fn renders_markdown_with_local_image_fixture() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let markdown = root.join("tests/fixtures/test-29-markdown-image.md");
    let output_dir = root.join("tests/output");
    std::fs::create_dir_all(&output_dir).unwrap();
    let output = output_dir.join("test-29-markdown-image-rust.pdf");

    let result = render_file_to_pdf(markdown, Option::<PathBuf>::None, &output, RenderOptions::default());

    let result = match result {
        Ok(result) => result,
        Err(LynPdfError::FontNotFound(_)) => return,
        Err(err) => panic!("failed to render markdown image fixture: {err}"),
    };

    let pdf_text = String::from_utf8_lossy(&result.bytes);
    assert!(result.pages >= 1);
    assert!(result.bytes.starts_with(b"%PDF-1.7"));
    assert!(pdf_text.contains("/Subtype /Image"));
    assert!(output.exists());
}
