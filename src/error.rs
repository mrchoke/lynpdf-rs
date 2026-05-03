use thiserror::Error;

pub type Result<T> = std::result::Result<T, LynPdfError>;

#[derive(Debug, Error)]
pub enum LynPdfError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("HTML parse error: {0}")]
    Html(String),

    #[error("CSS parse error: {0}")]
    Css(String),

    #[error("font not found: {0}")]
    FontNotFound(String),

    #[error("font parse error: {0}")]
    FontParse(String),

    #[error("PDF emit error: {0}")]
    Pdf(String),
}
