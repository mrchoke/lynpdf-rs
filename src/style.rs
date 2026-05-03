use crate::html::HtmlNode;
use crate::types::Diagnostic;
use lightningcss::stylesheet::{ParserOptions, StyleSheet as LightningStyleSheet};

#[derive(Debug, Clone)]
pub struct StyleSheet {
    pub rules: Vec<StyleRule>,
    pub font_faces: Vec<FontFaceRule>,
    pub page: PageStyle,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone)]
pub struct StyleRule {
    pub selectors: Vec<String>,
    pub declarations: Vec<Declaration>,
    pub order: usize,
}

#[derive(Debug, Clone)]
pub struct Declaration {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone)]
pub struct FontFaceRule {
    pub font_family: String,
    pub sources: Vec<FontFaceSource>,
    pub font_weight: FontWeight,
    pub font_style: FontStyle,
}

#[derive(Debug, Clone)]
pub struct FontFaceSource {
    pub url: String,
    pub format_hint: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct PageStyle {
    pub width_pt: Option<f32>,
    pub height_pt: Option<f32>,
    pub margin_pt: Option<f32>,
}

#[derive(Debug, Clone)]
pub struct ComputedStyle {
    pub display: Display,
    pub font_family: String,
    pub font_families: Vec<String>,
    pub font_size_pt: f32,
    pub font_weight: FontWeight,
    pub font_style: FontStyle,
    pub line_height_pt: f32,
    pub color: Color,
    pub background_color: Option<Color>,
    pub margin: Edges,
    pub padding: Edges,
    pub border_width_pt: f32,
    pub border_color: Color,
    pub border_style: BorderStyle,
    pub border_collapse: BorderCollapse,
    pub border_spacing_pt: f32,
    pub width_pt: Option<f32>,
    pub width_percent: Option<f32>,
    pub text_align: TextAlign,
    pub text_justify: TextJustify,
    pub white_space: WhiteSpace,
    pub word_break: WordBreak,
    pub overflow_wrap: OverflowWrap,
    pub line_break: LineBreak,
    pub hyphens: Hyphens,
    pub page_break_before: bool,
    pub page_break_inside_avoid: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Display {
    Block,
    Inline,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FontWeight {
    Regular,
    Bold,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FontStyle {
    Normal,
    Italic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAlign {
    Left,
    Center,
    Right,
    Justify,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextJustify {
    Auto,
    None,
    InterWord,
    InterCharacter,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhiteSpace {
    Normal,
    NoWrap,
    Pre,
    PreWrap,
    PreLine,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WordBreak {
    Normal,
    BreakAll,
    KeepAll,
    BreakWord,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverflowWrap {
    Normal,
    BreakWord,
    Anywhere,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineBreak {
    Auto,
    Loose,
    Normal,
    Strict,
    Anywhere,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hyphens {
    Manual,
    None,
    Auto,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorderCollapse {
    Separate,
    Collapse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BorderStyle {
    None,
    Hidden,
    Dotted,
    Dashed,
    Solid,
    Double,
    Groove,
    Ridge,
    Inset,
    Outset,
}

impl BorderStyle {
    pub fn is_visible(self) -> bool {
        !matches!(self, BorderStyle::None | BorderStyle::Hidden)
    }

    pub fn precedence_rank(self) -> u8 {
        match self {
            BorderStyle::Hidden => 9,
            BorderStyle::Double => 8,
            BorderStyle::Solid => 7,
            BorderStyle::Dashed => 6,
            BorderStyle::Dotted => 5,
            BorderStyle::Ridge => 4,
            BorderStyle::Outset => 3,
            BorderStyle::Groove => 2,
            BorderStyle::Inset => 1,
            BorderStyle::None => 0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Edges {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl StyleSheet {
    pub fn parse(css: &str) -> Self {
        let mut diagnostics = Vec::new();

        if let Err(err) = LightningStyleSheet::parse(css, ParserOptions::default()) {
            diagnostics.push(Diagnostic::warning(
                "CSS_LIGHTNING_PARSE_WARNING",
                format!("lightningcss rejected part of the stylesheet; falling back to MVP parser: {err}"),
            ));
        }

        let mut rules = Vec::new();
        let mut font_faces = Vec::new();
        let mut page = PageStyle {
            width_pt: None,
            height_pt: None,
            margin_pt: None,
        };

        for (order, (selector, body)) in split_css_rules(&strip_comments(css))
            .into_iter()
            .enumerate()
        {
            let selector_trimmed = selector.trim();
            let declarations = parse_declarations(&body);
            if selector_trimmed.starts_with("@font-face") {
                match parse_font_face_rule(&declarations) {
                    Ok(rule) => font_faces.push(rule),
                    Err(err) => diagnostics.push(Diagnostic::warning("CSS_FONT_FACE_INVALID", err)),
                }
                continue;
            }

            if selector_trimmed.starts_with("@page") {
                if let Some(margin) = declaration_value(&declarations, "margin")
                    .and_then(|value| parse_length(value, 10.5))
                {
                    page.margin_pt = Some(margin);
                }
                if let Some(size) = declaration_value(&declarations, "size") {
                    if let Some((width, height)) = parse_page_size(size) {
                        page.width_pt = Some(width);
                        page.height_pt = Some(height);
                    }
                }
                continue;
            }
            if selector_trimmed.starts_with('@') {
                continue;
            }

            rules.push(StyleRule {
                selectors: selector
                    .split(',')
                    .map(|part| part.trim().to_string())
                    .collect(),
                declarations,
                order,
            });
        }

        Self {
            rules,
            font_faces,
            page,
            diagnostics,
        }
    }
}

impl ComputedStyle {
    pub fn root(default_font_family: &str, default_font_size_pt: f32) -> Self {
        let font_size_pt = default_font_size_pt;
        let mut font_families = parse_font_family_list(default_font_family);
        if font_families.is_empty() {
            font_families.push(default_font_family.to_string());
        }
        let font_family = font_families
            .first()
            .cloned()
            .unwrap_or_else(|| default_font_family.to_string());
        Self {
            display: Display::Block,
            font_family,
            font_families,
            font_size_pt,
            font_weight: FontWeight::Regular,
            font_style: FontStyle::Normal,
            line_height_pt: font_size_pt * 1.45,
            color: Color::rgb_u8(51, 51, 51),
            background_color: None,
            margin: Edges::zero(),
            padding: Edges::zero(),
            border_width_pt: 0.0,
            border_color: Color::rgb_u8(204, 204, 204),
            border_style: BorderStyle::Solid,
            border_collapse: BorderCollapse::Separate,
            border_spacing_pt: 0.0,
            width_pt: None,
            width_percent: None,
            text_align: TextAlign::Left,
            text_justify: TextJustify::Auto,
            white_space: WhiteSpace::Normal,
            word_break: WordBreak::Normal,
            overflow_wrap: OverflowWrap::Normal,
            line_break: LineBreak::Auto,
            hyphens: Hyphens::Manual,
            page_break_before: false,
            page_break_inside_avoid: false,
        }
    }
}

impl Edges {
    pub fn zero() -> Self {
        Self {
            top: 0.0,
            right: 0.0,
            bottom: 0.0,
            left: 0.0,
        }
    }

    pub fn uniform(value: f32) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }

    pub fn horizontal(&self) -> f32 {
        self.left + self.right
    }

    pub fn vertical(&self) -> f32 {
        self.top + self.bottom
    }
}

impl Color {
    pub fn rgb_u8(r: u8, g: u8, b: u8) -> Self {
        Self {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
            a: 1.0,
        }
    }

    pub fn rgba_f32(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self {
            r: r.clamp(0.0, 1.0),
            g: g.clamp(0.0, 1.0),
            b: b.clamp(0.0, 1.0),
            a: a.clamp(0.0, 1.0),
        }
    }
}

pub fn compute_style(
    node: &HtmlNode,
    ancestors: &[&HtmlNode],
    sheet: &StyleSheet,
    parent: &ComputedStyle,
) -> ComputedStyle {
    let mut style = inherit(parent);
    apply_tag_defaults(node, &mut style);

    let mut matching = sheet
        .rules
        .iter()
        .filter(|rule| {
            rule.selectors
                .iter()
                .any(|selector| selector_matches(selector, node, ancestors))
        })
        .collect::<Vec<_>>();

    matching.sort_by_key(|rule| {
        let specificity = rule
            .selectors
            .iter()
            .filter(|selector| selector_matches(selector, node, ancestors))
            .map(|selector| selector_specificity(selector))
            .max()
            .unwrap_or(0);
        (specificity, rule.order)
    });

    for rule in matching {
        apply_declarations(&mut style, &rule.declarations);
    }

    if let Some(inline) = node.attr("style") {
        apply_declarations(&mut style, &parse_declarations(inline));
    }

    style.line_height_pt = style.line_height_pt.max(style.font_size_pt * 1.05);
    style
}

pub fn parse_length(raw: &str, font_size_pt: f32) -> Option<f32> {
    let value = raw.trim().trim_matches('"').trim_matches('\'');
    if value.eq_ignore_ascii_case("auto") {
        return None;
    }

    let number = value
        .chars()
        .take_while(|ch| ch.is_ascii_digit() || matches!(ch, '.' | '-' | '+'))
        .collect::<String>();
    let amount = number.parse::<f32>().ok()?;
    let unit = value[number.len()..].trim().to_ascii_lowercase();

    Some(match unit.as_str() {
        "" | "pt" => amount,
        "px" => amount * 0.75,
        "in" => amount * 72.0,
        "cm" => amount * 72.0 / 2.54,
        "mm" => amount * 72.0 / 25.4,
        "pc" => amount * 12.0,
        "em" => amount * font_size_pt,
        "rem" => amount * 10.5,
        "%" => return None,
        _ => amount,
    })
}

pub fn parse_percentage(raw: &str) -> Option<f32> {
    let value = raw.trim().trim_matches('"').trim_matches('\'');
    let percent = value.strip_suffix('%')?.trim().parse::<f32>().ok()?;
    Some((percent / 100.0).max(0.0))
}

fn inherit(parent: &ComputedStyle) -> ComputedStyle {
    ComputedStyle {
        display: Display::Block,
        font_family: parent.font_family.clone(),
        font_families: parent.font_families.clone(),
        font_size_pt: parent.font_size_pt,
        font_weight: parent.font_weight,
        font_style: parent.font_style,
        line_height_pt: parent.line_height_pt,
        color: parent.color,
        background_color: None,
        margin: Edges::zero(),
        padding: Edges::zero(),
        border_width_pt: 0.0,
        border_color: Color::rgb_u8(204, 204, 204),
        border_style: BorderStyle::Solid,
        border_collapse: parent.border_collapse,
        border_spacing_pt: parent.border_spacing_pt,
        width_pt: None,
        width_percent: None,
        text_align: parent.text_align,
        text_justify: parent.text_justify,
        white_space: parent.white_space,
        word_break: parent.word_break,
        overflow_wrap: parent.overflow_wrap,
        line_break: parent.line_break,
        hyphens: parent.hyphens,
        page_break_before: false,
        page_break_inside_avoid: false,
    }
}

fn apply_tag_defaults(node: &HtmlNode, style: &mut ComputedStyle) {
    let Some(tag) = node.tag_name() else {
        style.display = Display::Inline;
        return;
    };

    match tag {
        "script" | "style" | "head" | "title" | "meta" | "link" => style.display = Display::None,
        "strong" | "b" => {
            style.display = Display::Inline;
            style.font_weight = FontWeight::Bold;
        }
        "em" | "i" => {
            style.display = Display::Inline;
            style.font_style = FontStyle::Italic;
        }
        "span" | "a" => style.display = Display::Inline,
        "pre" => {
            style.white_space = WhiteSpace::Pre;
        }
        "h1" => {
            style.font_size_pt = 21.0;
            style.font_weight = FontWeight::Bold;
            style.line_height_pt = 30.0;
            style.margin.bottom = 9.0;
        }
        "h2" => {
            style.font_size_pt = 16.5;
            style.font_weight = FontWeight::Bold;
            style.line_height_pt = 24.0;
            style.margin.bottom = 7.5;
        }
        "h3" => {
            style.font_size_pt = 13.5;
            style.font_weight = FontWeight::Bold;
            style.line_height_pt = 20.0;
            style.margin.bottom = 6.0;
        }
        "p" => style.margin.bottom = 7.5,
        "li" => style.margin.bottom = 4.0,
        _ => {}
    }
}

fn apply_declarations(style: &mut ComputedStyle, declarations: &[Declaration]) {
    for declaration in declarations {
        let name = declaration.name.as_str();
        let value = declaration.value.trim();
        match name {
            "display" if value.eq_ignore_ascii_case("none") => style.display = Display::None,
            "display" if value.eq_ignore_ascii_case("inline") => style.display = Display::Inline,
            "display" => style.display = Display::Block,
            "font-family" => {
                let families = parse_font_family_list(value);
                if let Some(primary) = families.first() {
                    style.font_family = primary.clone();
                    style.font_families = families;
                }
            }
            "font-size" => {
                if let Some(length) = parse_length(value, style.font_size_pt) {
                    style.font_size_pt = length;
                }
            }
            "font-weight" => {
                style.font_weight = parse_font_weight_value(value);
            }
            "font-style" => {
                style.font_style = parse_font_style_value(value);
            }
            "line-height" => {
                style.line_height_pt =
                    parse_line_height(value, style.font_size_pt).unwrap_or(style.line_height_pt);
            }
            "color" => {
                if let Some(color) = parse_color(value) {
                    style.color = color;
                }
            }
            "background" | "background-color" => {
                style.background_color = parse_color(value);
            }
            "margin" => style.margin = parse_edges(value, style.font_size_pt),
            "margin-top" => {
                style.margin.top =
                    parse_length(value, style.font_size_pt).unwrap_or(style.margin.top)
            }
            "margin-right" => {
                style.margin.right =
                    parse_length(value, style.font_size_pt).unwrap_or(style.margin.right)
            }
            "margin-bottom" => {
                style.margin.bottom =
                    parse_length(value, style.font_size_pt).unwrap_or(style.margin.bottom)
            }
            "margin-left" => {
                style.margin.left =
                    parse_length(value, style.font_size_pt).unwrap_or(style.margin.left)
            }
            "padding" => style.padding = parse_edges(value, style.font_size_pt),
            "padding-top" => {
                style.padding.top =
                    parse_length(value, style.font_size_pt).unwrap_or(style.padding.top)
            }
            "padding-right" => {
                style.padding.right =
                    parse_length(value, style.font_size_pt).unwrap_or(style.padding.right)
            }
            "padding-bottom" => {
                style.padding.bottom =
                    parse_length(value, style.font_size_pt).unwrap_or(style.padding.bottom)
            }
            "padding-left" => {
                style.padding.left =
                    parse_length(value, style.font_size_pt).unwrap_or(style.padding.left)
            }
            "border" => parse_border(value, style),
            "border-width" => {
                style.border_width_pt = parse_border_width_value(value, style.font_size_pt)
                    .unwrap_or(style.border_width_pt)
            }
            "border-color" => style.border_color = parse_color(value).unwrap_or(style.border_color),
            "border-style" => {
                if let Some(border_style) = parse_border_style_value(value) {
                    style.border_style = border_style;
                }
            }
            "border-collapse" => {
                style.border_collapse = if value.eq_ignore_ascii_case("collapse") {
                    BorderCollapse::Collapse
                } else {
                    BorderCollapse::Separate
                };
            }
            "border-spacing" => {
                if let Some(spacing) = parse_border_spacing(value, style.font_size_pt) {
                    style.border_spacing_pt = spacing.max(0.0);
                }
            }
            "width" => {
                if let Some(percent) = parse_percentage(value) {
                    style.width_percent = Some(percent);
                    style.width_pt = None;
                } else {
                    style.width_pt = parse_length(value, style.font_size_pt);
                    style.width_percent = None;
                }
            }
            "text-align" => {
                style.text_align = match value.to_ascii_lowercase().as_str() {
                    "center" => TextAlign::Center,
                    "right" => TextAlign::Right,
                    "justify" => TextAlign::Justify,
                    _ => TextAlign::Left,
                }
            }
            "text-justify" => {
                style.text_justify = match value.to_ascii_lowercase().as_str() {
                    "inherit" => style.text_justify,
                    "initial" | "auto" => TextJustify::Auto,
                    "none" => TextJustify::None,
                    "inter-word" => TextJustify::InterWord,
                    "inter-character" => TextJustify::InterCharacter,
                    _ => TextJustify::Auto,
                }
            }
            "white-space" => {
                style.white_space = match value.to_ascii_lowercase().as_str() {
                    "inherit" => style.white_space,
                    "initial" | "normal" => WhiteSpace::Normal,
                    "nowrap" => WhiteSpace::NoWrap,
                    "pre" => WhiteSpace::Pre,
                    "pre-wrap" => WhiteSpace::PreWrap,
                    "pre-line" => WhiteSpace::PreLine,
                    _ => WhiteSpace::Normal,
                }
            }
            "word-break" => {
                style.word_break = match value.to_ascii_lowercase().as_str() {
                    "break-all" => WordBreak::BreakAll,
                    "keep-all" => WordBreak::KeepAll,
                    "break-word" => WordBreak::BreakWord,
                    _ => WordBreak::Normal,
                }
            }
            "overflow-wrap" | "word-wrap" => {
                style.overflow_wrap = match value.to_ascii_lowercase().as_str() {
                    "anywhere" => OverflowWrap::Anywhere,
                    "break-word" => OverflowWrap::BreakWord,
                    _ => OverflowWrap::Normal,
                }
            }
            "line-break" => {
                style.line_break = match value.to_ascii_lowercase().as_str() {
                    "loose" => LineBreak::Loose,
                    "normal" => LineBreak::Normal,
                    "strict" => LineBreak::Strict,
                    "anywhere" => LineBreak::Anywhere,
                    _ => LineBreak::Auto,
                }
            }
            "hyphens" => {
                style.hyphens = match value.to_ascii_lowercase().as_str() {
                    "none" => Hyphens::None,
                    "auto" => Hyphens::Auto,
                    _ => Hyphens::Manual,
                }
            }
            "page-break-before" | "break-before" => {
                style.page_break_before = value.contains("always") || value.contains("page");
            }
            "page-break-inside" | "break-inside" => {
                style.page_break_inside_avoid = value.contains("avoid");
            }
            _ => {}
        }
    }

    if matches!(style.border_style, BorderStyle::None | BorderStyle::Hidden) {
        style.border_width_pt = 0.0;
    }

    style.line_height_pt = style.line_height_pt.max(style.font_size_pt * 1.1);
}

fn strip_comments(css: &str) -> String {
    let mut output = String::new();
    let mut chars = css.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '/' && chars.peek() == Some(&'*') {
            chars.next();
            while let Some(inner) = chars.next() {
                if inner == '*' && chars.peek() == Some(&'/') {
                    chars.next();
                    break;
                }
            }
        } else {
            output.push(ch);
        }
    }
    output
}

fn split_css_rules(css: &str) -> Vec<(String, String)> {
    let mut rules = Vec::new();
    let mut selector = String::new();
    let mut body = String::new();
    let mut depth = 0usize;
    let mut in_body = false;

    for ch in css.chars() {
        match ch {
            '{' => {
                if depth == 0 {
                    in_body = true;
                } else if in_body {
                    body.push(ch);
                }
                depth += 1;
            }
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 && in_body {
                    rules.push((selector.trim().to_string(), body.trim().to_string()));
                    selector.clear();
                    body.clear();
                    in_body = false;
                } else if in_body {
                    body.push(ch);
                }
            }
            _ if in_body => body.push(ch),
            _ => selector.push(ch),
        }
    }

    rules
}

fn parse_declarations(body: &str) -> Vec<Declaration> {
    let mut declarations = Vec::new();
    for declaration in body.split(';') {
        let Some((name, value)) = declaration.split_once(':') else {
            continue;
        };
        declarations.push(Declaration {
            name: name.trim().to_ascii_lowercase(),
            value: value.trim().to_string(),
        });
    }
    declarations
}

fn declaration_value<'a>(declarations: &'a [Declaration], name: &str) -> Option<&'a str> {
    declarations
        .iter()
        .rev()
        .find(|declaration| declaration.name == name)
        .map(|declaration| declaration.value.as_str())
}

fn selector_matches(selector: &str, node: &HtmlNode, ancestors: &[&HtmlNode]) -> bool {
    let parts = selector
        .split_whitespace()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    if parts.is_empty() {
        return false;
    }

    if !compound_selector_matches(parts[parts.len() - 1], node) {
        return false;
    }

    if parts.len() == 1 {
        return true;
    }

    let mut ancestor_index = ancestors.len();
    for selector_part in parts[..parts.len() - 1].iter().rev() {
        let mut found = false;
        while ancestor_index > 0 {
            ancestor_index -= 1;
            if compound_selector_matches(selector_part, ancestors[ancestor_index]) {
                found = true;
                break;
            }
        }
        if !found {
            return false;
        }
    }

    true
}

fn compound_selector_matches(selector: &str, node: &HtmlNode) -> bool {
    if selector == "*" {
        return true;
    }

    let tag = node.tag_name().unwrap_or("");
    let mut wanted_tag = String::new();
    let mut classes = Vec::new();
    let mut id = None;
    let mut current = String::new();
    let mut mode = 't';

    for ch in selector.chars() {
        match ch {
            '.' | '#' => {
                flush_selector_part(mode, &current, &mut wanted_tag, &mut classes, &mut id);
                current.clear();
                mode = ch;
            }
            ':' => break,
            _ => current.push(ch),
        }
    }
    flush_selector_part(mode, &current, &mut wanted_tag, &mut classes, &mut id);

    if !wanted_tag.is_empty() && wanted_tag != tag {
        return false;
    }

    if let Some(id) = id {
        if node.attr("id") != Some(id.as_str()) {
            return false;
        }
    }

    classes
        .into_iter()
        .all(|class| node.classes().any(|candidate| candidate == class))
}

fn flush_selector_part(
    mode: char,
    value: &str,
    wanted_tag: &mut String,
    classes: &mut Vec<String>,
    id: &mut Option<String>,
) {
    let value = value.trim();
    if value.is_empty() {
        return;
    }
    match mode {
        '.' => classes.push(value.to_string()),
        '#' => *id = Some(value.to_string()),
        _ => *wanted_tag = value.to_ascii_lowercase(),
    }
}

fn selector_specificity(selector: &str) -> usize {
    let ids = selector.matches('#').count() * 100;
    let classes = selector.matches('.').count() * 10;
    let tags = selector
        .split_whitespace()
        .filter(|part| !part.starts_with('.') && !part.starts_with('#') && *part != "*")
        .count();
    ids + classes + tags
}

fn first_font_family(value: &str) -> String {
    parse_font_family_list(value)
        .into_iter()
        .next()
        .unwrap_or_else(|| {
            value
                .trim()
                .trim_matches('"')
                .trim_matches('\'')
                .to_string()
        })
}

pub fn parse_font_family_list(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(|part| part.trim().trim_matches('"').trim_matches('\'').to_string())
        .filter(|part| !part.is_empty())
        .filter(|part| {
            !matches!(
                part.to_ascii_lowercase().as_str(),
                "serif" | "sans-serif" | "monospace" | "system-ui" | "fantasy" | "cursive"
            )
        })
        .collect::<Vec<_>>()
}

fn parse_font_weight_value(value: &str) -> FontWeight {
    let lower = value.trim().to_ascii_lowercase();
    if lower.contains("bold") || lower.parse::<u16>().unwrap_or(400) >= 600 {
        FontWeight::Bold
    } else {
        FontWeight::Regular
    }
}

fn parse_font_style_value(value: &str) -> FontStyle {
    if value.trim().to_ascii_lowercase().contains("italic") {
        FontStyle::Italic
    } else {
        FontStyle::Normal
    }
}

fn parse_font_face_rule(declarations: &[Declaration]) -> std::result::Result<FontFaceRule, String> {
    let font_family = declaration_value(declarations, "font-family")
        .map(|value| first_font_family(value))
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "@font-face is missing font-family".to_string())?;

    let src = declaration_value(declarations, "src")
        .ok_or_else(|| format!("@font-face for '{font_family}' is missing src"))?;
    let sources = parse_font_src_urls(src);
    if sources.is_empty() {
        return Err(format!(
            "@font-face for '{font_family}' has no url(...) source"
        ));
    }

    let font_weight = declaration_value(declarations, "font-weight")
        .map(|value| parse_font_weight_value(value))
        .unwrap_or(FontWeight::Regular);
    let font_style = declaration_value(declarations, "font-style")
        .map(|value| parse_font_style_value(value))
        .unwrap_or(FontStyle::Normal);

    Ok(FontFaceRule {
        font_family,
        sources,
        font_weight,
        font_style,
    })
}

fn parse_font_src_urls(value: &str) -> Vec<FontFaceSource> {
    let mut sources = Vec::new();
    let mut remaining = value;
    while let Some(start) = remaining.find("url(") {
        let after = &remaining[start + 4..];
        let Some(end) = after.find(')') else {
            break;
        };
        let inside = after[..end].trim().trim_matches('"').trim_matches('\'');
        let rest_after_url = &after[end + 1..];

        if !inside.is_empty() {
            let format_hint = parse_format_hint(rest_after_url);
            sources.push(FontFaceSource {
                url: inside.to_string(),
                format_hint,
            });
        }
        remaining = rest_after_url;
    }
    sources
}

fn parse_format_hint(input: &str) -> Option<String> {
    let segment = input.split(',').next().unwrap_or(input);
    let start = segment.find("format(")?;
    let after = &segment[start + 7..];
    let end = after.find(')')?;
    let value = after[..end].trim().trim_matches('"').trim_matches('\'');
    (!value.is_empty()).then(|| value.to_ascii_lowercase())
}

fn parse_line_height(value: &str, font_size_pt: f32) -> Option<f32> {
    let value = value.trim();
    if let Ok(multiplier) = value.parse::<f32>() {
        return Some(multiplier * font_size_pt);
    }
    parse_length(value, font_size_pt)
}

fn parse_border_width_keyword(value: &str) -> Option<f32> {
    match value.trim().to_ascii_lowercase().as_str() {
        "thin" => Some(0.75),
        "medium" => Some(1.5),
        "thick" => Some(2.25),
        _ => None,
    }
}

fn parse_border_width_value(value: &str, font_size_pt: f32) -> Option<f32> {
    let first = value.split_whitespace().next().unwrap_or(value);
    parse_length(first, font_size_pt).or_else(|| parse_border_width_keyword(first))
}

fn parse_border_style_keyword(value: &str) -> Option<BorderStyle> {
    match value.trim().to_ascii_lowercase().as_str() {
        "none" => Some(BorderStyle::None),
        "hidden" => Some(BorderStyle::Hidden),
        "dotted" => Some(BorderStyle::Dotted),
        "dashed" => Some(BorderStyle::Dashed),
        "solid" => Some(BorderStyle::Solid),
        "double" => Some(BorderStyle::Double),
        "groove" => Some(BorderStyle::Groove),
        "ridge" => Some(BorderStyle::Ridge),
        "inset" => Some(BorderStyle::Inset),
        "outset" => Some(BorderStyle::Outset),
        _ => None,
    }
}

fn parse_border_style_value(value: &str) -> Option<BorderStyle> {
    value
        .split_whitespace()
        .find_map(parse_border_style_keyword)
}

fn parse_border_spacing(value: &str, font_size_pt: f32) -> Option<f32> {
    let first = value.split_whitespace().next()?;
    parse_length(first, font_size_pt)
}

fn parse_edges(value: &str, font_size_pt: f32) -> Edges {
    let parts = value.split_whitespace().collect::<Vec<_>>();
    let lengths = parts
        .iter()
        .filter_map(|part| parse_length(part, font_size_pt))
        .collect::<Vec<_>>();

    match lengths.as_slice() {
        [all] => Edges::uniform(*all),
        [vertical, horizontal] => Edges {
            top: *vertical,
            right: *horizontal,
            bottom: *vertical,
            left: *horizontal,
        },
        [top, horizontal, bottom] => Edges {
            top: *top,
            right: *horizontal,
            bottom: *bottom,
            left: *horizontal,
        },
        [top, right, bottom, left, ..] => Edges {
            top: *top,
            right: *right,
            bottom: *bottom,
            left: *left,
        },
        _ => Edges::zero(),
    }
}

fn parse_border(value: &str, style: &mut ComputedStyle) {
    let mut saw_style = false;
    let mut saw_width = false;

    for part in value.split_whitespace() {
        if let Some(length) = parse_border_width_value(part, style.font_size_pt) {
            style.border_width_pt = length;
            saw_width = true;
        } else if let Some(color) = parse_color(part) {
            style.border_color = color;
        } else if let Some(border_style) = parse_border_style_keyword(part) {
            style.border_style = border_style;
            saw_style = true;
        }
    }

    if matches!(style.border_style, BorderStyle::None | BorderStyle::Hidden) {
        style.border_width_pt = 0.0;
        return;
    }

    if style.border_width_pt == 0.0 && (saw_style || !saw_width) {
        style.border_width_pt = 0.75;
    }
}

fn parse_page_size(value: &str) -> Option<(f32, f32)> {
    let lower = value.trim().to_ascii_lowercase();
    let landscape = lower.contains("landscape");
    let portrait = lower.contains("portrait");
    let without_orientation = lower
        .replace("landscape", "")
        .replace("portrait", "")
        .trim()
        .to_string();

    let mut size = match without_orientation.as_str() {
        "a4" | "" => (595.2756, 841.8898),
        "letter" => (612.0, 792.0),
        "legal" => (612.0, 1008.0),
        other => {
            let parts = other.split_whitespace().collect::<Vec<_>>();
            if parts.len() >= 2 {
                (parse_length(parts[0], 10.5)?, parse_length(parts[1], 10.5)?)
            } else {
                return None;
            }
        }
    };

    if landscape && size.1 > size.0 || portrait && size.0 > size.1 {
        size = (size.1, size.0);
    }

    Some(size)
}

pub fn parse_color(value: &str) -> Option<Color> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("transparent") {
        return None;
    }

    if let Some(color) = parse_hex_color(value) {
        return Some(color);
    }
    if let Some(color) = parse_rgb_function(value) {
        return Some(color);
    }
    if let Some(color) = parse_hsl_function(value) {
        return Some(color);
    }

    parse_named_color(value)
}

fn parse_hex_color(value: &str) -> Option<Color> {
    let hex = value.strip_prefix('#')?;
    match hex.len() {
        3 => {
            let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
            let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
            let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
            Some(Color::rgb_u8(r, g, b))
        }
        4 => {
            let r = u8::from_str_radix(&hex[0..1].repeat(2), 16).ok()?;
            let g = u8::from_str_radix(&hex[1..2].repeat(2), 16).ok()?;
            let b = u8::from_str_radix(&hex[2..3].repeat(2), 16).ok()?;
            let a = u8::from_str_radix(&hex[3..4].repeat(2), 16).ok()?;
            Some(Color::rgba_f32(
                r as f32 / 255.0,
                g as f32 / 255.0,
                b as f32 / 255.0,
                a as f32 / 255.0,
            ))
        }
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            Some(Color::rgb_u8(r, g, b))
        }
        8 => {
            let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
            let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
            let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
            let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
            Some(Color::rgba_f32(
                r as f32 / 255.0,
                g as f32 / 255.0,
                b as f32 / 255.0,
                a as f32 / 255.0,
            ))
        }
        _ => None,
    }
}

fn parse_rgb_function(value: &str) -> Option<Color> {
    let lower = value.trim().to_ascii_lowercase();
    if !(lower.starts_with("rgb(") || lower.starts_with("rgba(")) {
        return None;
    }

    let start = value.find('(')? + 1;
    let end = value.rfind(')')?;
    if end <= start {
        return None;
    }

    let (parts, alpha_raw) = split_color_channels(&value[start..end])?;
    if parts.len() < 3 {
        return None;
    }

    let r = parse_rgb_channel(parts[0])?;
    let g = parse_rgb_channel(parts[1])?;
    let b = parse_rgb_channel(parts[2])?;
    let a = alpha_raw
        .and_then(parse_alpha_channel)
        .unwrap_or(1.0)
        .clamp(0.0, 1.0);
    Some(Color::rgba_f32(r, g, b, a))
}

fn parse_hsl_function(value: &str) -> Option<Color> {
    let lower = value.trim().to_ascii_lowercase();
    if !(lower.starts_with("hsl(") || lower.starts_with("hsla(")) {
        return None;
    }

    let start = value.find('(')? + 1;
    let end = value.rfind(')')?;
    if end <= start {
        return None;
    }

    let (parts, alpha_raw) = split_color_channels(&value[start..end])?;
    if parts.len() < 3 {
        return None;
    }

    let h = parse_hue_degrees(parts[0])?;
    let s = parse_hsl_percent(parts[1])?;
    let l = parse_hsl_percent(parts[2])?;
    let a = alpha_raw.and_then(parse_alpha_channel).unwrap_or(1.0);
    let (r, g, b) = hsl_to_rgb(h, s, l);
    Some(Color::rgba_f32(r, g, b, a))
}

fn parse_named_color(value: &str) -> Option<Color> {
    match value.to_ascii_lowercase().as_str() {
        "black" => Some(Color::rgb_u8(0, 0, 0)),
        "white" => Some(Color::rgb_u8(255, 255, 255)),
        "red" => Some(Color::rgb_u8(255, 0, 0)),
        "green" => Some(Color::rgb_u8(0, 128, 0)),
        "blue" => Some(Color::rgb_u8(0, 0, 255)),
        "gray" | "grey" => Some(Color::rgb_u8(128, 128, 128)),
        "silver" => Some(Color::rgb_u8(192, 192, 192)),
        "maroon" => Some(Color::rgb_u8(128, 0, 0)),
        "purple" => Some(Color::rgb_u8(128, 0, 128)),
        "fuchsia" | "magenta" => Some(Color::rgb_u8(255, 0, 255)),
        "lime" => Some(Color::rgb_u8(0, 255, 0)),
        "olive" => Some(Color::rgb_u8(128, 128, 0)),
        "yellow" => Some(Color::rgb_u8(255, 255, 0)),
        "navy" => Some(Color::rgb_u8(0, 0, 128)),
        "teal" => Some(Color::rgb_u8(0, 128, 128)),
        "aqua" | "cyan" => Some(Color::rgb_u8(0, 255, 255)),
        "orange" => Some(Color::rgb_u8(255, 165, 0)),
        "brown" => Some(Color::rgb_u8(165, 42, 42)),
        "pink" => Some(Color::rgb_u8(255, 192, 203)),
        "gold" => Some(Color::rgb_u8(255, 215, 0)),
        "indigo" => Some(Color::rgb_u8(75, 0, 130)),
        "violet" => Some(Color::rgb_u8(238, 130, 238)),
        "coral" => Some(Color::rgb_u8(255, 127, 80)),
        "salmon" => Some(Color::rgb_u8(250, 128, 114)),
        "tomato" => Some(Color::rgb_u8(255, 99, 71)),
        "crimson" => Some(Color::rgb_u8(220, 20, 60)),
        "rebeccapurple" => Some(Color::rgb_u8(102, 51, 153)),
        "aliceblue" => Some(Color::rgb_u8(240, 248, 255)),
        _ => None,
    }
}

fn parse_alpha_channel(raw: &str) -> Option<f32> {
    let value = raw.trim();
    if let Some(percent) = value.strip_suffix('%') {
        let parsed = percent.trim().parse::<f32>().ok()?;
        return Some((parsed / 100.0).clamp(0.0, 1.0));
    }
    let parsed = value.parse::<f32>().ok()?;
    Some(parsed.clamp(0.0, 1.0))
}

fn parse_rgb_channel(raw: &str) -> Option<f32> {
    let value = raw.trim();
    if let Some(percent) = value.strip_suffix('%') {
        let parsed = percent.trim().parse::<f32>().ok()?;
        return Some((parsed / 100.0).clamp(0.0, 1.0));
    }
    let parsed = value.parse::<f32>().ok()?;
    Some((parsed / 255.0).clamp(0.0, 1.0))
}

fn split_color_channels(raw: &str) -> Option<(Vec<&str>, Option<&str>)> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let (main, alpha_from_slash) = if let Some((lhs, rhs)) = trimmed.split_once('/') {
        (lhs.trim(), Some(rhs.trim()))
    } else {
        (trimmed, None)
    };

    let mut channels = if main.contains(',') {
        main.split(',').map(str::trim).collect::<Vec<_>>()
    } else {
        main.split_whitespace().collect::<Vec<_>>()
    };

    if channels.len() < 3 {
        return None;
    }

    let alpha = if alpha_from_slash.is_some() {
        alpha_from_slash
    } else if channels.len() > 3 {
        channels.pop()
    } else {
        None
    };

    Some((channels, alpha))
}

fn parse_hue_degrees(raw: &str) -> Option<f32> {
    let value = raw.trim().to_ascii_lowercase();
    let degrees = if let Some(value) = value.strip_suffix("deg") {
        value.trim().parse::<f32>().ok()?
    } else if let Some(value) = value.strip_suffix("grad") {
        value.trim().parse::<f32>().ok()? * 0.9
    } else if let Some(value) = value.strip_suffix("rad") {
        value.trim().parse::<f32>().ok()? * (180.0 / std::f32::consts::PI)
    } else if let Some(value) = value.strip_suffix("turn") {
        value.trim().parse::<f32>().ok()? * 360.0
    } else {
        value.parse::<f32>().ok()?
    };
    Some(degrees.rem_euclid(360.0))
}

fn parse_hsl_percent(raw: &str) -> Option<f32> {
    let value = raw.trim();
    if let Some(percent) = value.strip_suffix('%') {
        let parsed = percent.trim().parse::<f32>().ok()?;
        return Some((parsed / 100.0).clamp(0.0, 1.0));
    }
    let parsed = value.parse::<f32>().ok()?;
    Some(parsed.clamp(0.0, 1.0))
}

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (f32, f32, f32) {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let h_prime = (h / 60.0).rem_euclid(6.0);
    let x = c * (1.0 - ((h_prime.rem_euclid(2.0)) - 1.0).abs());

    let (r1, g1, b1) = if h_prime < 1.0 {
        (c, x, 0.0)
    } else if h_prime < 2.0 {
        (x, c, 0.0)
    } else if h_prime < 3.0 {
        (0.0, c, x)
    } else if h_prime < 4.0 {
        (0.0, x, c)
    } else if h_prime < 5.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };

    let m = l - c * 0.5;
    (r1 + m, g1 + m, b1 + m)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_px_to_points() {
        assert_eq!(parse_length("16px", 12.0), Some(12.0));
    }

    #[test]
    fn parses_hex_color() {
        let color = parse_color("#0f3460").unwrap();
        assert!((color.r - 15.0 / 255.0).abs() < 0.001);
    }

    #[test]
    fn parses_font_face_url() {
        let css = "@font-face { font-family: 'Sarabun'; src: url('../fonts/Sarabun-Regular.ttf') format('truetype'); font-weight: 400; font-style: normal; }";
        let stylesheet = StyleSheet::parse(css);
        assert_eq!(stylesheet.font_faces.len(), 1);
        assert_eq!(stylesheet.font_faces[0].font_family, "Sarabun");
        assert_eq!(stylesheet.font_faces[0].sources.len(), 1);
        assert_eq!(
            stylesheet.font_faces[0].sources[0].url,
            "../fonts/Sarabun-Regular.ttf"
        );
        assert_eq!(
            stylesheet.font_faces[0].sources[0].format_hint.as_deref(),
            Some("truetype")
        );
    }

    #[test]
    fn parses_font_face_multiple_url_sources() {
        let css = "@font-face { font-family: 'BrandSans'; src: local('BrandSans'), url('./BrandSans.woff2') format('woff2'), url('./BrandSans.ttf') format('truetype'); }";
        let stylesheet = StyleSheet::parse(css);
        assert_eq!(stylesheet.font_faces.len(), 1);
        assert_eq!(stylesheet.font_faces[0].sources.len(), 2);
        assert_eq!(stylesheet.font_faces[0].sources[0].url, "./BrandSans.woff2");
        assert_eq!(stylesheet.font_faces[0].sources[1].url, "./BrandSans.ttf");
    }

    #[test]
    fn parses_font_family_stack() {
        let families = parse_font_family_list("'Custom Thai', 'Sarabun', sans-serif");
        assert_eq!(families, vec!["Custom Thai", "Sarabun"]);
    }

    #[test]
    fn root_style_keeps_explicit_default_family() {
        let root = ComputedStyle::root("sans-serif", 10.5);
        assert_eq!(root.font_family, "sans-serif");
        assert_eq!(root.font_families, vec!["sans-serif"]);
    }

    #[test]
    fn parses_percentage_value() {
        assert_eq!(parse_percentage("100%"), Some(1.0));
        assert_eq!(parse_percentage("50%"), Some(0.5));
        assert_eq!(parse_percentage("12.5%"), Some(0.125));
    }

    #[test]
    fn parse_length_ignores_percent_unit() {
        assert_eq!(parse_length("100%", 12.0), None);
    }

    #[test]
    fn parses_page_size_landscape_keyword() {
        let (w, h) = parse_page_size("A4 landscape").unwrap();
        assert!(w > h);
        assert!((w - 841.8898).abs() < 0.01);
        assert!((h - 595.2756).abs() < 0.01);
    }

    #[test]
    fn parses_page_size_portrait_keyword() {
        let (w, h) = parse_page_size("A4 portrait").unwrap();
        assert!(h > w);
        assert!((w - 595.2756).abs() < 0.01);
        assert!((h - 841.8898).abs() < 0.01);
    }

    #[test]
    fn stylesheet_page_size_allows_inline_landscape_override() {
        let css = r#"
            @page {
                size: A4;
                margin: 18mm 14mm;
                @top-left { content: "LynPDF"; }
                @bottom-right { content: "Page " counter(page); }
            }
            @page :first {
                margin-top: 24mm;
                @top-left { content: ""; }
            }
            @page {
                size: A4 landscape;
                margin: 12mm;
                @top-left { content: ""; }
            }
        "#;
        let stylesheet = StyleSheet::parse(css);
        let width = stylesheet.page.width_pt.unwrap();
        let height = stylesheet.page.height_pt.unwrap();
        assert!(width > height);
        assert!((width - 841.8898).abs() < 0.01);
        assert!((height - 595.2756).abs() < 0.01);
    }

    #[test]
    fn parses_text_justify_inter_character() {
        let css = "p { text-align: justify; text-justify: inter-character; }";
        let stylesheet = StyleSheet::parse(css);
        let paragraph = HtmlNode {
            kind: crate::html::HtmlNodeKind::Element("p".to_string()),
            attrs: std::collections::HashMap::new(),
            children: Vec::new(),
        };
        let parent = ComputedStyle::root("Sarabun", 10.5);
        let computed = compute_style(&paragraph, &[], &stylesheet, &parent);
        assert_eq!(computed.text_align, TextAlign::Justify);
        assert_eq!(computed.text_justify, TextJustify::InterCharacter);
    }

    #[test]
    fn parses_line_breaking_properties() {
        let css = "p { white-space: pre-wrap; word-break: break-all; overflow-wrap: anywhere; line-break: strict; hyphens: auto; }";
        let stylesheet = StyleSheet::parse(css);
        let paragraph = HtmlNode {
            kind: crate::html::HtmlNodeKind::Element("p".to_string()),
            attrs: std::collections::HashMap::new(),
            children: Vec::new(),
        };
        let parent = ComputedStyle::root("Sarabun", 10.5);
        let computed = compute_style(&paragraph, &[], &stylesheet, &parent);
        assert_eq!(computed.white_space, WhiteSpace::PreWrap);
        assert_eq!(computed.word_break, WordBreak::BreakAll);
        assert_eq!(computed.overflow_wrap, OverflowWrap::Anywhere);
        assert_eq!(computed.line_break, LineBreak::Strict);
        assert_eq!(computed.hyphens, Hyphens::Auto);
    }

    #[test]
    fn white_space_inherit_preserves_parent_value() {
        let css = "code { white-space: inherit; }";
        let stylesheet = StyleSheet::parse(css);
        let code = HtmlNode {
            kind: crate::html::HtmlNodeKind::Element("code".to_string()),
            attrs: std::collections::HashMap::new(),
            children: Vec::new(),
        };
        let mut parent = ComputedStyle::root("Sarabun", 10.5);
        parent.white_space = WhiteSpace::PreWrap;
        let computed = compute_style(&code, &[], &stylesheet, &parent);
        assert_eq!(computed.white_space, WhiteSpace::PreWrap);
    }

    #[test]
    fn parses_border_collapse_and_spacing() {
        let css = "table { border-collapse: collapse; border-spacing: 6px; }";
        let stylesheet = StyleSheet::parse(css);
        let table = HtmlNode {
            kind: crate::html::HtmlNodeKind::Element("table".to_string()),
            attrs: std::collections::HashMap::new(),
            children: Vec::new(),
        };
        let parent = ComputedStyle::root("Sarabun", 10.5);
        let computed = compute_style(&table, &[], &stylesheet, &parent);
        assert_eq!(computed.border_collapse, BorderCollapse::Collapse);
        assert!((computed.border_spacing_pt - 4.5).abs() < 0.001);
    }

    #[test]
    fn border_style_none_forces_zero_width() {
        let css = "td { border-width: 3px; border-style: none; border-color: #f00; }";
        let stylesheet = StyleSheet::parse(css);
        let cell = HtmlNode {
            kind: crate::html::HtmlNodeKind::Element("td".to_string()),
            attrs: std::collections::HashMap::new(),
            children: Vec::new(),
        };
        let parent = ComputedStyle::root("Sarabun", 10.5);
        let computed = compute_style(&cell, &[], &stylesheet, &parent);
        assert_eq!(computed.border_style, BorderStyle::None);
        assert_eq!(computed.border_width_pt, 0.0);
    }

    #[test]
    fn parses_rgb_and_rgba_color_functions() {
        let rgb = parse_color("rgb(255, 128, 0)").unwrap();
        assert!((rgb.r - 1.0).abs() < 0.001);
        assert!((rgb.g - (128.0 / 255.0)).abs() < 0.001);
        assert!((rgb.b - 0.0).abs() < 0.001);

        let rgba = parse_color("rgba(64, 32, 16, 0.3)").unwrap();
        assert!((rgba.r - (64.0 / 255.0)).abs() < 0.001);
        assert!((rgba.g - (32.0 / 255.0)).abs() < 0.001);
        assert!((rgba.b - (16.0 / 255.0)).abs() < 0.001);
        assert!((rgba.a - 0.3).abs() < 0.001);

        let modern = parse_color("rgb(20 40 60 / 75%)").unwrap();
        assert!((modern.r - (20.0 / 255.0)).abs() < 0.001);
        assert!((modern.g - (40.0 / 255.0)).abs() < 0.001);
        assert!((modern.b - (60.0 / 255.0)).abs() < 0.001);
        assert!((modern.a - 0.75).abs() < 0.001);
    }

    #[test]
    fn parses_hsl_and_hex_alpha_colors() {
        let hsl = parse_color("hsl(210 100% 56% / 0.5)").unwrap();
        assert!((hsl.a - 0.5).abs() < 0.001);

        let hex8 = parse_color("#336699cc").unwrap();
        assert!((hex8.r - (51.0 / 255.0)).abs() < 0.001);
        assert!((hex8.g - (102.0 / 255.0)).abs() < 0.001);
        assert!((hex8.b - (153.0 / 255.0)).abs() < 0.001);
        assert!((hex8.a - (204.0 / 255.0)).abs() < 0.001);
    }
}
