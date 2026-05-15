use crate::error::{LynPdfError, Result};
use crate::style::{FontFaceRule, FontFaceSource, FontStyle, FontWeight};
use crate::types::{Diagnostic, RenderFontStyle, UserFontMapping};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use ttf_parser::{Face, GlyphId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FontKey(pub usize);

#[derive(Debug, Clone)]
pub struct LoadedFont {
    pub family: String,
    pub path: PathBuf,
    pub data: Vec<u8>,
    pub units_per_em: f32,
    pub ascender: f32,
    pub descender: f32,
    pub bbox: [i16; 4],
}

#[derive(Debug)]
pub struct FontRegistry {
    fonts: Vec<LoadedFont>,
    by_variant: HashMap<(String, FontWeight, FontStyle), FontKey>,
    default_key: FontKey,
}

impl FontRegistry {
    pub fn discover(default_family: &str) -> Result<Self> {
        let mut registry = Self {
            fonts: Vec::new(),
            by_variant: HashMap::new(),
            default_key: FontKey(0),
        };

        let roots = font_roots();
        for root in roots {
            registry.register_fonts_from_dir(&root, false)?;
        }

        if registry.fonts.is_empty() {
            for root in system_font_roots() {
                registry.register_fonts_from_dir(&root, false)?;
            }
        }

        if !registry.fonts.is_empty() {
            registry.default_key = registry.resolve_default_key(default_family).unwrap_or(FontKey(0));
        }

        Ok(registry)
    }

    pub fn finalize_default_font(&mut self, default_family: &str) -> Result<()> {
        if self.fonts.is_empty() {
            return Err(LynPdfError::FontNotFound(missing_font_message()));
        }

        self.default_key = self.resolve_default_key(default_family).unwrap_or(FontKey(0));

        Ok(())
    }

    fn resolve_default_key(&self, default_family: &str) -> Option<FontKey> {
        let mut normalized_seen = HashSet::new();
        let mut candidates = Vec::new();

        for family in [
            default_family,
            "Sarabun",
            "Waree",
            "Noto Sans Thai",
            "Noto Sans",
            "DejaVu Sans",
            "Liberation Sans",
            "Arial",
            "Helvetica",
        ] {
            let normalized = normalize_family(family);
            if normalized.is_empty() || !normalized_seen.insert(normalized) {
                continue;
            }
            candidates.push(family);
        }

        for family in candidates {
            if let Some(key) = self.resolve(family, FontWeight::Regular, FontStyle::Normal) {
                return Some(key);
            }
        }

        None
    }

    pub fn resolve(&self, family: &str, weight: FontWeight, style: FontStyle) -> Option<FontKey> {
        let family = normalize_family(family);
        self.by_variant
            .get(&(family.clone(), weight, style))
            .copied()
            .or_else(|| {
                self.by_variant
                    .get(&(family.clone(), FontWeight::Regular, style))
                    .copied()
            })
            .or_else(|| {
                self.by_variant
                    .get(&(family, FontWeight::Regular, FontStyle::Normal))
                    .copied()
            })
    }

    pub fn resolve_for_text(
        &self,
        families: &[String],
        weight: FontWeight,
        style: FontStyle,
        text: &str,
    ) -> FontKey {
        let mut first_found = None;
        for family in families {
            let Some(candidate) = self.resolve(family, weight, style) else {
                continue;
            };
            if first_found.is_none() {
                first_found = Some(candidate);
            }
            if self.get(candidate).supports_text(text) {
                return candidate;
            }
        }
        first_found.unwrap_or(self.default_key)
    }

    pub fn resolve_for_cluster(
        &self,
        families: &[String],
        weight: FontWeight,
        style: FontStyle,
        text: &str,
    ) -> FontKey {
        let prefers_color_emoji = families_prefer_color_emoji(families);

        if prefers_color_emoji && text_contains_emoji(text) {
            if let Some((idx, _)) = self
                .fonts
                .iter()
                .enumerate()
                .rev()
                .find(|(_, font)| font.is_color_emoji_font() && font.supports_text(text))
            {
                return FontKey(idx);
            }
        }

        let candidate = self.resolve_for_text(families, weight, style, text);
        if text.trim().is_empty() || self.get(candidate).supports_text(text) {
            return candidate;
        }

        if self.get(self.default_key).supports_text(text) {
            return self.default_key;
        }

        if text_contains_emoji(text) && !prefers_color_emoji {
            if let Some((idx, _)) = self
                .fonts
                .iter()
                .enumerate()
                .rev()
                .find(|(_, font)| !font.is_color_emoji_font() && font.supports_text(text))
            {
                return FontKey(idx);
            }
        }

        self.fonts
            .iter()
            .enumerate()
            .rev()
            .find(|(_, font)| font.supports_text(text))
            .map(|(idx, _)| FontKey(idx))
            .unwrap_or(candidate)
    }

    pub fn get(&self, key: FontKey) -> &LoadedFont {
        &self.fonts[key.0]
    }

    pub fn iter(&self) -> impl Iterator<Item = (FontKey, &LoadedFont)> {
        self.fonts
            .iter()
            .enumerate()
            .map(|(idx, font)| (FontKey(idx), font))
    }

    pub fn register_user_fonts(
        &mut self,
        user_font_dirs: &[PathBuf],
        user_font_mappings: &[UserFontMapping],
        base_dir: &Path,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for dir in user_font_dirs {
            let resolved = if dir.is_absolute() {
                dir.clone()
            } else {
                base_dir.join(dir)
            };
            if !resolved.exists() {
                diagnostics.push(Diagnostic::warning(
                    "USER_FONT_DIR_NOT_FOUND",
                    format!("user font dir not found: {}", resolved.display()),
                ));
                continue;
            }
            if let Err(err) = self.register_fonts_from_dir(&resolved, true) {
                diagnostics.push(Diagnostic::warning(
                    "USER_FONT_DIR_REGISTER_FAILED",
                    format!(
                        "failed to load user fonts from {}: {err}",
                        resolved.display()
                    ),
                ));
            }
        }

        for mapping in user_font_mappings {
            let path = if mapping.path.is_absolute() {
                mapping.path.clone()
            } else {
                base_dir.join(&mapping.path)
            };
            if !path.exists() {
                diagnostics.push(Diagnostic::warning(
                    "USER_FONT_MAPPING_NOT_FOUND",
                    format!(
                        "user font mapping file not found for '{}': {}",
                        mapping.family,
                        path.display()
                    ),
                ));
                continue;
            }

            let weight = if mapping.weight >= 600 {
                FontWeight::Bold
            } else {
                FontWeight::Regular
            };
            let style = match mapping.style {
                RenderFontStyle::Normal => FontStyle::Normal,
                RenderFontStyle::Italic => FontStyle::Italic,
            };

            if let Err(err) = self.register_font(&mapping.family, weight, style, path, true) {
                diagnostics.push(Diagnostic::warning(
                    "USER_FONT_MAPPING_REGISTER_FAILED",
                    format!(
                        "failed to register user font mapping '{}' ({:?}/{:?}): {err}",
                        mapping.family, weight, style
                    ),
                ));
            }
        }
    }

    pub fn register_font_faces(
        &mut self,
        font_faces: &[FontFaceRule],
        css_base_dir: &Path,
        html_base_dir: &Path,
        diagnostics: &mut Vec<Diagnostic>,
    ) {
        for font_face in font_faces {
            let mut registered = false;
            for source in &font_face.sources {
                let path = resolve_font_source_path(&source.url, css_base_dir, html_base_dir);
                let Some(path) = path else {
                    diagnostics.push(Diagnostic::warning(
                        "FONT_FACE_SOURCE_UNSUPPORTED",
                        format!(
                            "unsupported @font-face src '{}' for family '{}'",
                            source.url, font_face.font_family
                        ),
                    ));
                    continue;
                };

                if !is_supported_font_source(source, &path) {
                    diagnostics.push(Diagnostic::warning(
                        "FONT_FACE_SOURCE_FORMAT_UNSUPPORTED",
                        format!(
                            "unsupported @font-face format/source '{}' for family '{}'",
                            source.url, font_face.font_family
                        ),
                    ));
                    continue;
                }

                if !path.exists() {
                    diagnostics.push(Diagnostic::warning(
                        "FONT_FACE_SOURCE_NOT_FOUND",
                        format!(
                            "@font-face font file not found for family '{}': {}",
                            font_face.font_family,
                            path.display()
                        ),
                    ));
                    continue;
                }

                match self.register_font(
                    &font_face.font_family,
                    font_face.font_weight,
                    font_face.font_style,
                    path,
                    true,
                ) {
                    Ok(_) => {
                        registered = true;
                        break;
                    }
                    Err(err) => diagnostics.push(Diagnostic::warning(
                        "FONT_FACE_REGISTER_FAILED",
                        format!(
                            "failed to register @font-face '{}' ({:?}/{:?}): {err}",
                            font_face.font_family, font_face.font_weight, font_face.font_style
                        ),
                    )),
                }
            }

            if !registered {
                diagnostics.push(Diagnostic::warning(
                    "FONT_FACE_NO_USABLE_SOURCE",
                    format!(
                        "no usable @font-face source for '{}' ({:?}/{:?})",
                        font_face.font_family, font_face.font_weight, font_face.font_style
                    ),
                ));
            }
        }
    }

    fn register_fonts_from_dir(&mut self, root: &Path, override_existing: bool) -> Result<()> {
        if !root.exists() {
            return Ok(());
        }

        let mut font_files = Vec::new();
        collect_font_files(root, &mut font_files)?;
        font_files.sort();

        for path in font_files {
            let Some((family, weight, style)) = infer_variant_from_path(&path) else {
                continue;
            };
            if self
                .register_font(&family, weight, style, path.clone(), override_existing)
                .is_err()
            {
                continue;
            }
        }

        Ok(())
    }

    fn register_font(
        &mut self,
        family: &str,
        weight: FontWeight,
        style: FontStyle,
        path: PathBuf,
        override_existing: bool,
    ) -> Result<()> {
        let key_tuple = (normalize_family(family), weight, style);
        if !override_existing && self.by_variant.contains_key(&key_tuple) {
            return Ok(());
        }

        let data = fs::read(&path)?;
        let face = Face::parse(&data, 0)
            .map_err(|err| LynPdfError::FontParse(format!("{}: {err:?}", path.display())))?;
        let bbox = face.global_bounding_box();
        let units_per_em = face.units_per_em() as f32;
        let ascender = face.ascender() as f32;
        let descender = face.descender() as f32;
        let bbox_values = [bbox.x_min, bbox.y_min, bbox.x_max, bbox.y_max];
        let font = LoadedFont {
            family: family.to_string(),
            path,
            data,
            units_per_em,
            ascender,
            descender,
            bbox: bbox_values,
        };
        let key = FontKey(self.fonts.len());
        self.fonts.push(font);
        self.by_variant.insert(key_tuple, key);
        Ok(())
    }
}

impl LoadedFont {
    pub fn glyph_advance_design_units(&self, glyph_id: u16) -> u16 {
        Face::parse(&self.data, 0)
            .ok()
            .and_then(|face| face.glyph_hor_advance(GlyphId(glyph_id)))
            .unwrap_or(0)
    }

    pub fn glyph_advance_1000(&self, glyph_id: u16) -> i32 {
        let advance = self.glyph_advance_design_units(glyph_id) as f32;
        ((advance / self.units_per_em) * 1000.0).round() as i32
    }

    pub fn supports_text(&self, text: &str) -> bool {
        let Ok(face) = Face::parse(&self.data, 0) else {
            return false;
        };
        for ch in text.chars() {
            if ch.is_whitespace() || is_emoji_sequence_control_char(ch) {
                continue;
            }
            if face.glyph_index(ch).is_none() {
                return false;
            }
        }
        true
    }

    pub(crate) fn is_color_emoji_font(&self) -> bool {
        let family = self.family.to_ascii_lowercase();
        if family.contains("noto color emoji") || family.contains("notocoloremoji") {
            return true;
        }

        let file_name = self
            .path
            .file_name()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        file_name.contains("notocoloremoji")
    }
}

fn text_contains_emoji(text: &str) -> bool {
    text.chars().any(is_emoji_char)
}

fn families_prefer_color_emoji(families: &[String]) -> bool {
    families
        .iter()
        .any(|family| is_color_emoji_family_name(family))
}

fn is_color_emoji_family_name(family: &str) -> bool {
    let normalized = normalize_family(family);
    normalized.contains("notocoloremoji") || normalized.contains("coloremoji")
}

fn is_emoji_char(ch: char) -> bool {
    matches!(
        ch as u32,
        0x1F1E6..=0x1F1FF
            | 0x1F300..=0x1FAFF
            | 0x2600..=0x26FF
            | 0x2700..=0x27BF
    )
}

fn is_emoji_sequence_control_char(ch: char) -> bool {
    matches!(
        ch,
        '\u{200D}' | '\u{200C}' | '\u{FE0E}' | '\u{FE0F}' | '\u{20E3}' | '\u{E0020}'..='\u{E007F}'
    )
}

fn font_roots() -> Vec<PathBuf> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let cwd = std::env::current_dir().unwrap_or_else(|_| manifest.clone());
    let mut roots = Vec::new();
    let mut seen = HashSet::new();

    push_unique_root(&mut roots, &mut seen, manifest.join("fonts"));
    push_unique_root(&mut roots, &mut seen, manifest.join("..").join("fonts"));
    push_unique_root(&mut roots, &mut seen, cwd.join("fonts"));
    push_unique_root(&mut roots, &mut seen, cwd.join("lynpdf-rs").join("fonts"));

    for ancestor in cwd.ancestors() {
        push_unique_root(&mut roots, &mut seen, ancestor.join("fonts"));
        push_unique_root(
            &mut roots,
            &mut seen,
            ancestor.join("lynpdf-rs").join("fonts"),
        );
    }

    if let Ok(exe_path) = std::env::current_exe() {
        for ancestor in exe_path.ancestors() {
            push_unique_root(&mut roots, &mut seen, ancestor.join("fonts"));
        }
    }

    if let Some(raw) = std::env::var_os("LYNPDF_FONT_DIRS") {
        for path in split_font_dirs(raw.to_string_lossy().as_ref()) {
            let font_dir = PathBuf::from(path);
            if font_dir.as_os_str().is_empty() {
                continue;
            }
            if font_dir.is_absolute() {
                push_unique_root(&mut roots, &mut seen, font_dir);
            } else {
                push_unique_root(&mut roots, &mut seen, cwd.join(font_dir));
            }
        }
    }

    roots
}

