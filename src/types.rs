use std::path::PathBuf;

#[derive(Debug, Clone, Copy)]
pub struct PageSize {
    pub width_pt: f32,
    pub height_pt: f32,
}

impl PageSize {
    pub const A4: Self = Self {
        width_pt: 595.2756,
        height_pt: 841.8898,
    };

    pub const LETTER: Self = Self {
        width_pt: 612.0,
        height_pt: 792.0,
    };
}

#[derive(Debug, Clone)]
pub struct RenderOptions {
    pub page_size: PageSize,
    pub margin_pt: f32,
    pub page_scale_percent: f32,
    pub fit_to_page: bool,
    pub default_font_family: String,
    pub default_font_size_pt: f32,
    pub user_font_dirs: Vec<PathBuf>,
    pub user_font_mappings: Vec<UserFontMapping>,
    pub enable_kerning: bool,
    pub enable_ligatures: bool,
    pub font_variations: Vec<String>,
    pub enable_syntax_highlighting: bool,
    pub syntax_highlight_theme: String,
    pub deterministic: bool,
    pub verbose: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderFontStyle {
    Normal,
    Italic,
}

#[derive(Debug, Clone)]
pub struct UserFontMapping {
    pub family: String,
    pub path: PathBuf,
    pub weight: u16,
    pub style: RenderFontStyle,
}

impl UserFontMapping {
    pub fn new(family: impl Into<String>, path: impl Into<PathBuf>) -> Self {
        Self {
            family: family.into(),
            path: path.into(),
            weight: 400,
            style: RenderFontStyle::Normal,
        }
    }

    pub fn with_weight(mut self, weight: u16) -> Self {
        self.weight = weight;
        self
    }

    pub fn with_style(mut self, style: RenderFontStyle) -> Self {
        self.style = style;
        self
    }
}

impl Default for RenderOptions {
    fn default() -> Self {
        Self {
            page_size: PageSize::A4,
            margin_pt: 42.5197,
            page_scale_percent: 100.0,
            fit_to_page: false,
            default_font_family: "Sarabun".to_string(),
            default_font_size_pt: 10.5,
            user_font_dirs: Vec::new(),
            user_font_mappings: Vec::new(),
            enable_kerning: true,
            enable_ligatures: true,
            font_variations: Vec::new(),
            enable_syntax_highlighting: true,
            syntax_highlight_theme: "lynpdf-light".to_string(),
            deterministic: true,
            verbose: false,
        }
    }
}

impl RenderOptions {
    pub fn page_scale_factor(&self) -> f32 {
        (self.page_scale_percent.clamp(10.0, 400.0)) / 100.0
    }

    pub fn with_user_font_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.user_font_dirs.push(dir.into());
        self
    }

    pub fn with_page_scale_percent(mut self, percent: f32) -> Self {
        self.page_scale_percent = percent;
        self
    }

    pub fn with_fit_to_page(mut self, enabled: bool) -> Self {
        self.fit_to_page = enabled;
        self
    }

    pub fn with_user_font_mapping(mut self, mapping: UserFontMapping) -> Self {
        self.user_font_mappings.push(mapping);
        self
    }

    pub fn with_kerning(mut self, enabled: bool) -> Self {
        self.enable_kerning = enabled;
        self
    }

    pub fn with_ligatures(mut self, enabled: bool) -> Self {
        self.enable_ligatures = enabled;
        self
    }

    pub fn with_font_variation(mut self, variation: impl Into<String>) -> Self {
        self.font_variations.push(variation.into());
        self
    }

    pub fn with_syntax_highlighting(mut self, enabled: bool) -> Self {
        self.enable_syntax_highlighting = enabled;
        self
    }

    pub fn with_syntax_highlight_theme(mut self, theme: impl Into<String>) -> Self {
        self.syntax_highlight_theme = theme.into();
        self
    }
}

#[derive(Debug, Clone)]
pub struct RenderRequest {
    pub html: String,
    pub css: String,
    pub base_dir: PathBuf,
    pub css_base_dir: Option<PathBuf>,
    pub options: RenderOptions,
}

#[derive(Debug, Clone, Default)]
pub struct DocumentMetadata {
    pub title: Option<String>,
    pub author: Option<String>,
    pub subject: Option<String>,
    pub keywords: Option<String>,
    pub creator: Option<String>,
    pub producer: Option<String>,
}

#[derive(Debug, Clone)]
pub struct PdfDocument {
    pub bytes: Vec<u8>,
    pub pages: usize,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticLevel {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub level: DiagnosticLevel,
    pub code: &'static str,
    pub message: String,
}

impl Diagnostic {
    pub fn info(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            level: DiagnosticLevel::Info,
            code,
            message: message.into(),
        }
    }

    pub fn warning(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            level: DiagnosticLevel::Warning,
            code,
            message: message.into(),
        }
    }
}
