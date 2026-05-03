use crate::error::{LynPdfError, Result};
use crate::fonts::{FontKey, FontRegistry, LoadedFont};
use crate::html::{
    collect_links, collect_text, collect_text_preserving_whitespace, is_block_tag,
    is_text_container, HtmlLink, HtmlNode, HtmlNodeKind,
};
use crate::style::{
    compute_style, parse_color, parse_length, parse_percentage, BorderCollapse, BorderStyle, Color,
    ComputedStyle, Display, StyleSheet, TextAlign, WhiteSpace,
};
use crate::text::{shape_lines, PositionedGlyph, TextLine};
use crate::types::{PageSize, RenderOptions};
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use ttf_parser::{Face, GlyphId, RasterImageFormat};

const TABLE_MIN_COL_WIDTH_PT: f32 = 28.0;
const TABLE_ABSOLUTE_MIN_COL_WIDTH_PT: f32 = 8.0;
const TABLE_MEASURE_MAX_WIDTH_PT: f32 = 50_000.0;
const COLLAPSED_BORDER_EPSILON_PT: f32 = 0.01;
const COLLAPSED_BORDER_AXIS_SCALE: f32 = 1000.0;
const BORDER_SOURCE_TABLE: u8 = 1;
const BORDER_SOURCE_CELL: u8 = 2;

#[derive(Debug, Clone)]
pub struct LayoutDocument {
    pub page_size: PageSize,
    pub output_page_size: PageSize,
    pub content_scale: f32,
    pub margin_pt: f32,
    pub pages: Vec<LayoutPage>,
}

#[derive(Debug, Clone)]
pub struct LayoutPage {
    pub ops: Vec<PaintOp>,
}

#[derive(Debug, Clone)]
pub enum PaintOp {
    Rect {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        fill: Option<Color>,
        stroke: Option<Color>,
        stroke_width: f32,
    },
    Glyph {
        font_key: FontKey,
        glyph_id: u16,
        cluster_text: String,
        font_size_pt: f32,
        color: Color,
        x: f32,
        baseline_y: f32,
        y_offset_pt: f32,
    },
    Line {
        x1: f32,
        y1: f32,
        x2: f32,
        y2: f32,
        color: Color,
        width: f32,
        border_style: BorderStyle,
        edge: PaintLineEdge,
    },
    Link {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        target: LinkTarget,
    },
    Anchor {
        name: String,
        y: f32,
    },
    Image {
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        pixel_width: u32,
        pixel_height: u32,
        rgb_data: Vec<u8>,
        alpha_data: Option<Vec<u8>>,
    },
    SvgPath {
        commands: Vec<SvgPathCommand>,
        fill: Option<SvgPaint>,
        stroke: Option<SvgPaint>,
        fill_opacity: f32,
        stroke_opacity: f32,
        stroke_width: f32,
        line_cap: SvgLineCap,
        line_join: SvgLineJoin,
    },
}

#[derive(Debug, Clone)]
pub enum SvgPaint {
    Solid(Color),
    LinearGradient(SvgLinearGradientPaint),
}

#[derive(Debug, Clone)]
pub struct SvgLinearGradientPaint {
    pub x1: f32,
    pub y1: f32,
    pub x2: f32,
    pub y2: f32,
    pub stops: Vec<SvgGradientStop>,
}

#[derive(Debug, Clone)]
pub struct SvgGradientStop {
    pub offset: f32,
    pub color: Color,
    pub opacity: f32,
}

#[derive(Debug, Clone)]
pub enum SvgPathCommand {
    MoveTo {
        x: f32,
        y: f32,
    },
    LineTo {
        x: f32,
        y: f32,
    },
    CurveTo {
        cx1: f32,
        cy1: f32,
        cx2: f32,
        cy2: f32,
        x: f32,
        y: f32,
    },
    ClosePath,
}

#[derive(Debug, Clone, Copy)]
pub enum SvgLineCap {
    Butt,
    Round,
    Square,
}

#[derive(Debug, Clone, Copy)]
pub enum SvgLineJoin {
    Miter,
    Round,
    Bevel,
}

#[derive(Debug, Clone)]
pub enum LinkTarget {
    External(String),
    Internal(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaintLineEdge {
    Top,
    Right,
    Bottom,
    Left,
}

#[derive(Debug, Clone)]
struct TableCellRef<'a> {
    node: &'a HtmlNode,
    colspan: usize,
    rowspan: usize,
}

#[derive(Debug, Clone)]
struct TableCellPlacement {
    cell_index: usize,
    col_start: usize,
    colspan: usize,
    rowspan: usize,
}

#[derive(Debug, Clone)]
struct TableRowLayout {
    placements: Vec<TableCellPlacement>,
}

#[derive(Debug, Clone)]
struct TableRowRef<'a> {
    node: &'a HtmlNode,
    ancestors: Vec<&'a HtmlNode>,
    cells: Vec<TableCellRef<'a>>,
}

#[derive(Debug, Clone, Default)]
struct TableSections<'a> {
    header: Vec<TableRowRef<'a>>,
    body: Vec<TableRowRef<'a>>,
    footer: Vec<TableRowRef<'a>>,
}

#[derive(Debug, Clone)]
struct MeasuredTableCell {
    style: ComputedStyle,
    col_start: usize,
    colspan: usize,
    rowspan: usize,
    lines: Vec<TextLine>,
    line_layouts: Vec<LineLayoutMetrics>,
    required_height: f32,
    render_height: f32,
}

#[derive(Debug, Clone)]
struct MeasuredTableRow {
    cells: Vec<MeasuredTableCell>,
    height: f32,
    min_block_height: f32,
}

#[derive(Debug, Clone)]
struct SvgRenderContext {
    view_x: f32,
    view_y: f32,
    view_width: f32,
    view_height: f32,
    transform: SvgTransform,
    inherited_opacity: f32,
    gradients: HashMap<String, SvgLinearGradientDef>,
    style: ComputedStyle,
}

#[derive(Debug, Clone)]
struct SvgLinearGradientDef {
    x1: SvgLengthValue,
    y1: SvgLengthValue,
    x2: SvgLengthValue,
    y2: SvgLengthValue,
    units: SvgGradientUnits,
    gradient_transform: SvgTransform,
    stops: Vec<SvgGradientStop>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SvgGradientUnits {
    ObjectBoundingBox,
    UserSpaceOnUse,
}

#[derive(Debug, Clone, Copy)]
enum SvgLengthValue {
    Number(f32),
    Percent(f32),
}

#[derive(Debug, Clone, Copy)]
struct SvgTransform {
    a: f32,
    b: f32,
    c: f32,
    d: f32,
    e: f32,
    f: f32,
}

#[derive(Debug, Clone)]
struct TextLinkSpan {
    start: usize,
    end: usize,
    target: LinkTarget,
}

#[derive(Debug, Clone, Copy)]
struct TextColorSpan {
    start: usize,
    end: usize,
    color: Color,
}

#[derive(Debug, Clone)]
struct LineClusterBounds {
    start: usize,
    end: usize,
    x_start_pt: f32,
    x_end_pt: f32,
}

#[derive(Debug, Clone, Copy)]
struct LineLayoutMetrics {
    ascent_pt: f32,
    height_pt: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum BorderEdgeOrientation {
    Vertical,
    Horizontal,
}

#[derive(Debug, Clone, Copy)]
struct BorderEdgeCandidate {
    orientation: BorderEdgeOrientation,
    axis_pt: f32,
    start_pt: f32,
    end_pt: f32,
    width_pt: f32,
    color: Color,
    border_style: BorderStyle,
    edge: PaintLineEdge,
    source_priority: u8,
    row_anchor: usize,
    col_anchor: usize,
}

pub fn layout_document(
    body: &HtmlNode,
    sheet: &StyleSheet,
    fonts: &FontRegistry,
    options: &RenderOptions,
    base_dir: &Path,
) -> Result<LayoutDocument> {
    let output_page_size = PageSize {
        width_pt: sheet.page.width_pt.unwrap_or(options.page_size.width_pt),
        height_pt: sheet.page.height_pt.unwrap_or(options.page_size.height_pt),
    };
    let content_scale = options.page_scale_factor();
    let page_size = PageSize {
        width_pt: output_page_size.width_pt / content_scale,
        height_pt: output_page_size.height_pt / content_scale,
    };
    let margin_pt = sheet.page.margin_pt.unwrap_or(options.margin_pt) / content_scale;
    let root_style =
        ComputedStyle::root(&options.default_font_family, options.default_font_size_pt);
    let mut context = LayoutContext {
        document: LayoutDocument {
            page_size,
            output_page_size,
            content_scale,
            margin_pt,
            pages: vec![LayoutPage { ops: Vec::new() }],
        },
        cursor_y: margin_pt,
        fonts,
        sheet,
        options,
        base_dir,
        root_style,
    };

    let mut ancestors = Vec::new();
    let root_style = context.root_style.clone();
    context.layout_children(
        body,
        &mut ancestors,
        &root_style,
        margin_pt,
        page_size.width_pt - margin_pt * 2.0,
    )?;
    Ok(context.document)
}

struct LayoutContext<'a> {
    document: LayoutDocument,
    cursor_y: f32,
    fonts: &'a FontRegistry,
    sheet: &'a StyleSheet,
    options: &'a RenderOptions,
    base_dir: &'a Path,
    root_style: ComputedStyle,
}

impl<'a> LayoutContext<'a> {
    fn layout_children(
        &mut self,
        node: &'a HtmlNode,
        ancestors: &mut Vec<&'a HtmlNode>,
        parent_style: &ComputedStyle,
        x: f32,
        width: f32,
    ) -> Result<()> {
        ancestors.push(node);
        for child in &node.children {
            self.layout_node(child, ancestors, parent_style, x, width)?;
        }
        ancestors.pop();
        Ok(())
    }

    fn layout_node(
        &mut self,
        node: &'a HtmlNode,
        ancestors: &mut Vec<&'a HtmlNode>,
        parent_style: &ComputedStyle,
        x: f32,
        width: f32,
    ) -> Result<()> {
        if matches!(node.kind, HtmlNodeKind::Text(_)) {
            return Ok(());
        }

        let style = compute_style(node, ancestors, self.sheet, parent_style);
        if style.display == Display::None {
            return Ok(());
        }

        let Some(tag) = node.tag_name() else {
            return Ok(());
        };
        let anchor_name = node
            .attr("id")
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string);

        if tag == "br" {
            self.cursor_y += style.line_height_pt;
            return Ok(());
        }

        if style.page_break_before && self.cursor_y > self.document.margin_pt + 1.0 {
            self.new_page();
        }

        if tag == "table" {
            if let Some(anchor_name) = anchor_name.as_ref() {
                let anchor_y = self.cursor_y + style.margin.top;
                self.current_ops().push(PaintOp::Anchor {
                    name: anchor_name.clone(),
                    y: anchor_y,
                });
            }
            self.layout_table(node, ancestors, &style, x, width)?;
            return Ok(());
        }

        if tag == "img" {
            self.layout_image_node(node, &style, x, width, anchor_name.as_deref())?;
            return Ok(());
        }

        if tag == "svg" {
            self.layout_svg_node(node, &style, x, width, anchor_name.as_deref())?;
            return Ok(());
        }

        if is_text_container(tag) || has_only_inline_children(node) {
            let (text, color_spans) = collect_text_and_color_spans_for_style(node, &style);
            if !text.is_empty() {
                let links = collect_links(node);
                let link_spans = resolve_text_link_spans(&text, &links);
                self.layout_text_block(
                    &text,
                    &style,
                    x,
                    width,
                    &link_spans,
                    &color_spans,
                    anchor_name.as_deref(),
                )?;
            }
            return Ok(());
        }

        if is_block_tag(tag) {
            let box_width = resolve_style_width_pt(&style, width).min(width).max(0.0);
            let content_x = x + style.margin.left + style.border_width_pt + style.padding.left;
            let content_width = (box_width
                - style.margin.horizontal()
                - style.padding.horizontal()
                - style.border_width_pt * 2.0)
                .max(1.0);
            self.cursor_y += style.margin.top;
            let start_y = self.cursor_y;
            if let Some(anchor_name) = anchor_name.as_ref() {
                self.current_ops().push(PaintOp::Anchor {
                    name: anchor_name.clone(),
                    y: start_y,
                });
            }
            self.cursor_y += style.border_width_pt + style.padding.top;

            let rect_page_index = self.document.pages.len() - 1;
            let op_index = self.current_ops().len();
            if style.background_color.is_some() || has_visible_border(&style) {
                self.current_ops().push(PaintOp::Rect {
                    x: x + style.margin.left,
                    y: start_y,
                    width: box_width - style.margin.horizontal(),
                    height: 0.0,
                    fill: style.background_color,
                    stroke: has_visible_border(&style).then_some(style.border_color),
                    stroke_width: style.border_width_pt,
                });
            }

            self.layout_children(node, ancestors, &style, content_x, content_width)?;
            self.cursor_y += style.padding.bottom + style.border_width_pt;

            if rect_page_index == self.document.pages.len() - 1 {
                let rect_height = (self.cursor_y - start_y).max(0.0);
                if let Some(PaintOp::Rect { height, .. }) = self.current_ops().get_mut(op_index) {
                    *height = rect_height;
                }
            }
            self.cursor_y += style.margin.bottom;
        }

        Ok(())
    }

    fn layout_text_block(
        &mut self,
        text: &str,
        style: &ComputedStyle,
        x: f32,
        width: f32,
        link_spans: &[TextLinkSpan],
        color_spans: &[TextColorSpan],
        anchor_name: Option<&str>,
    ) -> Result<()> {
        let block_width = resolve_style_width_pt(style, width).min(width).max(1.0);
        let outer_x = x + style.margin.left;
        let content_x = outer_x + style.border_width_pt + style.padding.left;
        let content_width = (block_width
            - style.margin.horizontal()
            - style.padding.horizontal()
            - style.border_width_pt * 2.0)
            .max(1.0);

        let lines = shape_lines(text, style, content_width, self.fonts, self.options)?;
        let line_layouts = if lines.is_empty() {
            vec![self.default_line_layout_metrics(style.font_size_pt, style.line_height_pt)]
        } else {
            lines
                .iter()
                .map(|line| {
                    self.measure_line_layout_metrics(line, style.font_size_pt, style.line_height_pt)
                })
                .collect::<Vec<_>>()
        };
        let text_height = line_layouts.iter().map(|line| line.height_pt).sum::<f32>();
        let block_height = style.padding.vertical() + style.border_width_pt * 2.0 + text_height;

        if self.cursor_y + style.margin.top + block_height + style.margin.bottom
            > self.page_bottom()
            && self.cursor_y > self.document.margin_pt + 1.0
        {
            self.new_page();
        }

        self.cursor_y += style.margin.top;
        let start_y = self.cursor_y;
        if let Some(anchor_name) = anchor_name {
            self.current_ops().push(PaintOp::Anchor {
                name: anchor_name.to_string(),
                y: start_y,
            });
        }
        if style.background_color.is_some() || has_visible_border(style) {
            self.current_ops().push(PaintOp::Rect {
                x: outer_x,
                y: start_y,
                width: block_width - style.margin.horizontal(),
                height: block_height,
                fill: style.background_color,
                stroke: has_visible_border(style).then_some(style.border_color),
                stroke_width: style.border_width_pt,
            });
        }

        self.cursor_y += style.border_width_pt + style.padding.top;
        for (line, line_layout) in lines.iter().zip(line_layouts.iter()) {
            if self.cursor_y + line_layout.height_pt > self.page_bottom()
                && self.cursor_y > self.document.margin_pt + 1.0
            {
                self.new_page();
            }
            let baseline_y = self.cursor_y + line_layout.ascent_pt;

            let line_clusters = collect_line_cluster_bounds(line);

            self.emit_line_glyphs_with_emoji_fallback(
                line,
                content_x,
                baseline_y,
                style.font_size_pt,
                style.color,
                color_spans,
            );

            if !line_clusters.is_empty() && !link_spans.is_empty() {
                let line_height = line_layout.height_pt.max(style.font_size_pt * 1.1);
                let line_top = self.cursor_y.max(0.0);
                for (x_start_pt, x_end_pt, target) in
                    resolve_line_link_rects(&line_clusters, link_spans)
                {
                    let width_pt = (x_end_pt - x_start_pt).max(0.0);
                    if width_pt <= 0.5 {
                        continue;
                    }
                    self.current_ops().push(PaintOp::Link {
                        x: content_x + x_start_pt,
                        y: line_top,
                        width: width_pt,
                        height: line_height,
                        target,
                    });
                }
            }

            self.cursor_y += line_layout.height_pt;
        }

        self.cursor_y = self.cursor_y.max(start_y + block_height);
        self.cursor_y += style.margin.bottom;
        Ok(())
    }

    fn default_line_layout_metrics(
        &self,
        font_size_pt: f32,
        line_height_pt: f32,
    ) -> LineLayoutMetrics {
        let height_pt = line_height_pt.max(font_size_pt * 1.1);
        let ascent_pt = (font_size_pt * 0.86).clamp(0.0, height_pt.max(0.01));
        LineLayoutMetrics {
            ascent_pt,
            height_pt,
        }
    }

