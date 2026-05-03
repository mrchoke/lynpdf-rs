use lynpdf_rs::{render_file_to_pdf, RenderOptions};
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let input = root.join("examples/demo-thai-fonts.html");
    let css = root.join("examples/styles.css");
    let output_dir = root.join("examples/output");
    std::fs::create_dir_all(&output_dir)?;
    let output = output_dir.join("demo-thai-fonts-rust.pdf");
    let result = render_file_to_pdf(input, Some(css), &output, RenderOptions::default())?;
    println!("wrote {} page(s) to {}", result.pages, output.display());
    Ok(())
}