fn system_font_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    let mut seen = HashSet::new();

    #[cfg(target_os = "linux")]
    {
        push_unique_root(&mut roots, &mut seen, PathBuf::from("/usr/share/fonts"));
        push_unique_root(&mut roots, &mut seen, PathBuf::from("/usr/local/share/fonts"));
        if let Some(home) = std::env::var_os("HOME") {
            let home = PathBuf::from(home);
            push_unique_root(&mut roots, &mut seen, home.join(".fonts"));
            push_unique_root(&mut roots, &mut seen, home.join(".local/share/fonts"));
        }
    }

    #[cfg(target_os = "macos")]
    {
        push_unique_root(&mut roots, &mut seen, PathBuf::from("/System/Library/Fonts"));
        push_unique_root(&mut roots, &mut seen, PathBuf::from("/Library/Fonts"));
        if let Some(home) = std::env::var_os("HOME") {
            push_unique_root(
                &mut roots,
                &mut seen,
                PathBuf::from(home).join("Library/Fonts"),
            );
        }
    }

    #[cfg(target_os = "windows")]
    {
        if let Some(win_dir) = std::env::var_os("WINDIR") {
            push_unique_root(
                &mut roots,
                &mut seen,
                PathBuf::from(win_dir).join("Fonts"),
            );
        }
    }

    roots
}