    fn measure_line_layout_metrics(
        &self,
        line: &TextLine,
        font_size_pt: f32,
        line_height_pt: f32,
    ) -> LineLayoutMetrics {
        let default_metrics = self.default_line_layout_metrics(font_size_pt, line_height_pt);
        let mut ascent_pt = default_metrics.ascent_pt;
        let mut descent_pt = (default_metrics.height_pt - default_metrics.ascent_pt).max(0.0);

        let mut active_cluster = (usize::MAX, usize::MAX);
        for glyph in &line.glyphs {
            let cluster_key = (glyph.cluster_start, glyph.cluster_end);
            if cluster_key == active_cluster {
                continue;
            }
            active_cluster = cluster_key;

            if !cluster_contains_emoji(&glyph.cluster_text) {
                continue;
            }

            let font = self.fonts.get(glyph.font_key);
            if !font.is_color_emoji_font() {
                continue;
            }

            let Some(image) = decode_font_glyph_raster_image(font, glyph.glyph_id, font_size_pt)
            else {
                continue;
            };

            let scale = font_size_pt / image.pixels_per_em as f32;
            if scale <= 0.0 {
                continue;
            }

            let image_height_pt = image.pixel_height as f32 * scale;
            let top_above_baseline_pt = self.color_emoji_top_above_baseline_pt(
                font_size_pt,
                image.y as f32 * scale,
                glyph.y_offset_pt,
            );
            if top_above_baseline_pt.is_finite() {
                ascent_pt = ascent_pt.max(top_above_baseline_pt.max(0.0));
            }

            let bottom_below_baseline_pt = (image_height_pt - top_above_baseline_pt).max(0.0);
            if bottom_below_baseline_pt.is_finite() {
                descent_pt = descent_pt.max(bottom_below_baseline_pt);
            }
        }

        LineLayoutMetrics {
            ascent_pt,
            height_pt: (ascent_pt + descent_pt).max(default_metrics.height_pt),
        }
    }

    fn color_emoji_top_above_baseline_pt(
        &self,
        font_size_pt: f32,
        raster_top_above_baseline_pt: f32,
        glyph_y_offset_pt: f32,
    ) -> f32 {
        // Some color emoji fonts report bitmap y close to zero; use text-like ascent in that case.
        let text_like_top = font_size_pt * 0.86 + glyph_y_offset_pt;
        let raster_top = raster_top_above_baseline_pt + glyph_y_offset_pt;
        let top = if raster_top_above_baseline_pt <= font_size_pt * 0.25 {
            text_like_top
        } else {
            raster_top
        };
        top.max(0.0)
    }

    fn emit_line_glyphs_with_emoji_fallback(
        &mut self,
        line: &TextLine,
        origin_x: f32,
        baseline_y: f32,
        font_size_pt: f32,
        default_color: Color,
        color_spans: &[TextColorSpan],
    ) {
        let mut active_cluster = (usize::MAX, usize::MAX);
        let mut cluster_is_emoji_image = false;
        let mut cluster_color = default_color;

        for glyph in &line.glyphs {
            let cluster_key = (glyph.cluster_start, glyph.cluster_end);
            if cluster_key != active_cluster {
                active_cluster = cluster_key;
                cluster_color = resolve_cluster_color(
                    glyph.cluster_start,
                    glyph.cluster_end,
                    default_color,
                    color_spans,
                );
                cluster_is_emoji_image =
                    self.try_emit_color_emoji_glyph(glyph, origin_x, baseline_y, font_size_pt);
            }

            if cluster_is_emoji_image {
                continue;
            }

            self.current_ops().push(PaintOp::Glyph {
                font_key: glyph.font_key,
                glyph_id: glyph.glyph_id,
                cluster_text: glyph.cluster_text.clone(),
                font_size_pt,
                color: cluster_color,
                x: origin_x + glyph.x_pt,
                baseline_y,
                y_offset_pt: glyph.y_offset_pt,
            });
        }
    }

    fn try_emit_color_emoji_glyph(
        &mut self,
        glyph: &PositionedGlyph,
        origin_x: f32,
        baseline_y: f32,
        font_size_pt: f32,
    ) -> bool {
        if !cluster_contains_emoji(&glyph.cluster_text) {
            return false;
        }

        let font = self.fonts.get(glyph.font_key);
        if !font.is_color_emoji_font() {
            return false;
        }
        let Some(image) = decode_font_glyph_raster_image(font, glyph.glyph_id, font_size_pt) else {
            return false;
        };

        let scale = font_size_pt / image.pixels_per_em as f32;
        let width_pt = (image.pixel_width as f32 * scale).max(0.5);
        let height_pt = (image.pixel_height as f32 * scale).max(0.5);
        let x = origin_x + glyph.x_pt + image.x as f32 * scale;
        let top_above_baseline_pt = self.color_emoji_top_above_baseline_pt(
            font_size_pt,
            image.y as f32 * scale,
            glyph.y_offset_pt,
        );
        let y = baseline_y - top_above_baseline_pt;

        if width_pt <= 0.0 || height_pt <= 0.0 {
            return false;
        }

        self.current_ops().push(PaintOp::Image {
            x,
            y,
            width: width_pt,
            height: height_pt,
            pixel_width: image.pixel_width,
            pixel_height: image.pixel_height,
            rgb_data: image.rgb_data,
            alpha_data: image.alpha_data,
        });
        true
    }

    fn layout_image_node(
        &mut self,
        node: &'a HtmlNode,
        style: &ComputedStyle,
        x: f32,
        width: f32,
        anchor_name: Option<&str>,
    ) -> Result<()> {
        let Some(src) = node
            .attr("src")
            .map(str::trim)
            .filter(|value| !value.is_empty())
        else {
            return Ok(());
        };
        if src.starts_with("data:") {
            return Ok(());
        }

        let Some(path) = resolve_asset_path(self.base_dir, src) else {
            return Ok(());
        };

        let raster = load_raster_image(&path)?;
        if raster.pixel_width == 0 || raster.pixel_height == 0 {
            return Ok(());
        }

        let intrinsic_width_pt = raster.pixel_width as f32 * 0.75;
        let intrinsic_height_pt = raster.pixel_height as f32 * 0.75;
        let resolved_width = resolve_declared_width_pt(style, width)
            .or_else(|| node.attr("width").and_then(parse_attr_length_pt))
            .unwrap_or(intrinsic_width_pt)
            .max(1.0);

        let content_width = resolved_width.min(width.max(1.0));
        let content_height = node
            .attr("height")
            .and_then(parse_attr_length_pt)
            .unwrap_or_else(|| {
                let ratio = intrinsic_height_pt / intrinsic_width_pt.max(0.01);
                content_width * ratio
            })
            .max(1.0);

        let container_width = (width - style.margin.horizontal()).max(content_width);
        let outer_x = align_inline_x(
            x + style.margin.left,
            container_width,
            content_width + style.padding.horizontal() + style.border_width_pt * 2.0,
            style.text_align,
        );

        let block_height = style.padding.vertical() + style.border_width_pt * 2.0 + content_height;
        if self.cursor_y + style.margin.top + block_height + style.margin.bottom
            > self.page_bottom()
            && self.cursor_y > self.document.margin_pt + 1.0
        {
            self.new_page();
        }

        self.cursor_y += style.margin.top;
        let start_y = self.cursor_y;
        if let Some(anchor_name) = anchor_name {
            self.current_ops().push(PaintOp::Anchor {
                name: anchor_name.to_string(),
                y: start_y,
            });
        }

        let box_width = content_width + style.padding.horizontal() + style.border_width_pt * 2.0;
        if style.background_color.is_some() || has_visible_border(style) {
            self.current_ops().push(PaintOp::Rect {
                x: outer_x,
                y: start_y,
                width: box_width,
                height: block_height,
                fill: style.background_color,
                stroke: has_visible_border(style).then_some(style.border_color),
                stroke_width: style.border_width_pt,
            });
        }

        let image_x = outer_x + style.border_width_pt + style.padding.left;
        let image_y = start_y + style.border_width_pt + style.padding.top;
        self.current_ops().push(PaintOp::Image {
            x: image_x,
            y: image_y,
            width: content_width,
            height: content_height,
            pixel_width: raster.pixel_width,
            pixel_height: raster.pixel_height,
            rgb_data: raster.rgb_data,
            alpha_data: raster.alpha_data,
        });

        self.cursor_y = start_y + block_height;
        self.cursor_y += style.margin.bottom;
        Ok(())
    }

    fn layout_svg_node(
        &mut self,
        node: &'a HtmlNode,
        style: &ComputedStyle,
        x: f32,
        width: f32,
        anchor_name: Option<&str>,
    ) -> Result<()> {
        let (view_x, view_y, view_width, view_height) = resolve_svg_view_box(node)?;

        let attr_width_pt = node.attr("width").and_then(parse_attr_length_pt);
        let attr_height_pt = node.attr("height").and_then(parse_attr_length_pt);
        let default_width_pt =
            attr_width_pt.unwrap_or_else(|| (view_width.max(1.0) * 0.75).max(1.0));
        let content_width = resolve_declared_width_pt(style, width)
            .unwrap_or(default_width_pt)
            .min(width.max(1.0))
            .max(1.0);

        let content_height = attr_height_pt
            .unwrap_or_else(|| {
                let ratio = view_height.max(1.0) / view_width.max(1.0);
                content_width * ratio
            })
            .max(1.0);

        let container_width = (width - style.margin.horizontal()).max(content_width);
        let outer_x = align_inline_x(
            x + style.margin.left,
            container_width,
            content_width + style.padding.horizontal() + style.border_width_pt * 2.0,
            style.text_align,
        );

        let block_height = style.padding.vertical() + style.border_width_pt * 2.0 + content_height;
        if self.cursor_y + style.margin.top + block_height + style.margin.bottom
            > self.page_bottom()
            && self.cursor_y > self.document.margin_pt + 1.0
        {
            self.new_page();
        }

        self.cursor_y += style.margin.top;
        let start_y = self.cursor_y;
        if let Some(anchor_name) = anchor_name {
            self.current_ops().push(PaintOp::Anchor {
                name: anchor_name.to_string(),
                y: start_y,
            });
        }

        let box_width = content_width + style.padding.horizontal() + style.border_width_pt * 2.0;
        if style.background_color.is_some() || has_visible_border(style) {
            self.current_ops().push(PaintOp::Rect {
                x: outer_x,
                y: start_y,
                width: box_width,
                height: block_height,
                fill: style.background_color,
                stroke: has_visible_border(style).then_some(style.border_color),
                stroke_width: style.border_width_pt,
            });
        }

        let draw_x = outer_x + style.border_width_pt + style.padding.left;
        let draw_y = start_y + style.border_width_pt + style.padding.top;
        let scale_x = content_width / view_width.max(0.001);
        let scale_y = content_height / view_height.max(0.001);
        let svg_ctx = SvgRenderContext {
            view_x,
            view_y,
            view_width,
            view_height,
            transform: SvgTransform::new(
                scale_x,
                0.0,
                0.0,
                scale_y,
                draw_x - view_x * scale_x,
                draw_y - view_y * scale_y,
            ),
            inherited_opacity: 1.0,
            gradients: collect_svg_gradient_defs(node),
            style: style.clone(),
        };
        let root_ctx = derive_svg_context(&svg_ctx, node);
        self.emit_svg_children(node, &root_ctx)?;

        self.cursor_y = start_y + block_height;
        self.cursor_y += style.margin.bottom;
        Ok(())
    }

