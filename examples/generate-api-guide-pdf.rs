use lynpdf_rs::{render_file_to_pdf, RenderOptions, Result};
use std::fs;

fn main() -> Result<()> {
    fs::create_dir_all("docs/output")?;

    let options = RenderOptions::default()
        .with_syntax_highlighting(true)
        .with_syntax_highlight_theme("lynpdf-light");

    let pdf = render_file_to_pdf(
        "docs/api-guide-v0.1.1.md",
        Some("docs/api-guide-theme.css"),
        "docs/output/api-guide-v0.1.1.pdf",
        options,
    )?;

    println!(
        "generated docs/output/api-guide-v0.1.1.pdf (pages={}, bytes={})",
        pdf.pages,
        pdf.bytes.len()
    );

    Ok(())
}