fn push_unique_root(roots: &mut Vec<PathBuf>, seen: &mut HashSet<PathBuf>, root: PathBuf) {
    if !seen.insert(root.clone()) {
        return;
    }
    roots.push(root);
}

fn split_font_dirs(value: &str) -> Vec<&str> {
    #[cfg(windows)]
    {
        value.split(';').map(str::trim).collect()
    }

    #[cfg(not(windows))]
    {
        value.split(':').map(str::trim).collect()
    }
}

fn missing_font_message() -> String {
    "No usable fonts found after scanning bundled/workspace/system font directories. Configure fonts via --font-dir, --font-map, CSS @font-face, or LYNPDF_FONT_DIRS. See README Font Setup section.".to_string()
}

fn normalize_family(family: &str) -> String {
    family
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .replace(' ', "")
        .to_ascii_lowercase()
}

fn collect_font_files(root: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            collect_font_files(&path, out)?;
            continue;
        }
        if !path.is_file() {
            continue;
        }
        let Some(ext) = path.extension().and_then(|value| value.to_str()) else {
            continue;
        };
        if matches!(ext.to_ascii_lowercase().as_str(), "ttf" | "otf" | "ttc") {
            out.push(path);
        }
    }
    Ok(())
}

fn infer_variant_from_path(path: &Path) -> Option<(String, FontWeight, FontStyle)> {
    let stem = path.file_stem()?.to_str()?.trim().to_string();
    if stem.is_empty() {
        return None;
    }

    let variants = [
        ("-bolditalic", FontWeight::Bold, FontStyle::Italic),
        ("-boldoblique", FontWeight::Bold, FontStyle::Italic),
        ("-bold", FontWeight::Bold, FontStyle::Normal),
        ("-italic", FontWeight::Regular, FontStyle::Italic),
        ("-oblique", FontWeight::Regular, FontStyle::Italic),
        ("-lightoblique", FontWeight::Regular, FontStyle::Italic),
        ("-light", FontWeight::Regular, FontStyle::Normal),
        ("-regular", FontWeight::Regular, FontStyle::Normal),
    ];

    let lower = stem.to_ascii_lowercase();
    let mut family_token = stem.as_str();
    let mut weight = FontWeight::Regular;
    let mut style = FontStyle::Normal;
    for (suffix, parsed_weight, parsed_style) in variants {
        if lower.ends_with(suffix) && stem.len() > suffix.len() {
            family_token = stem[..stem.len() - suffix.len()].trim_end_matches(['-', '_', ' ']);
            weight = parsed_weight;
            style = parsed_style;
            break;
        }
    }

    let family = map_family_alias(family_token);
    Some((family, weight, style))
}

