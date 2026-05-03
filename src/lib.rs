//! LynPDF RS core.
//!
//! This crate is intentionally split into small stages that mirror the Thai
//! rendering report: parse HTML, parse CSS, resolve styles, shape text, compose
//! pages, and emit a PDF with embedded CID fonts.

mod error;
mod fonts;
mod html;
mod layout;
mod markdown;
mod pdf;
mod renderer;
mod style;
mod syntax;
mod templates;
mod text;
mod types;

pub const LYNPDF_RS_VERSION: &str = env!("CARGO_PKG_VERSION");

pub use crate::error::{LynPdfError, Result};
pub use crate::renderer::{render_file_to_pdf, render_html_to_pdf};
pub use crate::syntax::apply_syntax_highlighting_to_html;
pub use crate::templates::{
    build_fullpage_certificate_html, render_fullpage_certificate_pdf,
    render_fullpage_certificate_pdf_to_file, CertificateOrientation, CertificateTemplateOptions,
};
pub use crate::types::{
    Diagnostic, DiagnosticLevel, PageSize, PdfDocument, RenderFontStyle, RenderOptions,
    RenderRequest, UserFontMapping,
};

pub mod prelude {
    pub use crate::{
        apply_syntax_highlighting_to_html, build_fullpage_certificate_html, render_file_to_pdf,
        render_fullpage_certificate_pdf, render_fullpage_certificate_pdf_to_file,
        render_html_to_pdf, CertificateOrientation, CertificateTemplateOptions, PageSize,
        RenderFontStyle, RenderOptions, RenderRequest, UserFontMapping, LYNPDF_RS_VERSION,
    };
}
