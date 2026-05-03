use lynpdf_rs::{
    render_fullpage_certificate_pdf_to_file, CertificateOrientation, CertificateTemplateOptions,
    Result,
};
use std::fs;
use std::path::PathBuf;

fn main() -> Result<()> {
    let output_dir = PathBuf::from("examples/output");
    fs::create_dir_all(&output_dir)?;

    let mut landscape = CertificateTemplateOptions::default();
    landscape.orientation = CertificateOrientation::Landscape;
    landscape.title = "Certificate of Achievement".to_string();
    landscape.recipient_name = "Landscape Recipient".to_string();
    landscape.certificate_id = "LPR-API-L-0001".to_string();
    landscape.verification_url = "https://example.com/verify/LPR-API-L-0001".to_string();

    let landscape_pdf = render_fullpage_certificate_pdf_to_file(
        &landscape,
        output_dir.join("api-certificate-single-page-landscape.pdf"),
    )?;

    let mut portrait = landscape.clone();
    portrait.orientation = CertificateOrientation::Portrait;
    portrait.title = "Certificate of Completion".to_string();
    portrait.recipient_name = "Portrait Recipient".to_string();
    portrait.certificate_id = "LPR-API-P-0001".to_string();
    portrait.verification_url = "https://example.com/verify/LPR-API-P-0001".to_string();

    let portrait_pdf = render_fullpage_certificate_pdf_to_file(
        &portrait,
        output_dir.join("api-certificate-single-page-portrait.pdf"),
    )?;

    println!(
        "generated landscape={} page(s), portrait={} page(s)",
        landscape_pdf.pages, portrait_pdf.pages
    );

    Ok(())
}