fn map_family_alias(token: &str) -> String {
    let normalized = token.trim().replace(['_', '-'], "");
    match normalized.to_ascii_lowercase().as_str() {
        "ibmplexsans" => "IBM Plex Sans".to_string(),
        "intervariable" => "Inter Variable".to_string(),
        "chakrapetch" => "Chakra Petch".to_string(),
        _ => token.trim().to_string(),
    }
}

fn resolve_font_source_path(
    src: &str,
    css_base_dir: &Path,
    html_base_dir: &Path,
) -> Option<PathBuf> {
    let src = src.trim();
    if src.is_empty() {
        return None;
    }
    if src.starts_with("http://") || src.starts_with("https://") || src.starts_with("data:") {
        return None;
    }

    let src = src.strip_prefix("file://").unwrap_or(src);
    let src = strip_query_and_fragment(src);
    let candidate = PathBuf::from(src);
    if candidate.is_absolute() {
        return Some(candidate);
    }

    let css_relative = css_base_dir.join(&candidate);
    if css_relative.exists() {
        return Some(css_relative);
    }

    let html_relative = html_base_dir.join(&candidate);
    if html_relative.exists() {
        return Some(html_relative);
    }

    let manifest_relative = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(&candidate);
    if manifest_relative.exists() {
        return Some(manifest_relative);
    }

    Some(css_relative)
}

fn is_supported_font_source(source: &FontFaceSource, path: &Path) -> bool {
    let ext = path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase());
    let ext_supported = matches!(ext.as_deref(), Some("ttf") | Some("otf") | Some("ttc"));

    if let Some(format_hint) = &source.format_hint {
        let format = format_hint.to_ascii_lowercase();
        let hint_supported = matches!(
            format.as_str(),
            "truetype"
                | "truetype-collection"
                | "opentype"
                | "ttf"
                | "otf"
                | "ttc"
                | "woff2-variations"
                | "woff-variations"
        );
        return ext_supported && hint_supported;
    }

    ext_supported
}

fn strip_query_and_fragment(input: &str) -> &str {
    let mut end = input.len();
    if let Some(index) = input.find('?') {
        end = end.min(index);
    }
    if let Some(index) = input.find('#') {
        end = end.min(index);
    }
    &input[..end]
}
