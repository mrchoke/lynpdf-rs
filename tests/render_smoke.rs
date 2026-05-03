use lynpdf_rs::{render_file_to_pdf, RenderOptions};
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