    fn emit_svg_children(&mut self, node: &'a HtmlNode, svg: &SvgRenderContext) -> Result<()> {
        for child in &node.children {
            let Some(tag) = child.tag_name() else {
                continue;
            };
            let child_ctx = derive_svg_context(svg, child);
            match tag {
                "rect" => {
                    if let Some(op) = build_svg_rect_path(child, &child_ctx) {
                        self.current_ops().push(op);
                    }
                }
                "circle" => {
                    if let Some(op) = build_svg_circle_path(child, &child_ctx) {
                        self.current_ops().push(op);
                    }
                }
                "ellipse" => {
                    if let Some(op) = build_svg_ellipse_path(child, &child_ctx) {
                        self.current_ops().push(op);
                    }
                }
                "line" => {
                    if let Some(op) = build_svg_line_path(child, &child_ctx) {
                        self.current_ops().push(op);
                    }
                }
                "polyline" => {
                    if let Some(op) = build_svg_poly_path(child, &child_ctx, false) {
                        self.current_ops().push(op);
                    }
                }
                "polygon" => {
                    if let Some(op) = build_svg_poly_path(child, &child_ctx, true) {
                        self.current_ops().push(op);
                    }
                }
                "path" => {
                    if let Some(op) = build_svg_path(child, &child_ctx) {
                        self.current_ops().push(op);
                    }
                }
                "text" => {
                    self.emit_svg_text(child, &child_ctx)?;
                }
                "g" | "svg" => {
                    self.emit_svg_children(child, &child_ctx)?;
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn emit_svg_text(&mut self, node: &'a HtmlNode, svg: &SvgRenderContext) -> Result<()> {
        let text = collect_text(node);
        if text.trim().is_empty() {
            return Ok(());
        }

        let x_unit = svg_attr_number(node, "x").unwrap_or(0.0);
        let y_unit = svg_attr_number(node, "y").unwrap_or(0.0);
        let font_size_unit = svg_attr_number(node, "font-size").unwrap_or(12.0).max(1.0);
        let font_size_pt = font_size_unit * svg.transform.average_scale().max(0.1);
        let fill = svg_paint(
            node,
            "fill",
            Some(SvgPaint::Solid(Color::rgb_u8(0, 0, 0))),
            &svg.gradients,
            svg,
            None,
        )
        .map(svg_paint_sample_color)
        .unwrap_or(Color::rgb_u8(0, 0, 0));

        let mut text_style = svg.style.clone();
        text_style.font_size_pt = font_size_pt;
        text_style.line_height_pt = font_size_pt * 1.2;
        text_style.color = fill;
        text_style.margin = crate::style::Edges::zero();
        text_style.padding = crate::style::Edges::zero();

        let mut lines = shape_lines(&text, &text_style, 20_000.0, self.fonts, self.options)?;
        let Some(line) = lines.pop() else {
            return Ok(());
        };

        let (mut start_x, baseline_y) = svg.transform.apply(x_unit, y_unit);
        let anchor = svg_attr_string(node, "text-anchor").unwrap_or_default();
        if anchor.eq_ignore_ascii_case("middle") {
            start_x -= line.width_pt * 0.5;
        } else if anchor.eq_ignore_ascii_case("end") {
            start_x -= line.width_pt;
        }

        self.emit_line_glyphs_with_emoji_fallback(
            &line,
            start_x,
            baseline_y,
            font_size_pt,
            fill,
            &[],
        );

        Ok(())
    }

    fn layout_table(
        &mut self,
        table: &'a HtmlNode,
        ancestors: &[&'a HtmlNode],
        style: &ComputedStyle,
        x: f32,
        width: f32,
    ) -> Result<()> {
        let table_width = resolve_style_width_pt(style, width).min(width).max(1.0);
        let content_x = x + style.margin.left + style.border_width_pt + style.padding.left;
        let content_width = (table_width
            - style.margin.horizontal()
            - style.padding.horizontal()
            - style.border_width_pt * 2.0)
            .max(1.0);

        let mut table_ancestors = ancestors.to_vec();
        table_ancestors.push(table);
        let sections = collect_table_sections(table, &table_ancestors);
        if sections.header.is_empty() && sections.body.is_empty() && sections.footer.is_empty() {
            return Ok(());
        }

        let column_count = table_column_count(&sections).max(1);
        let header_layouts = build_table_row_layouts(&sections.header, column_count);
        let body_layouts = build_table_row_layouts(&sections.body, column_count);
        let footer_layouts = build_table_row_layouts(&sections.footer, column_count);

        let column_widths = self.compute_table_column_widths(
            &sections,
            &header_layouts,
            &body_layouts,
            &footer_layouts,
            style,
            content_width,
            column_count,
        );
        let measured_header =
            self.measure_table_rows(&sections.header, &header_layouts, style, &column_widths)?;
        let measured_body =
            self.measure_table_rows(&sections.body, &body_layouts, style, &column_widths)?;
        let measured_footer =
            self.measure_table_rows(&sections.footer, &footer_layouts, style, &column_widths)?;

        let header_height = measured_header.iter().map(|row| row.height).sum::<f32>();
        let body_height = measured_body.iter().map(|row| row.height).sum::<f32>();
        let footer_height = measured_footer.iter().map(|row| row.height).sum::<f32>();
        let estimated_total_height = style.margin.vertical()
            + style.padding.vertical()
            + style.border_width_pt * 2.0
            + header_height
            + body_height
            + footer_height;

        let page_content_height = self.page_bottom() - self.document.margin_pt;
        if style.page_break_inside_avoid
            && estimated_total_height <= page_content_height
            && self.cursor_y + estimated_total_height > self.page_bottom()
            && self.cursor_y > self.document.margin_pt + 1.0
        {
            self.new_page();
        }

        self.cursor_y += style.margin.top;
        self.cursor_y += style.border_width_pt + style.padding.top;

        if !measured_header.is_empty() {
            if self.cursor_y + header_height > self.page_bottom()
                && self.cursor_y > self.document.margin_pt + 1.0
            {
                self.new_page();
                self.cursor_y += style.border_width_pt + style.padding.top;
            }
            self.render_table_rows(&measured_header, &column_widths, content_x, style, true)?;
        }

        let mut body_index = 0usize;
        let mut first_body_row_in_fragment = measured_header.is_empty();
        while body_index < measured_body.len() {
            let fragment_start = body_index;
            let mut probe_y = self.cursor_y;

            while body_index < measured_body.len() {
                let row = &measured_body[body_index];
                let required_height = row.min_block_height.max(row.height);
                if probe_y + required_height > self.page_bottom()
                    && probe_y > self.document.margin_pt + 1.0
                {
                    break;
                }

                probe_y += row.height;
                body_index += 1;
            }

            if body_index == fragment_start {
                if self.cursor_y > self.document.margin_pt + 1.0 {
                    self.new_page();
                    self.cursor_y += style.border_width_pt + style.padding.top;
                    if !measured_header.is_empty() {
                        self.render_table_rows(
                            &measured_header,
                            &column_widths,
                            content_x,
                            style,
                            true,
                        )?;
                        first_body_row_in_fragment = false;
                    } else {
                        first_body_row_in_fragment = true;
                    }
                    continue;
                }
                body_index += 1;
            }

            self.render_table_rows(
                &measured_body[fragment_start..body_index],
                &column_widths,
                content_x,
                style,
                first_body_row_in_fragment,
            )?;
            first_body_row_in_fragment = false;

            if body_index < measured_body.len() {
                self.new_page();
                self.cursor_y += style.border_width_pt + style.padding.top;
                if !measured_header.is_empty() {
                    self.render_table_rows(
                        &measured_header,
                        &column_widths,
                        content_x,
                        style,
                        true,
                    )?;
                    first_body_row_in_fragment = false;
                } else {
                    first_body_row_in_fragment = true;
                }
            }
        }

        if !measured_footer.is_empty() {
            if self.cursor_y + footer_height > self.page_bottom()
                && self.cursor_y > self.document.margin_pt + 1.0
            {
                self.new_page();
                self.cursor_y += style.border_width_pt + style.padding.top;
                if !measured_header.is_empty() {
                    self.render_table_rows(
                        &measured_header,
                        &column_widths,
                        content_x,
                        style,
                        true,
                    )?;
                }
            }
            self.render_table_rows(&measured_footer, &column_widths, content_x, style, false)?;
        }

        self.cursor_y += style.padding.bottom + style.border_width_pt;
        self.cursor_y += style.margin.bottom;
        Ok(())
    }

    fn compute_table_column_widths(
        &self,
        sections: &TableSections<'a>,
        header_layouts: &[TableRowLayout],
        body_layouts: &[TableRowLayout],
        footer_layouts: &[TableRowLayout],
        table_style: &ComputedStyle,
        content_width: f32,
        column_count: usize,
    ) -> Vec<f32> {
        let mut min_widths = vec![TABLE_MIN_COL_WIDTH_PT; column_count];
        let mut preferred_widths = vec![TABLE_MIN_COL_WIDTH_PT; column_count];

        for (row, row_layout) in sections.header.iter().zip(header_layouts.iter()) {
            let row_style = compute_style(row.node, &row.ancestors, self.sheet, table_style);
            for placement in &row_layout.placements {
                if placement.col_start >= column_count {
                    continue;
                }

                let span = placement
                    .colspan
                    .min(column_count - placement.col_start)
                    .max(1);
                let cell = &row.cells[placement.cell_index];
                let mut cell_ancestors = row.ancestors.clone();
                cell_ancestors.push(row.node);
                let cell_style = compute_style(cell.node, &cell_ancestors, self.sheet, &row_style);

                let declared_width_pt = self.resolve_table_cell_declared_width_pt(
                    cell.node,
                    &cell_style,
                    content_width,
                );
                let (intrinsic_min_pt, intrinsic_pref_pt) =
                    self.measure_table_cell_intrinsic_widths(cell.node, &cell_style);

                let box_overhead_pt =
                    cell_style.padding.horizontal() + cell_style.border_width_pt * 2.0;
                let mut required_min_pt =
                    (intrinsic_min_pt + box_overhead_pt).max(TABLE_MIN_COL_WIDTH_PT * span as f32);
                let mut required_pref_pt =
                    (intrinsic_pref_pt + box_overhead_pt).max(required_min_pt);

                if let Some(declared_width_pt) = declared_width_pt {
                    required_min_pt = required_min_pt.max(declared_width_pt);
                    required_pref_pt = required_pref_pt.max(declared_width_pt);
                }

                distribute_required_width(
                    &mut min_widths[placement.col_start..placement.col_start + span],
                    required_min_pt,
                );
                distribute_required_width(
                    &mut preferred_widths[placement.col_start..placement.col_start + span],
                    required_pref_pt,
                );
            }
        }

        for (row, row_layout) in sections.body.iter().zip(body_layouts.iter()) {
            let row_style = compute_style(row.node, &row.ancestors, self.sheet, table_style);
            for placement in &row_layout.placements {
                if placement.col_start >= column_count {
                    continue;
                }

                let span = placement
                    .colspan
                    .min(column_count - placement.col_start)
                    .max(1);
                let cell = &row.cells[placement.cell_index];
                let mut cell_ancestors = row.ancestors.clone();
                cell_ancestors.push(row.node);
                let cell_style = compute_style(cell.node, &cell_ancestors, self.sheet, &row_style);

                let declared_width_pt = self.resolve_table_cell_declared_width_pt(
                    cell.node,
                    &cell_style,
                    content_width,
                );
                let (intrinsic_min_pt, intrinsic_pref_pt) =
                    self.measure_table_cell_intrinsic_widths(cell.node, &cell_style);

                let box_overhead_pt =
                    cell_style.padding.horizontal() + cell_style.border_width_pt * 2.0;
                let mut required_min_pt =
                    (intrinsic_min_pt + box_overhead_pt).max(TABLE_MIN_COL_WIDTH_PT * span as f32);
                let mut required_pref_pt =
                    (intrinsic_pref_pt + box_overhead_pt).max(required_min_pt);

                if let Some(declared_width_pt) = declared_width_pt {
                    required_min_pt = required_min_pt.max(declared_width_pt);
                    required_pref_pt = required_pref_pt.max(declared_width_pt);
                }

                distribute_required_width(
                    &mut min_widths[placement.col_start..placement.col_start + span],
                    required_min_pt,
                );
                distribute_required_width(
                    &mut preferred_widths[placement.col_start..placement.col_start + span],
                    required_pref_pt,
                );
            }
        }

        for (row, row_layout) in sections.footer.iter().zip(footer_layouts.iter()) {
            let row_style = compute_style(row.node, &row.ancestors, self.sheet, table_style);
            for placement in &row_layout.placements {
                if placement.col_start >= column_count {
                    continue;
                }

                let span = placement
                    .colspan
                    .min(column_count - placement.col_start)
                    .max(1);
                let cell = &row.cells[placement.cell_index];
                let mut cell_ancestors = row.ancestors.clone();
                cell_ancestors.push(row.node);
                let cell_style = compute_style(cell.node, &cell_ancestors, self.sheet, &row_style);

                let declared_width_pt = self.resolve_table_cell_declared_width_pt(
                    cell.node,
                    &cell_style,
                    content_width,
                );
                let (intrinsic_min_pt, intrinsic_pref_pt) =
                    self.measure_table_cell_intrinsic_widths(cell.node, &cell_style);

                let box_overhead_pt =
                    cell_style.padding.horizontal() + cell_style.border_width_pt * 2.0;
                let mut required_min_pt =
                    (intrinsic_min_pt + box_overhead_pt).max(TABLE_MIN_COL_WIDTH_PT * span as f32);
                let mut required_pref_pt =
                    (intrinsic_pref_pt + box_overhead_pt).max(required_min_pt);

                if let Some(declared_width_pt) = declared_width_pt {
                    required_min_pt = required_min_pt.max(declared_width_pt);
                    required_pref_pt = required_pref_pt.max(declared_width_pt);
                }

                distribute_required_width(
                    &mut min_widths[placement.col_start..placement.col_start + span],
                    required_min_pt,
                );
                distribute_required_width(
                    &mut preferred_widths[placement.col_start..placement.col_start + span],
                    required_pref_pt,
                );
            }
        }

        for (preferred, min) in preferred_widths.iter_mut().zip(min_widths.iter()) {
            *preferred = preferred.max(*min);
        }

        let mut resolved = solve_auto_table_widths(&min_widths, &preferred_widths, content_width);
        rebalance_width_sum(
            &mut resolved,
            content_width,
            TABLE_ABSOLUTE_MIN_COL_WIDTH_PT,
        );
        resolved
    }

    fn resolve_table_cell_declared_width_pt(
        &self,
        cell_node: &HtmlNode,
        cell_style: &ComputedStyle,
        table_content_width: f32,
    ) -> Option<f32> {
        let from_css = resolve_declared_width_pt(cell_style, table_content_width);
        if from_css.is_some() {
            return from_css;
        }

        let raw_attr = cell_node.attr("width")?;
        if let Some(percent) = parse_percentage(raw_attr) {
            return Some(table_content_width * percent);
        }
        parse_length(raw_attr, cell_style.font_size_pt)
    }

    fn measure_table_cell_intrinsic_widths(
        &self,
        cell_node: &HtmlNode,
        cell_style: &ComputedStyle,
    ) -> (f32, f32) {
        let text = collect_text_for_style(cell_node, cell_style);
        if text.trim().is_empty() {
            return (0.0, 0.0);
        }

        let max_content_pt =
            self.measure_shaped_text_width_pt(&text, cell_style, TABLE_MEASURE_MAX_WIDTH_PT);
        let probe_width_pt = (cell_style.font_size_pt * 2.6)
            .max(TABLE_MIN_COL_WIDTH_PT)
            .min(TABLE_MEASURE_MAX_WIDTH_PT);
        let wrapped_probe_pt = self.measure_shaped_text_width_pt(&text, cell_style, probe_width_pt);
        let longest_token_pt = self.measure_longest_token_width_pt(&text, cell_style);

        let contains_thai = text.chars().any(is_thai_script_char);
        let min_content_pt = if contains_thai {
            wrapped_probe_pt.max(cell_style.font_size_pt * 1.4)
        } else {
            wrapped_probe_pt.max(longest_token_pt)
        }
        .min(max_content_pt)
        .max(0.0);

        (min_content_pt, max_content_pt.max(min_content_pt))
    }

    fn measure_longest_token_width_pt(&self, text: &str, cell_style: &ComputedStyle) -> f32 {
        text.split(is_token_break_char)
            .map(str::trim)
            .filter(|token| !token.is_empty())
            .take(48)
            .map(|token| {
                self.measure_shaped_text_width_pt(token, cell_style, TABLE_MEASURE_MAX_WIDTH_PT)
            })
            .fold(0.0, f32::max)
    }

    fn measure_shaped_text_width_pt(
        &self,
        text: &str,
        cell_style: &ComputedStyle,
        max_width_pt: f32,
    ) -> f32 {
        match shape_lines(text, cell_style, max_width_pt, self.fonts, self.options) {
            Ok(lines) if !lines.is_empty() => {
                lines.iter().map(|line| line.width_pt).fold(0.0, f32::max)
            }
            Ok(_) => 0.0,
            Err(_) => text.chars().count() as f32 * cell_style.font_size_pt * 0.55,
        }
    }

    fn measure_table_rows(
        &mut self,
        rows: &[TableRowRef<'a>],
        row_layouts: &[TableRowLayout],
        table_style: &ComputedStyle,
        column_widths: &[f32],
    ) -> Result<Vec<MeasuredTableRow>> {
        if rows.is_empty() {
            return Ok(Vec::new());
        }

        let mut measured = Vec::with_capacity(rows.len());
        let mut row_heights = vec![0.0f32; rows.len()];

        for (row_index, (row, row_layout)) in rows.iter().zip(row_layouts.iter()).enumerate() {
            let row_style = compute_style(row.node, &row.ancestors, self.sheet, table_style);
            let row_base_height = row_style.padding.vertical()
                + row_style.border_width_pt * 2.0
                + row_style.line_height_pt.max(row_style.font_size_pt * 1.1);
            row_heights[row_index] = row_base_height.max(1.0);

            let mut measured_cells = Vec::with_capacity(row_layout.placements.len());
            for placement in &row_layout.placements {
                if placement.col_start >= column_widths.len() {
                    continue;
                }

                let span = placement
                    .colspan
                    .min(column_widths.len() - placement.col_start)
                    .max(1);
                let cell_ref = &row.cells[placement.cell_index];
                let cell_width = column_widths[placement.col_start..placement.col_start + span]
                    .iter()
                    .sum::<f32>();

                let mut cell_ancestors = row.ancestors.clone();
                cell_ancestors.push(row.node);
                let cell_style =
                    compute_style(cell_ref.node, &cell_ancestors, self.sheet, &row_style);
                let cell_text = collect_text_for_style(cell_ref.node, &cell_style);
                let cell_content_width = (cell_width
                    - cell_style.padding.horizontal()
                    - cell_style.border_width_pt * 2.0)
                    .max(1.0);

                let lines = if cell_text.is_empty() {
                    Vec::new()
                } else {
                    shape_lines(
                        &cell_text,
                        &cell_style,
                        cell_content_width,
                        self.fonts,
                        self.options,
                    )?
                };

                let line_layouts = lines
                    .iter()
                    .map(|line| {
                        self.measure_line_layout_metrics(
                            line,
                            cell_style.font_size_pt,
                            cell_style.line_height_pt,
                        )
                    })
                    .collect::<Vec<_>>();
                let text_height = if line_layouts.is_empty() {
                    cell_style.line_height_pt
                } else {
                    line_layouts.iter().map(|line| line.height_pt).sum::<f32>()
                };
                let required_height =
                    cell_style.padding.vertical() + cell_style.border_width_pt * 2.0 + text_height;

                if placement.rowspan <= 1 {
                    row_heights[row_index] = row_heights[row_index].max(required_height);
                }

                measured_cells.push(MeasuredTableCell {
                    style: cell_style,
                    col_start: placement.col_start,
                    colspan: span,
                    rowspan: placement.rowspan.max(1),
                    lines,
                    line_layouts,
                    required_height,
                    render_height: required_height,
                });
            }

            measured.push(MeasuredTableRow {
                cells: measured_cells,
                height: row_heights[row_index].max(1.0),
                min_block_height: row_heights[row_index].max(1.0),
            });
        }

        for _ in 0..6 {
            let mut changed = false;
            for row_index in 0..measured.len() {
                for cell in &measured[row_index].cells {
                    if cell.rowspan <= 1 {
                        continue;
                    }
                    let span_end = (row_index + cell.rowspan).min(measured.len());
                    let current_height = row_heights[row_index..span_end].iter().sum::<f32>();
                    if current_height + 0.01 >= cell.required_height {
                        continue;
                    }

                    let deficit = cell.required_height - current_height;
                    let span_rows = (span_end - row_index).max(1);
                    let delta = deficit / span_rows as f32;
                    for height in &mut row_heights[row_index..span_end] {
                        *height += delta;
                    }
                    changed = true;
                }
            }
            if !changed {
                break;
            }
        }

        for (row_index, row) in measured.iter_mut().enumerate() {
            row.height = row_heights[row_index].max(1.0);
            row.min_block_height = row.height;
        }

        let row_count = measured.len();
        for row_index in 0..row_count {
            let mut max_block_height = measured[row_index].height;
            for cell in &mut measured[row_index].cells {
                let span_end = (row_index + cell.rowspan).min(row_count);
                let span_height = row_heights[row_index..span_end]
                    .iter()
                    .sum::<f32>()
                    .max(1.0);
                cell.render_height = span_height;
                max_block_height = max_block_height.max(span_height);
            }
            measured[row_index].min_block_height = max_block_height;
        }

        Ok(measured)
    }

    fn render_table_rows(
        &mut self,
        rows: &[MeasuredTableRow],
        column_widths: &[f32],
        content_x: f32,
        table_style: &ComputedStyle,
        is_new_table_fragment: bool,
    ) -> Result<()> {
        if rows.is_empty() {
            return Ok(());
        }

        let collapse_borders = matches!(table_style.border_collapse, BorderCollapse::Collapse);
        let border_spacing = if collapse_borders {
            0.0
        } else {
            table_style.border_spacing_pt.max(0.0)
        };

        let mut row_top = self.cursor_y;
        let mut row_tops = Vec::with_capacity(rows.len());
        for row in rows {
            row_tops.push(row_top);
            self.render_table_row_content(
                row,
                column_widths,
                content_x,
                row_top,
                collapse_borders,
                border_spacing,
            )?;
            row_top += row.height;
        }

        if collapse_borders {
            self.emit_collapsed_fragment_borders(
                rows,
                column_widths,
                content_x,
                table_style,
                &row_tops,
                is_new_table_fragment,
            );
        }

        self.cursor_y = row_top;
        Ok(())
    }

    fn render_table_row_content(
        &mut self,
        row: &MeasuredTableRow,
        column_widths: &[f32],
        content_x: f32,
        row_top: f32,
        collapse_borders: bool,
        border_spacing: f32,
    ) -> Result<()> {
        for cell in &row.cells {
            if cell.col_start >= column_widths.len() {
                break;
            }

            let span = cell
                .colspan
                .min(column_widths.len() - cell.col_start)
                .max(1);
            let cell_x_base = content_x + column_widths[..cell.col_start].iter().sum::<f32>();
            let cell_width_base = column_widths[cell.col_start..cell.col_start + span]
                .iter()
                .sum::<f32>();

            let mut draw_x = cell_x_base;
            let mut draw_y = row_top;
            let mut draw_width = cell_width_base;
            let mut draw_height = cell.render_height;

            if border_spacing > 0.0 {
                let inset = border_spacing * 0.5;
                draw_x += inset;
                draw_y += inset;
                draw_width = (draw_width - border_spacing).max(1.0);
                draw_height = (draw_height - border_spacing).max(1.0);
            }

            let separate_border = !collapse_borders && has_visible_border(&cell.style);
            if cell.style.background_color.is_some() {
                self.current_ops().push(PaintOp::Rect {
                    x: draw_x,
                    y: draw_y,
                    width: draw_width,
                    height: draw_height,
                    fill: cell.style.background_color,
                    stroke: None,
                    stroke_width: 0.0,
                });
            }
            if separate_border {
                self.emit_styled_box_borders(draw_x, draw_y, draw_width, draw_height, &cell.style);
            }

            let text_x = draw_x + cell.style.border_width_pt + cell.style.padding.left;
            let mut line_top = draw_y + cell.style.border_width_pt + cell.style.padding.top;
            for (line_index, line) in cell.lines.iter().enumerate() {
                let line_layout = cell
                    .line_layouts
                    .get(line_index)
                    .copied()
                    .unwrap_or_else(|| {
                        self.default_line_layout_metrics(
                            cell.style.font_size_pt,
                            cell.style.line_height_pt,
                        )
                    });
                let baseline_y = line_top + line_layout.ascent_pt;
                self.emit_line_glyphs_with_emoji_fallback(
                    line,
                    text_x,
                    baseline_y,
                    cell.style.font_size_pt,
                    cell.style.color,
                    &[],
                );
                line_top += line_layout.height_pt;
            }
        }

        Ok(())
    }

    fn emit_collapsed_fragment_borders(
        &mut self,
        rows: &[MeasuredTableRow],
        column_widths: &[f32],
        content_x: f32,
        table_style: &ComputedStyle,
        row_tops: &[f32],
        is_first_visual_row: bool,
    ) {
        if rows.is_empty() || row_tops.is_empty() || column_widths.is_empty() {
            return;
        }

        let fragment_top = row_tops[0];
        let fragment_bottom =
            row_tops[row_tops.len() - 1] + rows.last().map(|row| row.height).unwrap_or(0.0);
        let fragment_left = content_x;
        let fragment_right = content_x + column_widths.iter().sum::<f32>();

        let mut grouped_edges: BTreeMap<(BorderEdgeOrientation, i32), Vec<BorderEdgeCandidate>> =
            BTreeMap::new();

        for (row_index, row) in rows.iter().enumerate() {
            let row_top = row_tops[row_index];
            for cell in &row.cells {
                if cell.col_start >= column_widths.len() {
                    continue;
                }

                let span = cell
                    .colspan
                    .min(column_widths.len() - cell.col_start)
                    .max(1);
                let cell_x = content_x + column_widths[..cell.col_start].iter().sum::<f32>();
                let cell_width = column_widths[cell.col_start..cell.col_start + span]
                    .iter()
                    .sum::<f32>();
                let cell_y = row_top;
                let cell_height = cell.render_height.max(1.0);

                let row_anchor_top = row_index;
                let row_anchor_bottom = row_index + cell.rowspan.saturating_sub(1);
                let col_anchor_left = cell.col_start;
                let col_anchor_right = cell.col_start + span;

                append_collapsed_border_candidate(
                    &mut grouped_edges,
                    BorderEdgeOrientation::Vertical,
                    cell_x,
                    cell_y,
                    cell_y + cell_height,
                    &cell.style,
                    PaintLineEdge::Left,
                    BORDER_SOURCE_CELL,
                    row_anchor_top,
                    col_anchor_left,
                    fragment_top,
                    fragment_bottom,
                );
                if row_index > 0 || is_first_visual_row {
                    append_collapsed_border_candidate(
                        &mut grouped_edges,
                        BorderEdgeOrientation::Horizontal,
                        cell_y,
                        cell_x,
                        cell_x + cell_width,
                        &cell.style,
                        PaintLineEdge::Top,
                        BORDER_SOURCE_CELL,
                        row_anchor_top,
                        col_anchor_left,
                        fragment_left,
                        fragment_right,
                    );
                }
                append_collapsed_border_candidate(
                    &mut grouped_edges,
                    BorderEdgeOrientation::Vertical,
                    cell_x + cell_width,
                    cell_y,
                    cell_y + cell_height,
                    &cell.style,
                    PaintLineEdge::Right,
                    BORDER_SOURCE_CELL,
                    row_anchor_top,
                    col_anchor_right,
                    fragment_top,
                    fragment_bottom,
                );
                append_collapsed_border_candidate(
                    &mut grouped_edges,
                    BorderEdgeOrientation::Horizontal,
                    cell_y + cell_height,
                    cell_x,
                    cell_x + cell_width,
                    &cell.style,
                    PaintLineEdge::Bottom,
                    BORDER_SOURCE_CELL,
                    row_anchor_bottom,
                    col_anchor_left,
                    fragment_left,
                    fragment_right,
                );
            }
        }

        if contributes_collapsed_border(table_style) {
            append_collapsed_border_candidate(
                &mut grouped_edges,
                BorderEdgeOrientation::Vertical,
                fragment_left,
                fragment_top,
                fragment_bottom,
                table_style,
                PaintLineEdge::Left,
                BORDER_SOURCE_TABLE,
                0,
                0,
                fragment_top,
                fragment_bottom,
            );
            append_collapsed_border_candidate(
                &mut grouped_edges,
                BorderEdgeOrientation::Vertical,
                fragment_right,
                fragment_top,
                fragment_bottom,
                table_style,
                PaintLineEdge::Right,
                BORDER_SOURCE_TABLE,
                0,
                column_widths.len(),
                fragment_top,
                fragment_bottom,
            );
            if is_first_visual_row {
                append_collapsed_border_candidate(
                    &mut grouped_edges,
                    BorderEdgeOrientation::Horizontal,
                    fragment_top,
                    fragment_left,
                    fragment_right,
                    table_style,
                    PaintLineEdge::Top,
                    BORDER_SOURCE_TABLE,
                    0,
                    0,
                    fragment_left,
                    fragment_right,
                );
            }
            append_collapsed_border_candidate(
                &mut grouped_edges,
                BorderEdgeOrientation::Horizontal,
                fragment_bottom,
                fragment_left,
                fragment_right,
                table_style,
                PaintLineEdge::Bottom,
                BORDER_SOURCE_TABLE,
                rows.len(),
                0,
                fragment_left,
                fragment_right,
            );
        }

        for ((orientation, _axis_key), candidates) in grouped_edges.into_iter() {
            self.emit_resolved_collapsed_axis(orientation, &candidates);
        }
    }

    fn emit_resolved_collapsed_axis(
        &mut self,
        orientation: BorderEdgeOrientation,
        candidates: &[BorderEdgeCandidate],
    ) {
        if candidates.is_empty() {
            return;
        }

        let mut breakpoints = Vec::with_capacity(candidates.len() * 2);
        for candidate in candidates {
            breakpoints.push(candidate.start_pt);
            breakpoints.push(candidate.end_pt);
        }
        breakpoints.sort_by(|lhs, rhs| lhs.total_cmp(rhs));
        breakpoints.dedup_by(|lhs, rhs| (*lhs - *rhs).abs() <= COLLAPSED_BORDER_EPSILON_PT);
        if breakpoints.len() < 2 {
            return;
        }

        for segment in breakpoints.windows(2) {
            let segment_start = segment[0];
            let segment_end = segment[1];
            if segment_end - segment_start <= COLLAPSED_BORDER_EPSILON_PT {
                continue;
            }

            let mut winner: Option<&BorderEdgeCandidate> = None;
            for candidate in candidates {
                if candidate.start_pt <= segment_start + COLLAPSED_BORDER_EPSILON_PT
                    && candidate.end_pt >= segment_end - COLLAPSED_BORDER_EPSILON_PT
                {
                    if let Some(current) = winner {
                        if collapsed_border_candidate_wins(candidate, current) {
                            winner = Some(candidate);
                        }
                    } else {
                        winner = Some(candidate);
                    }
                }
            }

            let Some(winner) = winner else {
                continue;
            };

            if matches!(winner.border_style, BorderStyle::Hidden | BorderStyle::None)
                || winner.width_pt <= COLLAPSED_BORDER_EPSILON_PT
            {
                continue;
            }

            match orientation {
                BorderEdgeOrientation::Vertical => self.current_ops().push(PaintOp::Line {
                    x1: winner.axis_pt,
                    y1: segment_start,
                    x2: winner.axis_pt,
                    y2: segment_end,
                    color: winner.color,
                    width: winner.width_pt,
                    border_style: normalize_paint_line_style(winner.border_style),
                    edge: winner.edge,
                }),
                BorderEdgeOrientation::Horizontal => self.current_ops().push(PaintOp::Line {
                    x1: segment_start,
                    y1: winner.axis_pt,
                    x2: segment_end,
                    y2: winner.axis_pt,
                    color: winner.color,
                    width: winner.width_pt,
                    border_style: normalize_paint_line_style(winner.border_style),
                    edge: winner.edge,
                }),
            }
        }
    }

    fn emit_styled_box_borders(
        &mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        style: &ComputedStyle,
    ) {
        if !has_visible_border(style) {
            return;
        }

        let line_style = normalize_paint_line_style(style.border_style);
        self.current_ops().push(PaintOp::Line {
            x1: x,
            y1: y,
            x2: x + width,
            y2: y,
            color: style.border_color,
            width: style.border_width_pt,
            border_style: line_style,
            edge: PaintLineEdge::Top,
        });
        self.current_ops().push(PaintOp::Line {
            x1: x + width,
            y1: y,
            x2: x + width,
            y2: y + height,
            color: style.border_color,
            width: style.border_width_pt,
            border_style: line_style,
            edge: PaintLineEdge::Right,
        });
        self.current_ops().push(PaintOp::Line {
            x1: x,
            y1: y + height,
            x2: x + width,
            y2: y + height,
            color: style.border_color,
            width: style.border_width_pt,
            border_style: line_style,
            edge: PaintLineEdge::Bottom,
        });
        self.current_ops().push(PaintOp::Line {
            x1: x,
            y1: y,
            x2: x,
            y2: y + height,
            color: style.border_color,
            width: style.border_width_pt,
            border_style: line_style,
            edge: PaintLineEdge::Left,
        });
    }

    fn current_ops(&mut self) -> &mut Vec<PaintOp> {
        &mut self.document.pages.last_mut().unwrap().ops
    }

    fn page_bottom(&self) -> f32 {
        self.document.page_size.height_pt - self.document.margin_pt
    }

    fn new_page(&mut self) {
        self.document.pages.push(LayoutPage { ops: Vec::new() });
        self.cursor_y = self.document.margin_pt;
    }
}

fn has_visible_border(style: &ComputedStyle) -> bool {
    style.border_style.is_visible() && style.border_width_pt > COLLAPSED_BORDER_EPSILON_PT
}

fn collect_text_for_style(node: &HtmlNode, style: &ComputedStyle) -> String {
    match style.white_space {
        WhiteSpace::Pre | WhiteSpace::PreWrap | WhiteSpace::PreLine => {
            collect_text_preserving_whitespace(node)
        }
        WhiteSpace::Normal | WhiteSpace::NoWrap => collect_text(node),
    }
}

fn collect_text_and_color_spans_for_style(
    node: &HtmlNode,
    style: &ComputedStyle,
) -> (String, Vec<TextColorSpan>) {
    let (raw_text, raw_spans) = collect_text_with_inline_color_spans(node, style.color);
    normalize_text_and_color_spans_for_white_space(
        &raw_text,
        &raw_spans,
        style.white_space,
        style.color,
    )
}

fn collect_text_with_inline_color_spans(
    node: &HtmlNode,
    default_color: Color,
) -> (String, Vec<TextColorSpan>) {
    let mut text = String::new();
    let mut spans = Vec::new();
    collect_text_with_inline_color_spans_inner(node, default_color, &mut text, &mut spans);
    (text, spans)
}

fn collect_text_with_inline_color_spans_inner(
    node: &HtmlNode,
    inherited_color: Color,
    text: &mut String,
    spans: &mut Vec<TextColorSpan>,
) {
    match &node.kind {
        HtmlNodeKind::Text(value) => push_colored_text(text, spans, value, inherited_color),
        HtmlNodeKind::Element(tag) if tag == "br" => {
            push_colored_text(text, spans, "\n", inherited_color)
        }
        HtmlNodeKind::Element(tag) if tag == "script" || tag == "style" => {}
        HtmlNodeKind::Element(_) | HtmlNodeKind::Document => {
            let mut active_color = inherited_color;
            if let Some(style_attr) = node.attr("style") {
                if let Some(inline_color) = parse_inline_color_from_style_attr(style_attr) {
                    active_color = inline_color;
                }
            }

            for child in &node.children {
                collect_text_with_inline_color_spans_inner(child, active_color, text, spans);
            }
        }
    }
}

fn push_colored_text(
    text: &mut String,
    spans: &mut Vec<TextColorSpan>,
    fragment: &str,
    color: Color,
) {
    if fragment.is_empty() {
        return;
    }

    let start = text.len();
    text.push_str(fragment);
    let end = text.len();
    if start >= end {
        return;
    }

    if let Some(last) = spans.last_mut() {
        if last.end == start && last.color == color {
            last.end = end;
            return;
        }
    }

    spans.push(TextColorSpan { start, end, color });
}

fn parse_inline_color_from_style_attr(style_attr: &str) -> Option<Color> {
    for declaration in style_attr.split(';') {
        let Some((name, value)) = declaration.split_once(':') else {
            continue;
        };

        if name.trim().eq_ignore_ascii_case("color") {
            if let Some(color) = parse_color(value.trim()) {
                return Some(color);
            }
        }
    }

    None
}

fn normalize_text_and_color_spans_for_white_space(
    text: &str,
    spans: &[TextColorSpan],
    white_space: WhiteSpace,
    default_color: Color,
) -> (String, Vec<TextColorSpan>) {
    match white_space {
        WhiteSpace::Pre | WhiteSpace::PreWrap => (text.to_string(), spans.to_vec()),
        WhiteSpace::Normal | WhiteSpace::NoWrap => {
            collapse_text_whitespace_with_color_spans(text, spans, false, default_color)
        }
        WhiteSpace::PreLine => {
            collapse_text_whitespace_with_color_spans(text, spans, true, default_color)
        }
    }
}

fn collapse_text_whitespace_with_color_spans(
    text: &str,
    spans: &[TextColorSpan],
    keep_newlines: bool,
    default_color: Color,
) -> (String, Vec<TextColorSpan>) {
    let mut output = String::new();
    let mut output_spans = Vec::new();
    let mut previous_space = false;

    for (start, ch) in text.char_indices() {
        let end = start + ch.len_utf8();
        let color = resolve_cluster_color(start, end, default_color, spans);

        if ch == '\n' {
            if keep_newlines {
                if !output.ends_with('\n') {
                    push_colored_text(&mut output, &mut output_spans, "\n", color);
                }
                previous_space = false;
            } else if !previous_space {
                push_colored_text(&mut output, &mut output_spans, " ", color);
                previous_space = true;
            }
            continue;
        }

        if ch.is_whitespace() {
            if !previous_space {
                push_colored_text(&mut output, &mut output_spans, " ", color);
                previous_space = true;
            }
        } else {
            let mut encoded = [0u8; 4];
            let fragment = ch.encode_utf8(&mut encoded);
            push_colored_text(&mut output, &mut output_spans, fragment, color);
            previous_space = false;
        }
    }

    trim_text_and_color_spans(output, output_spans)
}

fn trim_text_and_color_spans(
    text: String,
    spans: Vec<TextColorSpan>,
) -> (String, Vec<TextColorSpan>) {
    if text.is_empty() {
        return (text, spans);
    }

    let trimmed_start = text.len() - text.trim_start().len();
    let trimmed_end = text.trim_end().len();

    if trimmed_start >= trimmed_end {
        return (String::new(), Vec::new());
    }
    if trimmed_start == 0 && trimmed_end == text.len() {
        return (text, spans);
    }

    let trimmed_text = text[trimmed_start..trimmed_end].to_string();
    let mut trimmed_spans: Vec<TextColorSpan> = Vec::new();
    for span in spans {
        let start = span.start.max(trimmed_start);
        let end = span.end.min(trimmed_end);
        if start >= end {
            continue;
        }

        let adjusted_start = start - trimmed_start;
        let adjusted_end = end - trimmed_start;
        if let Some(last) = trimmed_spans.last_mut() {
            if last.end == adjusted_start && last.color == span.color {
                last.end = adjusted_end;
                continue;
            }
        }

        trimmed_spans.push(TextColorSpan {
            start: adjusted_start,
            end: adjusted_end,
            color: span.color,
        });
    }

    (trimmed_text, trimmed_spans)
}

fn resolve_cluster_color(
    cluster_start: usize,
    cluster_end: usize,
    default_color: Color,
    spans: &[TextColorSpan],
) -> Color {
    if spans.is_empty() {
        return default_color;
    }

    let mut resolved = default_color;
    let end = cluster_end.max(cluster_start + 1);
    for span in spans {
        if cluster_start < span.end && end > span.start {
            resolved = span.color;
        }
    }
    resolved
}

fn normalize_paint_line_style(style: BorderStyle) -> BorderStyle {
    match style {
        BorderStyle::None | BorderStyle::Hidden => BorderStyle::Solid,
        other => other,
    }
}

fn contributes_collapsed_border(style: &ComputedStyle) -> bool {
    match style.border_style {
        BorderStyle::None => false,
        BorderStyle::Hidden => true,
        _ => style.border_width_pt > COLLAPSED_BORDER_EPSILON_PT,
    }
}

fn quantize_border_axis(axis_pt: f32) -> i32 {
    (axis_pt * COLLAPSED_BORDER_AXIS_SCALE).round() as i32
}

fn append_collapsed_border_candidate(
    grouped_edges: &mut BTreeMap<(BorderEdgeOrientation, i32), Vec<BorderEdgeCandidate>>,
    orientation: BorderEdgeOrientation,
    axis_pt: f32,
    start_pt: f32,
    end_pt: f32,
    style: &ComputedStyle,
    edge: PaintLineEdge,
    source_priority: u8,
    row_anchor: usize,
    col_anchor: usize,
    clip_start_pt: f32,
    clip_end_pt: f32,
) {
    if !contributes_collapsed_border(style) {
        return;
    }

    let mut clamped_start = start_pt.min(end_pt).max(clip_start_pt);
    let mut clamped_end = start_pt.max(end_pt).min(clip_end_pt);
    if clamped_end < clamped_start {
        std::mem::swap(&mut clamped_start, &mut clamped_end);
    }
    if clamped_end - clamped_start <= COLLAPSED_BORDER_EPSILON_PT {
        return;
    }

    let candidate = BorderEdgeCandidate {
        orientation,
        axis_pt,
        start_pt: clamped_start,
        end_pt: clamped_end,
        width_pt: style.border_width_pt.max(0.0),
        color: style.border_color,
        border_style: style.border_style,
        edge,
        source_priority,
        row_anchor,
        col_anchor,
    };

    grouped_edges
        .entry((orientation, quantize_border_axis(axis_pt)))
        .or_default()
        .push(candidate);
}

fn collapsed_border_candidate_wins(
    challenger: &BorderEdgeCandidate,
    current: &BorderEdgeCandidate,
) -> bool {
    debug_assert_eq!(challenger.orientation, current.orientation);

    let challenger_hidden = matches!(challenger.border_style, BorderStyle::Hidden);
    let current_hidden = matches!(current.border_style, BorderStyle::Hidden);
    if challenger_hidden != current_hidden {
        return challenger_hidden;
    }

    let challenger_none = matches!(challenger.border_style, BorderStyle::None);
    let current_none = matches!(current.border_style, BorderStyle::None);
    if challenger_none != current_none {
        return !challenger_none;
    }

    if (challenger.width_pt - current.width_pt).abs() > COLLAPSED_BORDER_EPSILON_PT {
        return challenger.width_pt > current.width_pt;
    }

    let challenger_style_rank = challenger.border_style.precedence_rank();
    let current_style_rank = current.border_style.precedence_rank();
    if challenger_style_rank != current_style_rank {
        return challenger_style_rank > current_style_rank;
    }

    if challenger.source_priority != current.source_priority {
        return challenger.source_priority > current.source_priority;
    }

    match challenger.orientation {
        BorderEdgeOrientation::Vertical => {
            if challenger.col_anchor != current.col_anchor {
                return challenger.col_anchor < current.col_anchor;
            }
            if challenger.row_anchor != current.row_anchor {
                return challenger.row_anchor < current.row_anchor;
            }
        }
        BorderEdgeOrientation::Horizontal => {
            if challenger.row_anchor != current.row_anchor {
                return challenger.row_anchor < current.row_anchor;
            }
            if challenger.col_anchor != current.col_anchor {
                return challenger.col_anchor < current.col_anchor;
            }
        }
    }

    false
}

fn collect_table_sections<'a>(
    table: &'a HtmlNode,
    table_ancestors: &[&'a HtmlNode],
) -> TableSections<'a> {
    let mut sections = TableSections::default();

    for child in &table.children {
        match child.tag_name() {
            Some("thead") => collect_section_rows(child, table_ancestors, &mut sections.header),
            Some("tbody") => collect_section_rows(child, table_ancestors, &mut sections.body),
            Some("tfoot") => collect_section_rows(child, table_ancestors, &mut sections.footer),
            Some("tr") => push_table_row(child, table_ancestors, &mut sections.body),
            _ => {}
        }
    }

    sections
}

fn collect_section_rows<'a>(
    section: &'a HtmlNode,
    table_ancestors: &[&'a HtmlNode],
    out: &mut Vec<TableRowRef<'a>>,
) {
    let mut row_ancestors = table_ancestors.to_vec();
    row_ancestors.push(section);

    for child in &section.children {
        if child.tag_name() == Some("tr") {
            push_table_row(child, &row_ancestors, out);
        }
    }
}

fn push_table_row<'a>(
    row: &'a HtmlNode,
    ancestors: &[&'a HtmlNode],
    out: &mut Vec<TableRowRef<'a>>,
) {
    let cells = row
        .children
        .iter()
        .filter_map(|child| {
            matches!(child.tag_name(), Some("td") | Some("th")).then_some(TableCellRef {
                node: child,
                colspan: parse_span_attr(child.attr("colspan")),
                rowspan: parse_span_attr(child.attr("rowspan")),
            })
        })
        .collect::<Vec<_>>();

    if cells.is_empty() {
        return;
    }

    out.push(TableRowRef {
        node: row,
        ancestors: ancestors.to_vec(),
        cells,
    });
}

fn parse_span_attr(value: Option<&str>) -> usize {
    value
        .and_then(|raw| raw.trim().parse::<usize>().ok())
        .filter(|span| *span > 0)
        .unwrap_or(1)
}

fn table_column_count(sections: &TableSections<'_>) -> usize {
    section_column_count(&sections.header)
        .max(section_column_count(&sections.body))
        .max(section_column_count(&sections.footer))
}

fn section_column_count(rows: &[TableRowRef<'_>]) -> usize {
    let layouts = build_table_row_layouts(rows, 1);
    max_layout_column_end(&layouts)
}

fn build_table_row_layouts(
    rows: &[TableRowRef<'_>],
    column_count_hint: usize,
) -> Vec<TableRowLayout> {
    let mut occupied = vec![0usize; column_count_hint.max(1)];
    let mut layouts = Vec::with_capacity(rows.len());

    for row in rows {
        let mut placements = Vec::with_capacity(row.cells.len());
        let mut cursor = 0usize;

        for (cell_index, cell) in row.cells.iter().enumerate() {
            let colspan = cell.colspan.max(1);
            let rowspan = cell.rowspan.max(1);

            let mut col_start = cursor;
            loop {
                let col_end = col_start + colspan;
                if col_end > occupied.len() {
                    occupied.resize(col_end, 0);
                }
                if occupied[col_start..col_end].iter().all(|value| *value == 0) {
                    for value in &mut occupied[col_start..col_end] {
                        *value = (*value).max(rowspan);
                    }
                    placements.push(TableCellPlacement {
                        cell_index,
                        col_start,
                        colspan,
                        rowspan,
                    });
                    cursor = col_end;
                    break;
                }
                col_start += 1;
            }
        }

        for value in &mut occupied {
            if *value > 0 {
                *value -= 1;
            }
        }

        layouts.push(TableRowLayout { placements });
    }

    layouts
}

fn max_layout_column_end(layouts: &[TableRowLayout]) -> usize {
    layouts
        .iter()
        .flat_map(|row| row.placements.iter())
        .map(|placement| placement.col_start + placement.colspan)
        .max()
        .unwrap_or(0)
}

fn resolve_style_width_pt(style: &ComputedStyle, available_width_pt: f32) -> f32 {
    resolve_declared_width_pt(style, available_width_pt).unwrap_or(available_width_pt)
}

fn resolve_declared_width_pt(style: &ComputedStyle, available_width_pt: f32) -> Option<f32> {
    if let Some(percent) = style.width_percent {
        return Some((available_width_pt * percent).max(0.0));
    }
    style.width_pt
}

fn distribute_required_width(column_slice: &mut [f32], required_total_pt: f32) {
    if column_slice.is_empty() {
        return;
    }

    let current_total = column_slice.iter().sum::<f32>();
    if required_total_pt <= current_total {
        return;
    }

    let extra_per_col = (required_total_pt - current_total) / column_slice.len() as f32;
    for width in column_slice {
        *width += extra_per_col;
    }
}

fn solve_auto_table_widths(
    min_widths: &[f32],
    preferred_widths: &[f32],
    target_width_pt: f32,
) -> Vec<f32> {
    if min_widths.is_empty() {
        return Vec::new();
    }

    let min_total = min_widths.iter().sum::<f32>();
    let preferred_total = preferred_widths.iter().sum::<f32>();

    if target_width_pt <= min_total {
        let scale = if min_total > 0.0 {
            target_width_pt / min_total
        } else {
            1.0
        };
        return min_widths
            .iter()
            .map(|width| *width * scale)
            .collect::<Vec<_>>();
    }

    if target_width_pt <= preferred_total {
        let flex_total = preferred_widths
            .iter()
            .zip(min_widths.iter())
            .map(|(preferred, min)| (preferred - min).max(0.0))
            .sum::<f32>();
        if flex_total <= 0.0 {
            return min_widths.to_vec();
        }

        let available_flex = target_width_pt - min_total;
        return min_widths
            .iter()
            .zip(preferred_widths.iter())
            .map(|(min, preferred)| {
                let column_flex = (preferred - min).max(0.0);
                min + available_flex * (column_flex / flex_total)
            })
            .collect::<Vec<_>>();
    }

    let extra = target_width_pt - preferred_total;
    let preferred_nonzero = preferred_total.max(1.0);
    preferred_widths
        .iter()
        .map(|width| width + extra * (*width / preferred_nonzero))
        .collect::<Vec<_>>()
}

fn rebalance_width_sum(widths: &mut [f32], target_width_pt: f32, min_col_width_pt: f32) {
    if widths.is_empty() {
        return;
    }

    let total = widths.iter().sum::<f32>();
    let delta = target_width_pt - total;
    if delta.abs() < 0.01 {
        return;
    }

    if delta > 0.0 {
        let share = delta / widths.len() as f32;
        for width in widths {
            *width += share;
        }
        return;
    }

    let mut shrink_left = -delta;
    let shrinkable_total = widths
        .iter()
        .map(|width| (width - min_col_width_pt).max(0.0))
        .sum::<f32>();
    if shrinkable_total <= 0.0 {
        return;
    }

    for width in widths.iter_mut() {
        if shrink_left <= 0.0 {
            break;
        }
        let shrinkable = (*width - min_col_width_pt).max(0.0);
        if shrinkable <= 0.0 {
            continue;
        }
        let shrink = (shrink_left * (shrinkable / shrinkable_total)).min(shrinkable);
        *width -= shrink;
        shrink_left -= shrink;
    }
}

fn is_token_break_char(ch: char) -> bool {
    ch.is_whitespace()
        || matches!(
            ch,
            ',' | '.' | ':' | ';' | '/' | '\\' | '-' | '–' | '—' | '|' | '(' | ')' | '[' | ']'
        )
}

fn is_thai_script_char(ch: char) -> bool {
    ('\u{0E00}'..='\u{0E7F}').contains(&ch)
}

fn has_only_inline_children(node: &HtmlNode) -> bool {
    node.children.iter().all(|child| match &child.kind {
        HtmlNodeKind::Text(_) => true,
        HtmlNodeKind::Element(tag) => {
            !is_block_tag(tag)
                || matches!(
                    tag.as_str(),
                    "span" | "strong" | "b" | "em" | "i" | "a" | "br"
                )
        }
        HtmlNodeKind::Document => true,
    })
}

fn resolve_text_link_spans(text: &str, links: &[HtmlLink]) -> Vec<TextLinkSpan> {
    let mut spans = Vec::new();
    let mut cursor = 0usize;

    for link in links {
        let text_value = link.text.trim();
        if text_value.is_empty() {
            continue;
        }
        let Some(target) = resolve_link_target(&link.href) else {
            continue;
        };

        if let Some((start, end)) = find_non_overlapping_substring(text, text_value, cursor, &spans)
            .or_else(|| find_non_overlapping_substring(text, text_value, 0, &spans))
        {
            spans.push(TextLinkSpan { start, end, target });
            cursor = end;
        }
    }

    spans.sort_by_key(|span| span.start);
    spans
}

fn resolve_link_target(href: &str) -> Option<LinkTarget> {
    let href = href.trim();
    if href.is_empty() {
        return None;
    }

    if let Some(target) = href.strip_prefix('#') {
        let target = target.trim();
        if !target.is_empty() {
            return Some(LinkTarget::Internal(target.to_string()));
        }
        return None;
    }

    Some(LinkTarget::External(href.to_string()))
}

fn find_non_overlapping_substring(
    text: &str,
    needle: &str,
    search_from: usize,
    spans: &[TextLinkSpan],
) -> Option<(usize, usize)> {
    if needle.is_empty() || search_from >= text.len() {
        return None;
    }

    let mut offset = search_from;
    while offset < text.len() {
        let relative = text[offset..].find(needle)?;
        let start = offset + relative;
        let end = start + needle.len();
        let overlaps = spans
            .iter()
            .any(|span| start < span.end && end > span.start);
        if !overlaps {
            return Some((start, end));
        }
        let next = start + 1;
        offset = next_char_boundary(text, next);
    }

    None
}

fn next_char_boundary(text: &str, mut index: usize) -> usize {
    while index < text.len() && !text.is_char_boundary(index) {
        index += 1;
    }
    index
}

fn collect_line_cluster_bounds(line: &TextLine) -> Vec<LineClusterBounds> {
    if line.glyphs.is_empty() {
        return Vec::new();
    }

    let mut clusters = Vec::<LineClusterBounds>::new();
    for glyph in &line.glyphs {
        if let Some(last) = clusters.last_mut() {
            if last.start == glyph.cluster_start && last.end == glyph.cluster_end {
                last.x_start_pt = last.x_start_pt.min(glyph.x_pt);
                last.x_end_pt = last.x_end_pt.max(glyph.x_pt);
                continue;
            }
        }

        clusters.push(LineClusterBounds {
            start: glyph.cluster_start,
            end: glyph.cluster_end,
            x_start_pt: glyph.x_pt,
            x_end_pt: glyph.x_pt,
        });
    }

    for idx in 0..clusters.len() {
        let fallback_end = line.width_pt.max(clusters[idx].x_end_pt);
        let next_start = clusters
            .get(idx + 1)
            .map(|value| value.x_start_pt)
            .unwrap_or(fallback_end);
        clusters[idx].x_end_pt = next_start.max(clusters[idx].x_end_pt + 0.5);
    }

    clusters
}

fn resolve_line_link_rects(
    line_clusters: &[LineClusterBounds],
    link_spans: &[TextLinkSpan],
) -> Vec<(f32, f32, LinkTarget)> {
    let mut rects = Vec::new();
    for span in link_spans {
        let mut min_x = f32::MAX;
        let mut max_x = f32::MIN;
        let mut found = false;

        for cluster in line_clusters {
            if cluster.end <= span.start || cluster.start >= span.end {
                continue;
            }
            found = true;
            min_x = min_x.min(cluster.x_start_pt);
            max_x = max_x.max(cluster.x_end_pt);
        }

        if found {
            rects.push((min_x, max_x, span.target.clone()));
        }
    }
    rects
}

#[derive(Debug, Clone)]
struct LoadedRasterImage {
    pixel_width: u32,
    pixel_height: u32,
    rgb_data: Vec<u8>,
    alpha_data: Option<Vec<u8>>,
}

#[derive(Debug, Clone)]
struct DecodedGlyphRasterImage {
    pixel_width: u32,
    pixel_height: u32,
    x: i16,
    y: i16,
    pixels_per_em: u16,
    rgb_data: Vec<u8>,
    alpha_data: Option<Vec<u8>>,
}

#[derive(Debug, Clone, Copy, Default)]
struct SvgPoint {
    x: f32,
    y: f32,
}

#[derive(Debug, Clone, Copy)]
enum SvgPathToken {
    Command(char),
    Number(f32),
}

#[derive(Debug, Clone, Copy)]
struct SvgBounds {
    min_x: f32,
    min_y: f32,
    max_x: f32,
    max_y: f32,
}

impl SvgTransform {
    fn identity() -> Self {
        Self {
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            e: 0.0,
            f: 0.0,
        }
    }

    fn new(a: f32, b: f32, c: f32, d: f32, e: f32, f: f32) -> Self {
        Self { a, b, c, d, e, f }
    }

    fn multiply(self, other: Self) -> Self {
        Self {
            a: self.a * other.a + self.c * other.b,
            b: self.b * other.a + self.d * other.b,
            c: self.a * other.c + self.c * other.d,
            d: self.b * other.c + self.d * other.d,
            e: self.a * other.e + self.c * other.f + self.e,
            f: self.b * other.e + self.d * other.f + self.f,
        }
    }

    fn apply(self, x: f32, y: f32) -> (f32, f32) {
        (
            self.a * x + self.c * y + self.e,
            self.b * x + self.d * y + self.f,
        )
    }

    fn average_scale(self) -> f32 {
        let sx = (self.a * self.a + self.b * self.b).sqrt();
        let sy = (self.c * self.c + self.d * self.d).sqrt();
        ((sx + sy) * 0.5).max(0.001)
    }
}

fn derive_svg_context(parent: &SvgRenderContext, node: &HtmlNode) -> SvgRenderContext {
    let node_transform = svg_attr_string(node, "transform")
        .and_then(|value| parse_svg_transform(&value))
        .unwrap_or_else(SvgTransform::identity);
    let opacity = parse_svg_opacity(node, "opacity").unwrap_or(1.0);

    SvgRenderContext {
        view_x: parent.view_x,
        view_y: parent.view_y,
        view_width: parent.view_width,
        view_height: parent.view_height,
        transform: parent.transform.multiply(node_transform),
        inherited_opacity: (parent.inherited_opacity * opacity).clamp(0.0, 1.0),
        gradients: parent.gradients.clone(),
        style: parent.style.clone(),
    }
}

fn align_inline_x(base_x: f32, available_width: f32, content_width: f32, align: TextAlign) -> f32 {
    let slack = (available_width - content_width).max(0.0);
    match align {
        TextAlign::Center => base_x + slack * 0.5,
        TextAlign::Right => base_x + slack,
        _ => base_x,
    }
}

fn resolve_asset_path(base_dir: &Path, src: &str) -> Option<PathBuf> {
    let source_path = Path::new(src);
    if source_path.is_absolute() {
        return source_path.exists().then(|| source_path.to_path_buf());
    }

    for ancestor in base_dir.ancestors() {
        let candidate = ancestor.join(source_path);
        if candidate.exists() {
            return Some(candidate);
        }
    }
    None
}

fn parse_attr_length_pt(raw: &str) -> Option<f32> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("auto") {
        return None;
    }

    if trimmed
        .chars()
        .all(|ch| ch.is_ascii_digit() || matches!(ch, '.' | '+' | '-'))
    {
        return trimmed.parse::<f32>().ok().map(|value| value * 0.75);
    }

    parse_length(trimmed, 10.5)
}

fn parse_svg_number(raw: &str) -> Option<f32> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    let value = trimmed
        .chars()
        .take_while(|ch| ch.is_ascii_digit() || matches!(ch, '.' | '+' | '-' | 'e' | 'E'))
        .collect::<String>();
    value.parse::<f32>().ok()
}

fn load_raster_image(path: &Path) -> Result<LoadedRasterImage> {
    let reader = image::ImageReader::open(path).map_err(|err| {
        LynPdfError::Pdf(format!("failed to open image '{}': {err}", path.display()))
    })?;
    let image = reader
        .with_guessed_format()
        .map_err(|err| {
            LynPdfError::Pdf(format!(
                "failed to detect image format '{}': {err}",
                path.display()
            ))
        })?
        .decode()
        .map_err(|err| {
            LynPdfError::Pdf(format!(
                "failed to decode image '{}': {err}",
                path.display()
            ))
        })?;

    let rgba = image.to_rgba8();
    let (pixel_width, pixel_height) = rgba.dimensions();
    let raw = rgba.into_raw();
    let mut rgb_data = Vec::with_capacity((pixel_width * pixel_height * 3) as usize);
    let mut alpha_data = Vec::with_capacity((pixel_width * pixel_height) as usize);
    let mut has_alpha = false;
    for chunk in raw.chunks_exact(4) {
        rgb_data.extend_from_slice(&chunk[..3]);
        alpha_data.push(chunk[3]);
        if chunk[3] < u8::MAX {
            has_alpha = true;
        }
    }

    Ok(LoadedRasterImage {
        pixel_width,
        pixel_height,
        rgb_data,
        alpha_data: has_alpha.then_some(alpha_data),
    })
}

fn decode_font_glyph_raster_image(
    font: &LoadedFont,
    glyph_id: u16,
    font_size_pt: f32,
) -> Option<DecodedGlyphRasterImage> {
    let face = Face::parse(&font.data, 0).ok()?;
    let mut requested_ppem = (font_size_pt * (96.0 / 72.0)).round() as i32;
    requested_ppem = requested_ppem.clamp(1, i32::from(u16::MAX));
    let raster = face.glyph_raster_image(GlyphId(glyph_id), requested_ppem as u16)?;

    let (pixel_width, pixel_height, rgb_data, alpha_data) = match raster.format {
        RasterImageFormat::PNG => decode_png_raster(&raster)?,
        RasterImageFormat::BitmapPremulBgra32 => decode_premul_bgra_raster(&raster)?,
        _ => return None,
    };

    Some(DecodedGlyphRasterImage {
        pixel_width,
        pixel_height,
        x: raster.x,
        y: raster.y,
        pixels_per_em: raster.pixels_per_em.max(1),
        rgb_data,
        alpha_data,
    })
}

fn decode_png_raster(
    raster: &ttf_parser::RasterGlyphImage<'_>,
) -> Option<(u32, u32, Vec<u8>, Option<Vec<u8>>)> {
    let image = image::load_from_memory_with_format(raster.data, image::ImageFormat::Png).ok()?;
    let rgba = image.to_rgba8();
    let (pixel_width, pixel_height) = rgba.dimensions();
    let raw = rgba.into_raw();
    let mut rgb_data = Vec::with_capacity((pixel_width * pixel_height * 3) as usize);
    let mut alpha_data = Vec::with_capacity((pixel_width * pixel_height) as usize);
    let mut has_alpha = false;
    for chunk in raw.chunks_exact(4) {
        rgb_data.extend_from_slice(&chunk[..3]);
        alpha_data.push(chunk[3]);
        if chunk[3] < u8::MAX {
            has_alpha = true;
        }
    }

    Some((
        pixel_width,
        pixel_height,
        rgb_data,
        has_alpha.then_some(alpha_data),
    ))
}

fn decode_premul_bgra_raster(
    raster: &ttf_parser::RasterGlyphImage<'_>,
) -> Option<(u32, u32, Vec<u8>, Option<Vec<u8>>)> {
    let pixel_width = u32::from(raster.width);
    let pixel_height = u32::from(raster.height);
    let expected_len = (pixel_width as usize)
        .saturating_mul(pixel_height as usize)
        .saturating_mul(4);
    if raster.data.len() < expected_len {
        return None;
    }

    let mut rgb_data = Vec::with_capacity((pixel_width * pixel_height * 3) as usize);
    let mut alpha_data = Vec::with_capacity((pixel_width * pixel_height) as usize);
    let mut has_alpha = false;
    for chunk in raster.data[..expected_len].chunks_exact(4) {
        let b = chunk[0];
        let g = chunk[1];
        let r = chunk[2];
        let a = chunk[3];
        let (r, g, b) = if a == 0 {
            (0, 0, 0)
        } else {
            (
                ((u32::from(r) * 255 + u32::from(a) / 2) / u32::from(a)).min(255) as u8,
                ((u32::from(g) * 255 + u32::from(a) / 2) / u32::from(a)).min(255) as u8,
                ((u32::from(b) * 255 + u32::from(a) / 2) / u32::from(a)).min(255) as u8,
            )
        };

        rgb_data.extend_from_slice(&[r, g, b]);
        alpha_data.push(a);
        if a < u8::MAX {
            has_alpha = true;
        }
    }

    Some((
        pixel_width,
        pixel_height,
        rgb_data,
        has_alpha.then_some(alpha_data),
    ))
}

fn cluster_contains_emoji(text: &str) -> bool {
    text.chars().any(|ch| {
        matches!(
            ch as u32,
            0x1F1E6..=0x1F1FF | 0x1F300..=0x1FAFF | 0x2600..=0x26FF | 0x2700..=0x27BF
        )
    })
}

fn resolve_svg_view_box(node: &HtmlNode) -> Result<(f32, f32, f32, f32)> {
    if let Some(view_box) = node.attr("viewbox").or_else(|| node.attr("viewBox")) {
        let numbers = parse_svg_number_list(view_box);
        if numbers.len() >= 4 {
            let width = numbers[2].abs().max(1.0);
            let height = numbers[3].abs().max(1.0);
            return Ok((numbers[0], numbers[1], width, height));
        }
    }

    let width = node
        .attr("width")
        .and_then(parse_svg_number)
        .unwrap_or(100.0)
        .abs()
        .max(1.0);
    let height = node
        .attr("height")
        .and_then(parse_svg_number)
        .unwrap_or(100.0)
        .abs()
        .max(1.0);
    Ok((0.0, 0.0, width, height))
}

fn svg_attr_string(node: &HtmlNode, name: &str) -> Option<String> {
    let key = name.to_ascii_lowercase();
    if let Some(value) = node.attr(&key) {
        let trimmed = value.trim();
        if !trimmed.is_empty() {
            return Some(trimmed.to_string());
        }
    }

    let style = node.attr("style")?;
    let declarations = parse_inline_style_declarations(style);
    declarations
        .get(&key)
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn svg_attr_number(node: &HtmlNode, name: &str) -> Option<f32> {
    svg_attr_string(node, name).and_then(|value| parse_svg_number(&value))
}

fn parse_inline_style_declarations(style: &str) -> HashMap<String, String> {
    let mut declarations = HashMap::new();
    for part in style.split(';') {
        let Some((name, value)) = part.split_once(':') else {
            continue;
        };
        declarations.insert(name.trim().to_ascii_lowercase(), value.trim().to_string());
    }
    declarations
}

fn parse_svg_number_list(value: &str) -> Vec<f32> {
    let normalized = value.replace(',', " ");
    normalized
        .split_whitespace()
        .filter_map(parse_svg_number)
        .collect::<Vec<_>>()
}

fn parse_svg_length_value(value: &str) -> Option<SvgLengthValue> {
    let trimmed = value.trim();
    if let Some(percent) = trimmed.strip_suffix('%') {
        let parsed = percent.trim().parse::<f32>().ok()?;
        return Some(SvgLengthValue::Percent((parsed / 100.0).clamp(-10.0, 10.0)));
    }

    parse_svg_number(trimmed).map(SvgLengthValue::Number)
}

fn parse_svg_opacity(node: &HtmlNode, attr: &str) -> Option<f32> {
    svg_attr_string(node, attr)
        .and_then(|value| parse_opacity_value(&value))
        .map(|value| value.clamp(0.0, 1.0))
}

fn parse_opacity_value(value: &str) -> Option<f32> {
    let trimmed = value.trim();
    if let Some(percent) = trimmed.strip_suffix('%') {
        let parsed = percent.trim().parse::<f32>().ok()?;
        return Some(parsed / 100.0);
    }
    trimmed.parse::<f32>().ok()
}

fn parse_svg_transform(value: &str) -> Option<SvgTransform> {
    let mut transform = SvgTransform::identity();
    let mut changed = false;
    let mut index = 0usize;
    let bytes = value.as_bytes();

    while index < value.len() {
        while index < value.len()
            && ((bytes[index] as char).is_ascii_whitespace() || bytes[index] == b',')
        {
            index += 1;
        }
        if index >= value.len() {
            break;
        }

        let name_start = index;
        while index < value.len() && (bytes[index] as char).is_ascii_alphabetic() {
            index += 1;
        }
        if name_start == index {
            break;
        }
        let name = value[name_start..index].trim().to_ascii_lowercase();

        while index < value.len() && (bytes[index] as char).is_ascii_whitespace() {
            index += 1;
        }
        if index >= value.len() || bytes[index] != b'(' {
            break;
        }
        index += 1;

        let args_start = index;
        let mut depth = 1i32;
        while index < value.len() && depth > 0 {
            match bytes[index] as char {
                '(' => depth += 1,
                ')' => depth -= 1,
                _ => {}
            }
            index += 1;
        }
        if depth != 0 || index <= args_start {
            break;
        }

        let args = value[args_start..index - 1].trim();
        if let Some(parsed) = parse_svg_transform_function(&name, args) {
            transform = transform.multiply(parsed);
            changed = true;
        }
    }

    changed.then_some(transform)
}

fn parse_svg_transform_function(name: &str, args: &str) -> Option<SvgTransform> {
    let values = parse_svg_number_list(args);
    match name {
        "matrix" if values.len() >= 6 => Some(SvgTransform::new(
            values[0], values[1], values[2], values[3], values[4], values[5],
        )),
        "translate" if !values.is_empty() => {
            let tx = values[0];
            let ty = values.get(1).copied().unwrap_or(0.0);
            Some(SvgTransform::new(1.0, 0.0, 0.0, 1.0, tx, ty))
        }
        "scale" if !values.is_empty() => {
            let sx = values[0];
            let sy = values.get(1).copied().unwrap_or(sx);
            Some(SvgTransform::new(sx, 0.0, 0.0, sy, 0.0, 0.0))
        }
        "rotate" if !values.is_empty() => {
            let angle = values[0].to_radians();
            let cos = angle.cos();
            let sin = angle.sin();
            let rotation = SvgTransform::new(cos, sin, -sin, cos, 0.0, 0.0);
            if values.len() >= 3 {
                let cx = values[1];
                let cy = values[2];
                let translate_to_origin = SvgTransform::new(1.0, 0.0, 0.0, 1.0, -cx, -cy);
                let translate_back = SvgTransform::new(1.0, 0.0, 0.0, 1.0, cx, cy);
                Some(
                    translate_back
                        .multiply(rotation)
                        .multiply(translate_to_origin),
                )
            } else {
                Some(rotation)
            }
        }
        "skewx" if !values.is_empty() => {
            let tan = values[0].to_radians().tan();
            Some(SvgTransform::new(1.0, 0.0, tan, 1.0, 0.0, 0.0))
        }
        "skewy" if !values.is_empty() => {
            let tan = values[0].to_radians().tan();
            Some(SvgTransform::new(1.0, tan, 0.0, 1.0, 0.0, 0.0))
        }
        _ => None,
    }
}

fn parse_css_color_extended(value: &str) -> Option<Color> {
    let trimmed = value.trim();
    if let Some(color) = parse_color(trimmed) {
        return Some(color);
    }

    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("rgb(") || lower.starts_with("rgba(") {
        let start = lower.find('(')? + 1;
        let end = lower.rfind(')')?;
        if end <= start {
            return None;
        }
        let parts = lower[start..end]
            .split(',')
            .map(str::trim)
            .collect::<Vec<_>>();
        if parts.len() < 3 {
            return None;
        }
        let parse_channel = |raw: &str| -> Option<u8> {
            if let Some(percent) = raw.strip_suffix('%') {
                let value = percent.trim().parse::<f32>().ok()?;
                return Some((value.clamp(0.0, 100.0) * 2.55).round() as u8);
            }
            let value = raw.parse::<f32>().ok()?;
            Some(value.clamp(0.0, 255.0).round() as u8)
        };
        let r = parse_channel(parts[0])?;
        let g = parse_channel(parts[1])?;
        let b = parse_channel(parts[2])?;
        return Some(Color::rgb_u8(r, g, b));
    }

    None
}

fn collect_svg_gradient_defs(node: &HtmlNode) -> HashMap<String, SvgLinearGradientDef> {
    let mut gradients = HashMap::new();
    collect_svg_gradient_defs_inner(node, &mut gradients);
    gradients
}

fn collect_svg_gradient_defs_inner(
    node: &HtmlNode,
    gradients: &mut HashMap<String, SvgLinearGradientDef>,
) {
    if node.tag_name() == Some("lineargradient") {
        if let Some(id) = node
            .attr("id")
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            let mut stops = Vec::<SvgGradientStop>::new();
            for child in &node.children {
                if child.tag_name() != Some("stop") {
                    continue;
                }
                let offset = svg_attr_string(child, "offset")
                    .as_deref()
                    .and_then(parse_gradient_offset)
                    .unwrap_or(0.0)
                    .clamp(0.0, 1.0);
                let color = svg_attr_string(child, "stop-color")
                    .as_deref()
                    .and_then(parse_css_color_extended)
                    .unwrap_or(Color::rgb_u8(0, 0, 0));
                let stop_opacity = parse_svg_opacity(child, "stop-opacity").unwrap_or(1.0)
                    * parse_svg_opacity(child, "opacity").unwrap_or(1.0);
                stops.push(SvgGradientStop {
                    offset,
                    color,
                    opacity: stop_opacity.clamp(0.0, 1.0),
                });
            }

            stops.sort_by(|a, b| {
                a.offset
                    .partial_cmp(&b.offset)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            if stops.is_empty() {
                stops.push(SvgGradientStop {
                    offset: 0.0,
                    color: Color::rgb_u8(0, 0, 0),
                    opacity: 1.0,
                });
                stops.push(SvgGradientStop {
                    offset: 1.0,
                    color: Color::rgb_u8(0, 0, 0),
                    opacity: 1.0,
                });
            } else {
                if stops[0].offset > 0.0 {
                    let mut first = stops[0].clone();
                    first.offset = 0.0;
                    stops.insert(0, first);
                }
                if stops.last().map(|stop| stop.offset).unwrap_or(1.0) < 1.0 {
                    let mut last = stops.last().cloned().unwrap_or(SvgGradientStop {
                        offset: 1.0,
                        color: Color::rgb_u8(0, 0, 0),
                        opacity: 1.0,
                    });
                    last.offset = 1.0;
                    stops.push(last);
                }
            }

            let units = svg_attr_string(node, "gradientunits")
                .map(|value| value.to_ascii_lowercase())
                .as_deref()
                .map(|value| {
                    if value == "userspaceonuse" {
                        SvgGradientUnits::UserSpaceOnUse
                    } else {
                        SvgGradientUnits::ObjectBoundingBox
                    }
                })
                .unwrap_or(SvgGradientUnits::ObjectBoundingBox);

            let x1 = svg_attr_string(node, "x1")
                .as_deref()
                .and_then(parse_svg_length_value)
                .unwrap_or(SvgLengthValue::Percent(0.0));
            let y1 = svg_attr_string(node, "y1")
                .as_deref()
                .and_then(parse_svg_length_value)
                .unwrap_or(SvgLengthValue::Percent(0.0));
            let x2 = svg_attr_string(node, "x2")
                .as_deref()
                .and_then(parse_svg_length_value)
                .unwrap_or(SvgLengthValue::Percent(1.0));
            let y2 = svg_attr_string(node, "y2")
                .as_deref()
                .and_then(parse_svg_length_value)
                .unwrap_or(SvgLengthValue::Percent(0.0));
            let gradient_transform = svg_attr_string(node, "gradienttransform")
                .as_deref()
                .and_then(parse_svg_transform)
                .unwrap_or_else(SvgTransform::identity);

            gradients.insert(
                id.to_string(),
                SvgLinearGradientDef {
                    x1,
                    y1,
                    x2,
                    y2,
                    units,
                    gradient_transform,
                    stops,
                },
            );
        }
    }

    for child in &node.children {
        collect_svg_gradient_defs_inner(child, gradients);
    }
}

fn parse_gradient_offset(raw: &str) -> Option<f32> {
    let value = raw.trim();
    if let Some(percent) = value.strip_suffix('%') {
        let parsed = percent.trim().parse::<f32>().ok()?;
        return Some(parsed / 100.0);
    }
    value.parse::<f32>().ok()
}

fn parse_svg_paint(
    value: &str,
    gradients: &HashMap<String, SvgLinearGradientDef>,
    svg: &SvgRenderContext,
    shape_bounds: Option<SvgBounds>,
) -> Option<SvgPaint> {
    let trimmed = value.trim();
    if trimmed.eq_ignore_ascii_case("none") {
        return None;
    }

    let lower = trimmed.to_ascii_lowercase();
    if lower.starts_with("url(#") && lower.ends_with(')') {
        let id = lower
            .trim_start_matches("url(#")
            .trim_end_matches(')')
            .trim();
        let gradient = gradients.get(id)?;
        return resolve_svg_linear_gradient(gradient, svg, shape_bounds)
            .map(SvgPaint::LinearGradient);
    }

    parse_css_color_extended(trimmed).map(SvgPaint::Solid)
}

fn resolve_svg_linear_gradient(
    gradient: &SvgLinearGradientDef,
    svg: &SvgRenderContext,
    shape_bounds: Option<SvgBounds>,
) -> Option<SvgLinearGradientPaint> {
    let (mut x1, mut y1, mut x2, mut y2) = match gradient.units {
        SvgGradientUnits::ObjectBoundingBox => {
            let bounds = shape_bounds?;
            (
                resolve_length_in_bounds(gradient.x1, bounds.min_x, bounds.max_x),
                resolve_length_in_bounds(gradient.y1, bounds.min_y, bounds.max_y),
                resolve_length_in_bounds(gradient.x2, bounds.min_x, bounds.max_x),
                resolve_length_in_bounds(gradient.y2, bounds.min_y, bounds.max_y),
            )
        }
        SvgGradientUnits::UserSpaceOnUse => (
            resolve_length_in_user_space(gradient.x1, svg.view_x, svg.view_width),
            resolve_length_in_user_space(gradient.y1, svg.view_y, svg.view_height),
            resolve_length_in_user_space(gradient.x2, svg.view_x, svg.view_width),
            resolve_length_in_user_space(gradient.y2, svg.view_y, svg.view_height),
        ),
    };

    (x1, y1) = gradient.gradient_transform.apply(x1, y1);
    (x2, y2) = gradient.gradient_transform.apply(x2, y2);
    (x1, y1) = svg.transform.apply(x1, y1);
    (x2, y2) = svg.transform.apply(x2, y2);

    if (x2 - x1).abs() < 0.001 && (y2 - y1).abs() < 0.001 {
        x2 += 0.001;
    }

    Some(SvgLinearGradientPaint {
        x1,
        y1,
        x2,
        y2,
        stops: gradient.stops.clone(),
    })
}

fn resolve_length_in_bounds(value: SvgLengthValue, min: f32, max: f32) -> f32 {
    match value {
        SvgLengthValue::Percent(percent) => min + (max - min) * percent,
        SvgLengthValue::Number(number) => min + (max - min) * number,
    }
}

fn resolve_length_in_user_space(value: SvgLengthValue, origin: f32, extent: f32) -> f32 {
    match value {
        SvgLengthValue::Percent(percent) => origin + extent * percent,
        SvgLengthValue::Number(number) => number,
    }
}

fn svg_paint(
    node: &HtmlNode,
    attr: &str,
    default: Option<SvgPaint>,
    gradients: &HashMap<String, SvgLinearGradientDef>,
    svg: &SvgRenderContext,
    shape_bounds: Option<SvgBounds>,
) -> Option<SvgPaint> {
    let Some(raw) = svg_attr_string(node, attr) else {
        return default;
    };

    parse_svg_paint(&raw, gradients, svg, shape_bounds).or(default)
}

fn svg_paint_sample_color(paint: SvgPaint) -> Color {
    match paint {
        SvgPaint::Solid(color) => color,
        SvgPaint::LinearGradient(gradient) => {
            if gradient.stops.is_empty() {
                return Color::rgb_u8(0, 0, 0);
            }
            sample_gradient_color(&gradient.stops, 0.5)
        }
    }
}

fn sample_gradient_color(stops: &[SvgGradientStop], t: f32) -> Color {
    if stops.is_empty() {
        return Color::rgb_u8(0, 0, 0);
    }

    if stops.len() == 1 {
        return stops[0].color;
    }

    let clamped_t = t.clamp(0.0, 1.0);
    let mut previous = &stops[0];
    for stop in stops.iter().skip(1) {
        if clamped_t <= stop.offset {
            let span = (stop.offset - previous.offset).max(0.0001);
            let local_t = ((clamped_t - previous.offset) / span).clamp(0.0, 1.0);
            return Color {
                r: previous.color.r + (stop.color.r - previous.color.r) * local_t,
                g: previous.color.g + (stop.color.g - previous.color.g) * local_t,
                b: previous.color.b + (stop.color.b - previous.color.b) * local_t,
                a: previous.color.a + (stop.color.a - previous.color.a) * local_t,
            };
        }
        previous = stop;
    }
    previous.color
}

fn svg_line_cap(node: &HtmlNode) -> SvgLineCap {
    match svg_attr_string(node, "stroke-linecap")
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "round" => SvgLineCap::Round,
        "square" => SvgLineCap::Square,
        _ => SvgLineCap::Butt,
    }
}

fn svg_line_join(node: &HtmlNode) -> SvgLineJoin {
    match svg_attr_string(node, "stroke-linejoin")
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "round" => SvgLineJoin::Round,
        "bevel" => SvgLineJoin::Bevel,
        _ => SvgLineJoin::Miter,
    }
}

fn svg_stroke_width_pt(node: &HtmlNode, svg: &SvgRenderContext) -> f32 {
    let width_unit = svg_attr_number(node, "stroke-width").unwrap_or(1.0);
    let scale = svg.transform.average_scale();
    (width_unit * scale).max(0.1)
}

fn build_svg_rect_path(node: &HtmlNode, svg: &SvgRenderContext) -> Option<PaintOp> {
    let x = svg_attr_number(node, "x").unwrap_or(0.0);
    let y = svg_attr_number(node, "y").unwrap_or(0.0);
    let width = svg_attr_number(node, "width")?.abs();
    let height = svg_attr_number(node, "height")?.abs();
    if width <= 0.0 || height <= 0.0 {
        return None;
    }

    let rx = svg_attr_number(node, "rx")
        .unwrap_or(0.0)
        .abs()
        .min(width * 0.5);
    let ry = svg_attr_number(node, "ry")
        .unwrap_or(rx)
        .abs()
        .min(height * 0.5);

    let commands = if rx > 0.0 || ry > 0.0 {
        rounded_rect_commands(x, y, width, height, rx, ry)
    } else {
        vec![
            SvgPathCommand::MoveTo { x, y },
            SvgPathCommand::LineTo { x: x + width, y },
            SvgPathCommand::LineTo {
                x: x + width,
                y: y + height,
            },
            SvgPathCommand::LineTo { x, y: y + height },
            SvgPathCommand::ClosePath,
        ]
    };

    Some(svg_path_paint_op(
        commands,
        node,
        svg,
        Some(SvgPaint::Solid(Color::rgb_u8(0, 0, 0))),
        None,
    ))
}

fn build_svg_circle_path(node: &HtmlNode, svg: &SvgRenderContext) -> Option<PaintOp> {
    let cx = svg_attr_number(node, "cx")?;
    let cy = svg_attr_number(node, "cy")?;
    let r = svg_attr_number(node, "r")?.abs();
    if r <= 0.0 {
        return None;
    }

    let kappa = 0.552_284_8;
    let ox = r * kappa;
    let oy = r * kappa;
    let commands = vec![
        SvgPathCommand::MoveTo { x: cx + r, y: cy },
        SvgPathCommand::CurveTo {
            cx1: cx + r,
            cy1: cy + oy,
            cx2: cx + ox,
            cy2: cy + r,
            x: cx,
            y: cy + r,
        },
        SvgPathCommand::CurveTo {
            cx1: cx - ox,
            cy1: cy + r,
            cx2: cx - r,
            cy2: cy + oy,
            x: cx - r,
            y: cy,
        },
        SvgPathCommand::CurveTo {
            cx1: cx - r,
            cy1: cy - oy,
            cx2: cx - ox,
            cy2: cy - r,
            x: cx,
            y: cy - r,
        },
        SvgPathCommand::CurveTo {
            cx1: cx + ox,
            cy1: cy - r,
            cx2: cx + r,
            cy2: cy - oy,
            x: cx + r,
            y: cy,
        },
        SvgPathCommand::ClosePath,
    ];

    Some(svg_path_paint_op(
        commands,
        node,
        svg,
        Some(SvgPaint::Solid(Color::rgb_u8(0, 0, 0))),
        None,
    ))
}

fn build_svg_ellipse_path(node: &HtmlNode, svg: &SvgRenderContext) -> Option<PaintOp> {
    let cx = svg_attr_number(node, "cx")?;
    let cy = svg_attr_number(node, "cy")?;
    let rx = svg_attr_number(node, "rx")?.abs();
    let ry = svg_attr_number(node, "ry")?.abs();
    if rx <= 0.0 || ry <= 0.0 {
        return None;
    }

    let kappa = 0.552_284_8;
    let ox = rx * kappa;
    let oy = ry * kappa;
    let commands = vec![
        SvgPathCommand::MoveTo { x: cx + rx, y: cy },
        SvgPathCommand::CurveTo {
            cx1: cx + rx,
            cy1: cy + oy,
            cx2: cx + ox,
            cy2: cy + ry,
            x: cx,
            y: cy + ry,
        },
        SvgPathCommand::CurveTo {
            cx1: cx - ox,
            cy1: cy + ry,
            cx2: cx - rx,
            cy2: cy + oy,
            x: cx - rx,
            y: cy,
        },
        SvgPathCommand::CurveTo {
            cx1: cx - rx,
            cy1: cy - oy,
            cx2: cx - ox,
            cy2: cy - ry,
            x: cx,
            y: cy - ry,
        },
        SvgPathCommand::CurveTo {
            cx1: cx + ox,
            cy1: cy - ry,
            cx2: cx + rx,
            cy2: cy - oy,
            x: cx + rx,
            y: cy,
        },
        SvgPathCommand::ClosePath,
    ];

    Some(svg_path_paint_op(
        commands,
        node,
        svg,
        Some(SvgPaint::Solid(Color::rgb_u8(0, 0, 0))),
        None,
    ))
}

fn build_svg_line_path(node: &HtmlNode, svg: &SvgRenderContext) -> Option<PaintOp> {
    let x1 = svg_attr_number(node, "x1")?;
    let y1 = svg_attr_number(node, "y1")?;
    let x2 = svg_attr_number(node, "x2")?;
    let y2 = svg_attr_number(node, "y2")?;
    let commands = vec![
        SvgPathCommand::MoveTo { x: x1, y: y1 },
        SvgPathCommand::LineTo { x: x2, y: y2 },
    ];

    let mut op = svg_path_paint_op(
        commands,
        node,
        svg,
        None,
        Some(SvgPaint::Solid(Color::rgb_u8(0, 0, 0))),
    );
    if let PaintOp::SvgPath { fill, .. } = &mut op {
        *fill = None;
    }
    Some(op)
}

fn build_svg_poly_path(node: &HtmlNode, svg: &SvgRenderContext, close: bool) -> Option<PaintOp> {
    let points_raw = svg_attr_string(node, "points")?;
    let numbers = parse_svg_number_list(&points_raw);
    if numbers.len() < 4 {
        return None;
    }

    let mut points = Vec::new();
    for pair in numbers.chunks_exact(2) {
        points.push(SvgPoint {
            x: pair[0],
            y: pair[1],
        });
    }
    if points.len() < 2 {
        return None;
    }

    let mut commands = vec![SvgPathCommand::MoveTo {
        x: points[0].x,
        y: points[0].y,
    }];
    for point in points.iter().skip(1) {
        commands.push(SvgPathCommand::LineTo {
            x: point.x,
            y: point.y,
        });
    }
    if close {
        commands.push(SvgPathCommand::ClosePath);
    }

    let default_fill = close.then_some(SvgPaint::Solid(Color::rgb_u8(0, 0, 0)));
    Some(svg_path_paint_op(
        commands,
        node,
        svg,
        default_fill,
        Some(SvgPaint::Solid(Color::rgb_u8(0, 0, 0))),
    ))
}

fn build_svg_path(node: &HtmlNode, svg: &SvgRenderContext) -> Option<PaintOp> {
    let data = svg_attr_string(node, "d")?;
    let commands = parse_svg_path_commands(&data)?;
    Some(svg_path_paint_op(
        commands,
        node,
        svg,
        Some(SvgPaint::Solid(Color::rgb_u8(0, 0, 0))),
        None,
    ))
}

fn svg_path_paint_op(
    commands: Vec<SvgPathCommand>,
    node: &HtmlNode,
    svg: &SvgRenderContext,
    default_fill: Option<SvgPaint>,
    default_stroke: Option<SvgPaint>,
) -> PaintOp {
    let source_bounds = svg_path_bounds(&commands);
    let transformed = transform_svg_commands(commands, svg.transform);
    let node_opacity = svg.inherited_opacity.clamp(0.0, 1.0);
    let fill_opacity =
        (node_opacity * parse_svg_opacity(node, "fill-opacity").unwrap_or(1.0)).clamp(0.0, 1.0);
    let stroke_opacity =
        (node_opacity * parse_svg_opacity(node, "stroke-opacity").unwrap_or(1.0)).clamp(0.0, 1.0);

    let fill = if fill_opacity <= 0.0 {
        None
    } else {
        svg_paint(
            node,
            "fill",
            default_fill,
            &svg.gradients,
            svg,
            source_bounds,
        )
    };
    let stroke = if stroke_opacity <= 0.0 {
        None
    } else {
        svg_paint(
            node,
            "stroke",
            default_stroke,
            &svg.gradients,
            svg,
            source_bounds,
        )
    };

    PaintOp::SvgPath {
        commands: transformed,
        fill,
        stroke,
        fill_opacity,
        stroke_opacity,
        stroke_width: svg_stroke_width_pt(node, svg),
        line_cap: svg_line_cap(node),
        line_join: svg_line_join(node),
    }
}

fn transform_svg_commands(
    commands: Vec<SvgPathCommand>,
    transform: SvgTransform,
) -> Vec<SvgPathCommand> {
    commands
        .into_iter()
        .map(|command| match command {
            SvgPathCommand::MoveTo { x, y } => SvgPathCommand::MoveTo {
                x: transform.apply(x, y).0,
                y: transform.apply(x, y).1,
            },
            SvgPathCommand::LineTo { x, y } => SvgPathCommand::LineTo {
                x: transform.apply(x, y).0,
                y: transform.apply(x, y).1,
            },
            SvgPathCommand::CurveTo {
                cx1,
                cy1,
                cx2,
                cy2,
                x,
                y,
            } => SvgPathCommand::CurveTo {
                cx1: transform.apply(cx1, cy1).0,
                cy1: transform.apply(cx1, cy1).1,
                cx2: transform.apply(cx2, cy2).0,
                cy2: transform.apply(cx2, cy2).1,
                x: transform.apply(x, y).0,
                y: transform.apply(x, y).1,
            },
            SvgPathCommand::ClosePath => SvgPathCommand::ClosePath,
        })
        .collect::<Vec<_>>()
}

fn svg_path_bounds(commands: &[SvgPathCommand]) -> Option<SvgBounds> {
    let mut min_x = f32::INFINITY;
    let mut min_y = f32::INFINITY;
    let mut max_x = f32::NEG_INFINITY;
    let mut max_y = f32::NEG_INFINITY;
    let mut seen = false;

    let mut update = |x: f32, y: f32| {
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
        seen = true;
    };

    for command in commands {
        match *command {
            SvgPathCommand::MoveTo { x, y } | SvgPathCommand::LineTo { x, y } => update(x, y),
            SvgPathCommand::CurveTo {
                cx1,
                cy1,
                cx2,
                cy2,
                x,
                y,
            } => {
                update(cx1, cy1);
                update(cx2, cy2);
                update(x, y);
            }
            SvgPathCommand::ClosePath => {}
        }
    }

    seen.then_some(SvgBounds {
        min_x,
        min_y,
        max_x,
        max_y,
    })
}

fn rounded_rect_commands(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    rx: f32,
    ry: f32,
) -> Vec<SvgPathCommand> {
    let k = 0.552_284_8;
    let cx = rx * k;
    let cy = ry * k;
    vec![
        SvgPathCommand::MoveTo { x: x + rx, y },
        SvgPathCommand::LineTo {
            x: x + width - rx,
            y,
        },
        SvgPathCommand::CurveTo {
            cx1: x + width - rx + cx,
            cy1: y,
            cx2: x + width,
            cy2: y + ry - cy,
            x: x + width,
            y: y + ry,
        },
        SvgPathCommand::LineTo {
            x: x + width,
            y: y + height - ry,
        },
        SvgPathCommand::CurveTo {
            cx1: x + width,
            cy1: y + height - ry + cy,
            cx2: x + width - rx + cx,
            cy2: y + height,
            x: x + width - rx,
            y: y + height,
        },
        SvgPathCommand::LineTo {
            x: x + rx,
            y: y + height,
        },
        SvgPathCommand::CurveTo {
            cx1: x + rx - cx,
            cy1: y + height,
            cx2: x,
            cy2: y + height - ry + cy,
            x,
            y: y + height - ry,
        },
        SvgPathCommand::LineTo { x, y: y + ry },
        SvgPathCommand::CurveTo {
            cx1: x,
            cy1: y + ry - cy,
            cx2: x + rx - cx,
            cy2: y,
            x: x + rx,
            y,
        },
        SvgPathCommand::ClosePath,
    ]
}

fn parse_svg_path_commands(data: &str) -> Option<Vec<SvgPathCommand>> {
    let tokens = tokenize_svg_path_data(data);
    if tokens.is_empty() {
        return None;
    }

    let mut commands = Vec::new();
    let mut index = 0usize;
    let mut active_command = None::<char>;
    let mut current = SvgPoint::default();
    let mut subpath_start = SvgPoint::default();
    let mut prev_quad_ctrl = None::<SvgPoint>;
    let mut prev_cubic_ctrl = None::<SvgPoint>;

    while index < tokens.len() {
        if let SvgPathToken::Command(command) = tokens[index] {
            active_command = Some(command);
            index += 1;
        }

        let Some(command) = active_command else {
            break;
        };
        let is_relative = command.is_ascii_lowercase();
        let command = command.to_ascii_uppercase();

        let next_number = |idx: &mut usize| -> Option<f32> {
            let value = match tokens.get(*idx)? {
                SvgPathToken::Number(value) => *value,
                SvgPathToken::Command(_) => return None,
            };
            *idx += 1;
            Some(value)
        };

        let read_point = |idx: &mut usize| -> Option<SvgPoint> {
            let x = next_number(idx)?;
            let y = next_number(idx)?;
            Some(SvgPoint { x, y })
        };

        match command {
            'M' => {
                let Some(mut point) = read_point(&mut index) else {
                    break;
                };
                if is_relative {
                    point.x += current.x;
                    point.y += current.y;
                }
                commands.push(SvgPathCommand::MoveTo {
                    x: point.x,
                    y: point.y,
                });
                current = point;
                subpath_start = point;
                prev_quad_ctrl = None;
                prev_cubic_ctrl = None;

                while let Some(mut point) = read_point(&mut index) {
                    if is_relative {
                        point.x += current.x;
                        point.y += current.y;
                    }
                    commands.push(SvgPathCommand::LineTo {
                        x: point.x,
                        y: point.y,
                    });
                    current = point;
                }
            }
            'L' => {
                while let Some(mut point) = read_point(&mut index) {
                    if is_relative {
                        point.x += current.x;
                        point.y += current.y;
                    }
                    commands.push(SvgPathCommand::LineTo {
                        x: point.x,
                        y: point.y,
                    });
                    current = point;
                }
                prev_quad_ctrl = None;
                prev_cubic_ctrl = None;
            }
            'H' => {
                while let Some(value) = next_number(&mut index) {
                    let x = if is_relative {
                        current.x + value
                    } else {
                        value
                    };
                    commands.push(SvgPathCommand::LineTo { x, y: current.y });
                    current.x = x;
                }
                prev_quad_ctrl = None;
                prev_cubic_ctrl = None;
            }
            'V' => {
                while let Some(value) = next_number(&mut index) {
                    let y = if is_relative {
                        current.y + value
                    } else {
                        value
                    };
                    commands.push(SvgPathCommand::LineTo { x: current.x, y });
                    current.y = y;
                }
                prev_quad_ctrl = None;
                prev_cubic_ctrl = None;
            }
            'C' => {
                while let Some(mut c1) = read_point(&mut index) {
                    let Some(mut c2) = read_point(&mut index) else {
                        break;
                    };
                    let Some(mut end) = read_point(&mut index) else {
                        break;
                    };

                    if is_relative {
                        c1.x += current.x;
                        c1.y += current.y;
                        c2.x += current.x;
                        c2.y += current.y;
                        end.x += current.x;
                        end.y += current.y;
                    }

                    commands.push(SvgPathCommand::CurveTo {
                        cx1: c1.x,
                        cy1: c1.y,
                        cx2: c2.x,
                        cy2: c2.y,
                        x: end.x,
                        y: end.y,
                    });
                    current = end;
                    prev_cubic_ctrl = Some(c2);
                    prev_quad_ctrl = None;
                }
            }
            'S' => {
                while let Some(mut c2) = read_point(&mut index) {
                    let Some(mut end) = read_point(&mut index) else {
                        break;
                    };

                    if is_relative {
                        c2.x += current.x;
                        c2.y += current.y;
                        end.x += current.x;
                        end.y += current.y;
                    }

                    let c1 = if let Some(previous) = prev_cubic_ctrl {
                        SvgPoint {
                            x: current.x * 2.0 - previous.x,
                            y: current.y * 2.0 - previous.y,
                        }
                    } else {
                        current
                    };

                    commands.push(SvgPathCommand::CurveTo {
                        cx1: c1.x,
                        cy1: c1.y,
                        cx2: c2.x,
                        cy2: c2.y,
                        x: end.x,
                        y: end.y,
                    });
                    current = end;
                    prev_cubic_ctrl = Some(c2);
                    prev_quad_ctrl = None;
                }
            }
            'Q' => {
                while let Some(mut ctrl) = read_point(&mut index) {
                    let Some(mut end) = read_point(&mut index) else {
                        break;
                    };

                    if is_relative {
                        ctrl.x += current.x;
                        ctrl.y += current.y;
                        end.x += current.x;
                        end.y += current.y;
                    }

                    let c1 = SvgPoint {
                        x: current.x + (ctrl.x - current.x) * (2.0 / 3.0),
                        y: current.y + (ctrl.y - current.y) * (2.0 / 3.0),
                    };
                    let c2 = SvgPoint {
                        x: end.x + (ctrl.x - end.x) * (2.0 / 3.0),
                        y: end.y + (ctrl.y - end.y) * (2.0 / 3.0),
                    };
                    commands.push(SvgPathCommand::CurveTo {
                        cx1: c1.x,
                        cy1: c1.y,
                        cx2: c2.x,
                        cy2: c2.y,
                        x: end.x,
                        y: end.y,
                    });
                    current = end;
                    prev_quad_ctrl = Some(ctrl);
                    prev_cubic_ctrl = Some(c2);
                }
            }
            'T' => {
                while let Some(mut end) = read_point(&mut index) {
                    if is_relative {
                        end.x += current.x;
                        end.y += current.y;
                    }

                    let ctrl = if let Some(previous) = prev_quad_ctrl {
                        SvgPoint {
                            x: current.x * 2.0 - previous.x,
                            y: current.y * 2.0 - previous.y,
                        }
                    } else {
                        current
                    };

                    let c1 = SvgPoint {
                        x: current.x + (ctrl.x - current.x) * (2.0 / 3.0),
                        y: current.y + (ctrl.y - current.y) * (2.0 / 3.0),
                    };
                    let c2 = SvgPoint {
                        x: end.x + (ctrl.x - end.x) * (2.0 / 3.0),
                        y: end.y + (ctrl.y - end.y) * (2.0 / 3.0),
                    };
                    commands.push(SvgPathCommand::CurveTo {
                        cx1: c1.x,
                        cy1: c1.y,
                        cx2: c2.x,
                        cy2: c2.y,
                        x: end.x,
                        y: end.y,
                    });
                    current = end;
                    prev_quad_ctrl = Some(ctrl);
                    prev_cubic_ctrl = Some(c2);
                }
            }
            'Z' => {
                commands.push(SvgPathCommand::ClosePath);
                current = subpath_start;
                prev_quad_ctrl = None;
                prev_cubic_ctrl = None;
            }
            _ => {
                break;
            }
        }
    }

    (!commands.is_empty()).then_some(commands)
}

fn tokenize_svg_path_data(data: &str) -> Vec<SvgPathToken> {
    let bytes = data.as_bytes();
    let mut tokens = Vec::new();
    let mut idx = 0usize;

    while idx < bytes.len() {
        let ch = bytes[idx] as char;
        if ch.is_ascii_alphabetic() {
            tokens.push(SvgPathToken::Command(ch));
            idx += 1;
            continue;
        }
        if ch.is_ascii_whitespace() || ch == ',' {
            idx += 1;
            continue;
        }

        let start = idx;
        let mut end = idx;
        let mut seen_exp = false;
        while end < bytes.len() {
            let current = bytes[end] as char;
            if current.is_ascii_whitespace() || current == ',' || current.is_ascii_alphabetic() {
                break;
            }
            if (current == '+' || current == '-') && end > start {
                let prev = bytes[end - 1] as char;
                if prev != 'e' && prev != 'E' {
                    break;
                }
            }
            if (current == 'e' || current == 'E') && seen_exp {
                break;
            }
            if current == 'e' || current == 'E' {
                seen_exp = true;
            }
            end += 1;
        }

        if let Ok(value) = data[start..end].parse::<f32>() {
            tokens.push(SvgPathToken::Number(value));
        }
        idx = end.max(idx + 1);
    }

    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    fn element(tag: &str, attrs: &[(&str, &str)], children: Vec<HtmlNode>) -> HtmlNode {
        let mut map = std::collections::HashMap::new();
        for (key, value) in attrs {
            map.insert((*key).to_string(), (*value).to_string());
        }
        HtmlNode {
            kind: HtmlNodeKind::Element(tag.to_string()),
            attrs: map,
            children,
        }
    }

    fn edge_candidate(
        orientation: BorderEdgeOrientation,
        width_pt: f32,
        border_style: BorderStyle,
        row_anchor: usize,
        col_anchor: usize,
        source_priority: u8,
    ) -> BorderEdgeCandidate {
        BorderEdgeCandidate {
            orientation,
            axis_pt: 10.0,
            start_pt: 0.0,
            end_pt: 20.0,
            width_pt,
            color: Color::rgb_u8(0, 0, 0),
            border_style,
            edge: PaintLineEdge::Top,
            source_priority,
            row_anchor,
            col_anchor,
        }
    }

    #[test]
    fn rowspan_layout_shifts_cells_to_free_columns() {
        let row1 = element(
            "tr",
            &[],
            vec![
                element("td", &[("rowspan", "2")], Vec::new()),
                element("td", &[], Vec::new()),
            ],
        );
        let row2 = element("tr", &[], vec![element("td", &[], Vec::new())]);

        let rows = vec![
            TableRowRef {
                node: &row1,
                ancestors: Vec::new(),
                cells: vec![
                    TableCellRef {
                        node: &row1.children[0],
                        colspan: 1,
                        rowspan: 2,
                    },
                    TableCellRef {
                        node: &row1.children[1],
                        colspan: 1,
                        rowspan: 1,
                    },
                ],
            },
            TableRowRef {
                node: &row2,
                ancestors: Vec::new(),
                cells: vec![TableCellRef {
                    node: &row2.children[0],
                    colspan: 1,
                    rowspan: 1,
                }],
            },
        ];

        let layouts = build_table_row_layouts(&rows, 2);
        assert_eq!(layouts.len(), 2);
        assert_eq!(layouts[0].placements[0].col_start, 0);
        assert_eq!(layouts[0].placements[1].col_start, 1);
        assert_eq!(layouts[1].placements[0].col_start, 1);
    }

    #[test]
    fn auto_table_solver_interpolates_between_min_and_preferred() {
        let min_widths = vec![50.0, 70.0];
        let preferred_widths = vec![150.0, 130.0];
        let result = solve_auto_table_widths(&min_widths, &preferred_widths, 240.0);
        assert_eq!(result.len(), 2);
        assert!(result[0] > 50.0 && result[0] < 150.0);
        assert!(result[1] > 70.0 && result[1] < 130.0);
        let total = result.iter().sum::<f32>();
        assert!((total - 240.0).abs() < 0.01);
    }

    #[test]
    fn collapsed_border_hidden_wins_conflict() {
        let hidden = edge_candidate(
            BorderEdgeOrientation::Vertical,
            0.0,
            BorderStyle::Hidden,
            0,
            1,
            BORDER_SOURCE_CELL,
        );
        let visible = edge_candidate(
            BorderEdgeOrientation::Vertical,
            4.0,
            BorderStyle::Solid,
            0,
            0,
            BORDER_SOURCE_CELL,
        );
        assert!(collapsed_border_candidate_wins(&hidden, &visible));
        assert!(!collapsed_border_candidate_wins(&visible, &hidden));
    }

    #[test]
    fn collapsed_vertical_tie_prefers_left_cell() {
        let left = edge_candidate(
            BorderEdgeOrientation::Vertical,
            2.0,
            BorderStyle::Solid,
            0,
            0,
            BORDER_SOURCE_CELL,
        );
        let right = edge_candidate(
            BorderEdgeOrientation::Vertical,
            2.0,
            BorderStyle::Solid,
            0,
            1,
            BORDER_SOURCE_CELL,
        );
        assert!(collapsed_border_candidate_wins(&left, &right));
        assert!(!collapsed_border_candidate_wins(&right, &left));
    }

    #[test]
    fn collapsed_horizontal_tie_prefers_top_cell() {
        let top = edge_candidate(
            BorderEdgeOrientation::Horizontal,
            2.0,
            BorderStyle::Solid,
            0,
            0,
            BORDER_SOURCE_CELL,
        );
        let bottom = edge_candidate(
            BorderEdgeOrientation::Horizontal,
            2.0,
            BorderStyle::Solid,
            1,
            0,
            BORDER_SOURCE_CELL,
        );
        assert!(collapsed_border_candidate_wins(&top, &bottom));
        assert!(!collapsed_border_candidate_wins(&bottom, &top));
    }

    #[test]
    fn resolves_inline_link_spans_from_text() {
        let text = "1. Introduction 2. Architecture";
        let links = vec![
            HtmlLink {
                href: "#intro".to_string(),
                text: "Introduction".to_string(),
            },
            HtmlLink {
                href: "https://example.com/arch".to_string(),
                text: "Architecture".to_string(),
            },
        ];

        let spans = resolve_text_link_spans(text, &links);
        assert_eq!(spans.len(), 2);
        assert_eq!(&text[spans[0].start..spans[0].end], "Introduction");
        assert_eq!(&text[spans[1].start..spans[1].end], "Architecture");

        match &spans[0].target {
            LinkTarget::Internal(target) => assert_eq!(target, "intro"),
            _ => panic!("expected internal link"),
        }
        match &spans[1].target {
            LinkTarget::External(target) => assert_eq!(target, "https://example.com/arch"),
            _ => panic!("expected external link"),
        }
    }

    #[test]
    fn parses_svg_transform_chain_in_svg_order() {
        let transform = parse_svg_transform("translate(10 5) scale(2 3)").unwrap();
        let (x, y) = transform.apply(4.0, 6.0);

        // For SVG lists, translate then scale yields matrix T * S,
        // which applies scale first then translate when mapped to points.
        assert!((x - 18.0).abs() < 0.001);
        assert!((y - 23.0).abs() < 0.001);
    }

    #[test]
    fn derives_svg_context_with_transform_and_opacity() {
        let parent = SvgRenderContext {
            view_x: 0.0,
            view_y: 0.0,
            view_width: 100.0,
            view_height: 100.0,
            transform: SvgTransform::identity(),
            inherited_opacity: 0.5,
            gradients: HashMap::new(),
            style: ComputedStyle::root("sans-serif", 12.0),
        };
        let node = element(
            "g",
            &[
                ("transform", "translate(10 5) scale(2)"),
                ("opacity", "0.6"),
            ],
            Vec::new(),
        );

        let context = derive_svg_context(&parent, &node);
        let (x, y) = context.transform.apply(3.0, 4.0);

        assert!((x - 16.0).abs() < 0.001);
        assert!((y - 13.0).abs() < 0.001);
        assert!((context.inherited_opacity - 0.3).abs() < 0.001);
    }
}
