use crate::error::{LynPdfError, Result};
use crate::fonts::{FontKey, FontRegistry, LoadedFont};
use crate::layout::{
    LayoutDocument, LinkTarget, PaintLineEdge, PaintOp, SvgGradientStop, SvgLineCap, SvgLineJoin,
    SvgLinearGradientPaint, SvgPaint, SvgPathCommand,
};
use crate::style::{BorderStyle, Color};
use crate::types::DocumentMetadata;
use flate2::write::ZlibEncoder;
use flate2::Compression;
use image::{codecs::jpeg::JpegEncoder, ExtendedColorType};
use std::collections::HashMap;
use std::io::Write;

const JPEG_IMAGE_QUALITY: u8 = 84;

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
struct CidKey {
    glyph_id: u16,
    unicode: String,
}

#[derive(Debug, Clone)]
struct FontUsage {
    resource_name: String,
    cids: Vec<CidKey>,
    cid_lookup: HashMap<CidKey, u16>,
}

#[derive(Debug, Clone, Copy)]
struct AnchorDestination {
    page_index: usize,
    y: f32,
}

#[derive(Debug, Clone)]
struct PageXObjectResource {
    name: String,
    object_id: usize,
}

#[derive(Debug, Clone)]
struct PagePatternResource {
    name: String,
    object_id: usize,
}

#[derive(Debug, Clone)]
struct PageExtGStateResource {
    name: String,
    object_id: usize,
}

#[derive(Debug, Clone, Default)]
struct PageResources {
    xobjects: Vec<PageXObjectResource>,
    patterns: Vec<PagePatternResource>,
    ext_gstates: Vec<PageExtGStateResource>,
}

pub fn emit_pdf(
    layout: &LayoutDocument,
    fonts: &FontRegistry,
    metadata: &DocumentMetadata,
) -> Result<Vec<u8>> {
    let usages = collect_font_usage(layout, fonts);
    let mut pdf = PdfBuilder::new();
    let catalog_id = pdf.reserve();
    let pages_id = pdf.reserve();

    let mut font_object_ids = HashMap::new();
    for (font_key, usage) in &usages {
        let type0_id = emit_font_objects(&mut pdf, fonts.get(*font_key), usage)?;
        font_object_ids.insert(*font_key, type0_id);
    }

    let page_ids = (0..layout.pages.len())
        .map(|_| pdf.reserve())
        .collect::<Vec<_>>();
    let anchor_destinations = collect_anchor_destinations(layout);

    for (page_index, page) in layout.pages.iter().enumerate() {
        let (content, resources) = build_page_content(&mut pdf, layout, page, &usages)?;
        let content_id = pdf.add_stream("".to_string(), &content);
        let annotation_ids =
            build_page_annotations(&mut pdf, layout, page, &page_ids, &anchor_destinations);
        let annots = if annotation_ids.is_empty() {
            String::new()
        } else {
            let refs = annotation_ids
                .iter()
                .map(|id| format!("{id} 0 R"))
                .collect::<Vec<_>>()
                .join(" ");
            format!(" /Annots [{refs}]")
        };

        pdf.set(page_ids[page_index], format!(
            "<< /Type /Page /Parent {pages_id} 0 R /MediaBox [0 0 {} {}] /Resources {} /Contents {content_id} 0 R{annots} >>",
            fmt(layout.output_page_size.width_pt),
            fmt(layout.output_page_size.height_pt),
            resource_dict(&usages, &font_object_ids, &resources)
        ).into_bytes());
    }

    let kids = page_ids
        .iter()
        .map(|id| format!("{id} 0 R"))
        .collect::<Vec<_>>()
        .join(" ");
    pdf.set(
        pages_id,
        format!(
            "<< /Type /Pages /Kids [{kids}] /Count {} >>",
            page_ids.len()
        )
        .into_bytes(),
    );
    pdf.set(
        catalog_id,
        format!("<< /Type /Catalog /Pages {pages_id} 0 R >>").into_bytes(),
    );

    let info_id = emit_info_dictionary(&mut pdf, metadata);

    Ok(pdf.finish(catalog_id, info_id))
}

fn collect_font_usage(
    layout: &LayoutDocument,
    fonts: &FontRegistry,
) -> HashMap<FontKey, FontUsage> {
    let mut usages = HashMap::new();
    for (idx, (font_key, _)) in fonts.iter().enumerate() {
        usages.insert(
            font_key,
            FontUsage {
                resource_name: format!("F{}", idx + 1),
                cids: Vec::new(),
                cid_lookup: HashMap::new(),
            },
        );
    }

    for page in &layout.pages {
        for op in &page.ops {
            if let PaintOp::Glyph {
                font_key,
                glyph_id,
                cluster_text,
                ..
            } = op
            {
                let usage = usages.get_mut(font_key).unwrap();
                let key = CidKey {
                    glyph_id: *glyph_id,
                    unicode: cluster_text.clone(),
                };
                if !usage.cid_lookup.contains_key(&key) {
                    let cid = (usage.cids.len() + 1).min(u16::MAX as usize) as u16;
                    usage.cids.push(key.clone());
                    usage.cid_lookup.insert(key, cid);
                }
            }
        }
    }

    usages.retain(|_, usage| !usage.cids.is_empty());
    usages
}

fn emit_font_objects(pdf: &mut PdfBuilder, font: &LoadedFont, usage: &FontUsage) -> Result<usize> {
    let font_file_id = pdf.add_stream("/Subtype /TrueType".to_string(), &font.data);
    let cid_to_gid = build_cid_to_gid_map(usage);
    let cid_to_gid_id = pdf.add_stream("".to_string(), &cid_to_gid);
    let to_unicode = build_to_unicode_cmap(usage);
    let to_unicode_id = pdf.add_stream("".to_string(), to_unicode.as_bytes());
    let descriptor_id = pdf.add(format!(
        "<< /Type /FontDescriptor /FontName /{} /Flags 4 /FontBBox [{} {} {} {}] /ItalicAngle 0 /Ascent {} /Descent {} /CapHeight {} /StemV 80 /FontFile2 {font_file_id} 0 R >>",
        pdf_name(&font.family),
        font.bbox[0],
        font.bbox[1],
        font.bbox[2],
        font.bbox[3],
        font.ascender.round() as i32,
        font.descender.round() as i32,
        font.ascender.round() as i32,
    ).into_bytes());
    let descendant_id = pdf.add(format!(
        "<< /Type /Font /Subtype /CIDFontType2 /BaseFont /{} /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> /FontDescriptor {descriptor_id} 0 R /CIDToGIDMap {cid_to_gid_id} 0 R /W {} >>",
        pdf_name(&font.family),
        build_width_array(font, usage)
    ).into_bytes());
    let type0_id = pdf.add(format!(
        "<< /Type /Font /Subtype /Type0 /BaseFont /{} /Encoding /Identity-H /DescendantFonts [{descendant_id} 0 R] /ToUnicode {to_unicode_id} 0 R >>",
        pdf_name(&font.family)
    ).into_bytes());

    Ok(type0_id)
}

fn build_page_content(
    pdf: &mut PdfBuilder,
    layout: &LayoutDocument,
    page: &crate::layout::LayoutPage,
    usages: &HashMap<FontKey, FontUsage>,
) -> Result<(Vec<u8>, PageResources)> {
    let mut content = String::new();
    let mut resources = PageResources::default();
    let output_page_height_pt = layout.output_page_size.height_pt;
    let scale = layout.content_scale;
    let scaled = |value: f32| value * scale;

    for op in &page.ops {
        match op {
            PaintOp::Rect {
                x,
                y,
                width,
                height,
                fill,
                stroke,
                stroke_width,
            } => {
                let scaled_x = scaled(*x);
                let scaled_y = scaled(*y);
                let scaled_width = scaled(*width);
                let scaled_height = scaled(*height);
                let pdf_y = output_page_height_pt - scaled_y - scaled_height;
                if let Some(fill) = fill {
                    content.push_str("q\n");
                    if fill.a < 0.999 {
                        let gstate_name = add_ext_gstate(pdf, &mut resources, Some(fill.a), None);
                        content.push_str(&format!("/{gstate_name} gs\n"));
                    }
                    push_fill_color(&mut content, *fill);
                    content.push_str(&format!(
                        "{} {} {} {} re f\nQ\n",
                        fmt(scaled_x),
                        fmt(pdf_y),
                        fmt(scaled_width),
                        fmt(scaled_height)
                    ));
                }
                if let Some(stroke) = stroke {
                    content.push_str("q\n");
                    if stroke.a < 0.999 {
                        let gstate_name = add_ext_gstate(pdf, &mut resources, None, Some(stroke.a));
                        content.push_str(&format!("/{gstate_name} gs\n"));
                    }
                    push_stroke_color(&mut content, *stroke);
                    content.push_str(&format!("{} w\n", fmt(scaled(*stroke_width))));
                    content.push_str(&format!(
                        "{} {} {} {} re S\nQ\n",
                        fmt(scaled_x),
                        fmt(pdf_y),
                        fmt(scaled_width),
                        fmt(scaled_height)
                    ));
                }
            }
            PaintOp::Glyph {
                font_key,
                glyph_id,
                cluster_text,
                font_size_pt,
                color,
                x,
                baseline_y,
                y_offset_pt,
                ..
            } => {
                let usage = usages
                    .get(font_key)
                    .ok_or_else(|| LynPdfError::Pdf("missing font usage".to_string()))?;
                let cid = usage
                    .cid_lookup
                    .get(&CidKey {
                        glyph_id: *glyph_id,
                        unicode: cluster_text.clone(),
                    })
                    .ok_or_else(|| LynPdfError::Pdf("missing glyph CID".to_string()))?;
                let scaled_x = scaled(*x);
                let scaled_baseline = scaled(*baseline_y);
                let scaled_y_offset = scaled(*y_offset_pt);
                let pdf_y = output_page_height_pt - scaled_baseline + scaled_y_offset;
                content.push_str("q\n");
                if color.a < 0.999 {
                    let gstate_name = add_ext_gstate(pdf, &mut resources, Some(color.a), None);
                    content.push_str(&format!("/{gstate_name} gs\n"));
                }
                content.push_str("BT\n");
                push_fill_color(&mut content, *color);
                content.push_str(&format!(
                    "/{} {} Tf\n1 0 0 1 {} {} Tm\n<{}> Tj\nET\nQ\n",
                    usage.resource_name,
                    fmt(scaled(*font_size_pt)),
                    fmt(scaled_x),
                    fmt(pdf_y),
                    hex_u16(*cid)
                ));
            }
            PaintOp::Line {
                x1,
                y1,
                x2,
                y2,
                color,
                width,
                border_style,
                edge,
            } => {
                emit_styled_line(
                    &mut content,
                    output_page_height_pt,
                    scaled(*x1),
                    scaled(*y1),
                    scaled(*x2),
                    scaled(*y2),
                    *color,
                    scaled(*width),
                    *border_style,
                    *edge,
                    pdf,
                    &mut resources,
                );
            }
            PaintOp::SvgPath {
                commands,
                fill,
                stroke,
                fill_opacity,
                stroke_opacity,
                stroke_width,
                line_cap,
                line_join,
            } => {
                emit_svg_path(
                    pdf,
                    &mut content,
                    &mut resources,
                    output_page_height_pt,
                    scale,
                    commands,
                    fill,
                    stroke,
                    *fill_opacity,
                    *stroke_opacity,
                    *stroke_width,
                    *line_cap,
                    *line_join,
                );
            }
            PaintOp::Image {
                x,
                y,
                width,
                height,
                pixel_width,
                pixel_height,
                rgb_data,
                alpha_data,
            } => {
                if *width <= 0.0 || *height <= 0.0 || *pixel_width == 0 || *pixel_height == 0 {
                    continue;
                }

                let image_name = format!("Im{}", resources.xobjects.len() + 1);
                let image_object_id = emit_image_xobject(
                    pdf,
                    *pixel_width,
                    *pixel_height,
                    rgb_data,
                    alpha_data.as_deref(),
                );
                resources.xobjects.push(PageXObjectResource {
                    name: image_name.clone(),
                    object_id: image_object_id,
                });

                let scaled_x = scaled(*x);
                let scaled_y = scaled(*y);
                let scaled_width = scaled(*width);
                let scaled_height = scaled(*height);
                let pdf_y = output_page_height_pt - scaled_y - scaled_height;
                content.push_str("q\n");
                content.push_str(&format!(
                    "{} 0 0 {} {} {} cm\n/{} Do\nQ\n",
                    fmt(scaled_width),
                    fmt(scaled_height),
                    fmt(scaled_x),
                    fmt(pdf_y),
                    image_name
                ));
            }
            PaintOp::Link { .. } | PaintOp::Anchor { .. } => {}
        }
    }
    Ok((content.into_bytes(), resources))
}

fn collect_anchor_destinations(layout: &LayoutDocument) -> HashMap<String, AnchorDestination> {
    let mut destinations = HashMap::new();
    let scale = layout.content_scale;
    for (page_index, page) in layout.pages.iter().enumerate() {
        for op in &page.ops {
            if let PaintOp::Anchor { name, y } = op {
                destinations
                    .entry(name.clone())
                    .or_insert(AnchorDestination {
                        page_index,
                        y: *y * scale,
                    });
            }
        }
    }
    destinations
}

fn build_page_annotations(
    pdf: &mut PdfBuilder,
    layout: &LayoutDocument,
    page: &crate::layout::LayoutPage,
    page_ids: &[usize],
    anchor_destinations: &HashMap<String, AnchorDestination>,
) -> Vec<usize> {
    let mut annotation_ids = Vec::new();
    let scale = layout.content_scale;
    let page_width = layout.output_page_size.width_pt;
    let page_height = layout.output_page_size.height_pt;

    for op in &page.ops {
        let PaintOp::Link {
            x,
            y,
            width,
            height,
            target,
        } = op
        else {
            continue;
        };

        if *width <= 0.0 || *height <= 0.0 {
            continue;
        }

        let scaled_x = *x * scale;
        let scaled_y = *y * scale;
        let scaled_width = *width * scale;
        let scaled_height = *height * scale;
        let x1 = scaled_x.max(0.0);
        let x2 = (scaled_x + scaled_width).min(page_width);
        let y1 = (page_height - (scaled_y + scaled_height)).max(0.0);
        let y2 = (page_height - scaled_y).min(page_height);
        if x2 - x1 <= 0.5 || y2 - y1 <= 0.5 {
            continue;
        }

        let action = match target {
            LinkTarget::External(uri) => {
                format!("/A << /S /URI /URI {} >>", pdf_literal_string(uri))
            }
            LinkTarget::Internal(anchor_name) => {
                let Some(destination) = anchor_destinations.get(anchor_name) else {
                    continue;
                };
                let destination_page_id = page_ids[destination.page_index];
                let destination_top = (page_height - destination.y).clamp(0.0, page_height);
                format!(
                    "/A << /S /GoTo /D [{destination_page_id} 0 R /XYZ null {} null] >>",
                    fmt(destination_top)
                )
            }
        };

        let annotation_id = pdf.add(
            format!(
                "<< /Type /Annot /Subtype /Link /Rect [{} {} {} {}] /Border [0 0 0] {action} >>",
                fmt(x1),
                fmt(y1),
                fmt(x2),
                fmt(y2)
            )
            .into_bytes(),
        );
        annotation_ids.push(annotation_id);
    }

    annotation_ids
}

fn emit_info_dictionary(pdf: &mut PdfBuilder, metadata: &DocumentMetadata) -> Option<usize> {
    let mut fields = Vec::new();

    if let Some(title) = metadata
        .title
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        fields.push(format!("/Title {}", pdf_text_string(title)));
    }
    if let Some(author) = metadata
        .author
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        fields.push(format!("/Author {}", pdf_text_string(author)));
    }
    if let Some(subject) = metadata
        .subject
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        fields.push(format!("/Subject {}", pdf_text_string(subject)));
    }
    if let Some(keywords) = metadata
        .keywords
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        fields.push(format!("/Keywords {}", pdf_text_string(keywords)));
    }
    if let Some(creator) = metadata
        .creator
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        fields.push(format!("/Creator {}", pdf_text_string(creator)));
    }
    if let Some(producer) = metadata
        .producer
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        fields.push(format!("/Producer {}", pdf_text_string(producer)));
    }

    if fields.is_empty() {
        return None;
    }

    Some(pdf.add(format!("<< {} >>", fields.join(" ")).into_bytes()))
}

fn emit_svg_path(
    pdf: &mut PdfBuilder,
    content: &mut String,
    resources: &mut PageResources,
    page_height_pt: f32,
    coordinate_scale: f32,
    commands: &[SvgPathCommand],
    fill: &Option<SvgPaint>,
    stroke: &Option<SvgPaint>,
    fill_opacity: f32,
    stroke_opacity: f32,
    stroke_width: f32,
    line_cap: SvgLineCap,
    line_join: SvgLineJoin,
) {
    if commands.is_empty() {
        return;
    }

    let has_fill = fill.is_some() && fill_opacity > 0.0;
    let has_stroke = stroke.is_some() && stroke_opacity > 0.0;
    if !has_fill && !has_stroke {
        return;
    }

    if let Some(fill_paint) = fill {
        if fill_opacity > 0.0 {
            content.push_str("q\n");

            match fill_paint {
                SvgPaint::Solid(fill_color) => {
                    let effective_fill_opacity = (fill_opacity * fill_color.a).clamp(0.0, 1.0);
                    if effective_fill_opacity > 0.001 {
                        if effective_fill_opacity < 0.999 {
                            let gstate_name =
                                add_ext_gstate(pdf, resources, Some(effective_fill_opacity), None);
                            content.push_str(&format!("/{gstate_name} gs\n"));
                        }
                        push_fill_color(content, *fill_color);
                        emit_svg_path_geometry(content, page_height_pt, coordinate_scale, commands);
                        content.push_str("f\n");
                    }
                }
                SvgPaint::LinearGradient(gradient) => {
                    if fill_opacity < 0.999 {
                        let gstate_name = add_ext_gstate(pdf, resources, Some(fill_opacity), None);
                        content.push_str(&format!("/{gstate_name} gs\n"));
                    }
                    let pattern_name = add_linear_gradient_pattern(
                        pdf,
                        resources,
                        gradient,
                        page_height_pt,
                        coordinate_scale,
                    );
                    content.push_str("/Pattern cs\n");
                    content.push_str(&format!("/{pattern_name} scn\n"));
                    emit_svg_path_geometry(content, page_height_pt, coordinate_scale, commands);
                    content.push_str("f\n");
                }
            }
            content.push_str("Q\n");
        }
    }

    if let Some(stroke_paint) = stroke {
        if stroke_opacity <= 0.0 {
            return;
        }

        content.push_str("q\n");

        match stroke_paint {
            SvgPaint::Solid(color) => {
                let effective_stroke_opacity = (stroke_opacity * color.a).clamp(0.0, 1.0);
                if effective_stroke_opacity <= 0.001 {
                    content.push_str("Q\n");
                    return;
                }
                if effective_stroke_opacity < 0.999 {
                    let gstate_name =
                        add_ext_gstate(pdf, resources, None, Some(effective_stroke_opacity));
                    content.push_str(&format!("/{gstate_name} gs\n"));
                }
                push_stroke_color(content, *color)
            }
            SvgPaint::LinearGradient(gradient) => {
                if stroke_opacity < 0.999 {
                    let gstate_name = add_ext_gstate(pdf, resources, None, Some(stroke_opacity));
                    content.push_str(&format!("/{gstate_name} gs\n"));
                }
                let pattern_name = add_linear_gradient_pattern(
                    pdf,
                    resources,
                    gradient,
                    page_height_pt,
                    coordinate_scale,
                );
                content.push_str("/Pattern CS\n");
                content.push_str(&format!("/{pattern_name} SCN\n"));
            }
        }
        content.push_str(&format!(
            "{} w\n",
            fmt((stroke_width * coordinate_scale).max(0.1))
        ));
        let cap = match line_cap {
            SvgLineCap::Butt => 0,
            SvgLineCap::Round => 1,
            SvgLineCap::Square => 2,
        };
        let join = match line_join {
            SvgLineJoin::Miter => 0,
            SvgLineJoin::Round => 1,
            SvgLineJoin::Bevel => 2,
        };
        content.push_str(&format!("{cap} J\n{join} j\n"));

        emit_svg_path_geometry(content, page_height_pt, coordinate_scale, commands);
        content.push_str("S\nQ\n");
    }
}

fn emit_svg_path_geometry(
    content: &mut String,
    page_height_pt: f32,
    coordinate_scale: f32,
    commands: &[SvgPathCommand],
) {
    for command in commands {
        match command {
            SvgPathCommand::MoveTo { x, y } => {
                let scaled_x = *x * coordinate_scale;
                let scaled_y = *y * coordinate_scale;
                content.push_str(&format!(
                    "{} {} m\n",
                    fmt(scaled_x),
                    fmt(page_height_pt - scaled_y)
                ));
            }
            SvgPathCommand::LineTo { x, y } => {
                let scaled_x = *x * coordinate_scale;
                let scaled_y = *y * coordinate_scale;
                content.push_str(&format!(
                    "{} {} l\n",
                    fmt(scaled_x),
                    fmt(page_height_pt - scaled_y)
                ));
            }
            SvgPathCommand::CurveTo {
                cx1,
                cy1,
                cx2,
                cy2,
                x,
                y,
            } => {
                let scaled_cx1 = *cx1 * coordinate_scale;
                let scaled_cy1 = *cy1 * coordinate_scale;
                let scaled_cx2 = *cx2 * coordinate_scale;
                let scaled_cy2 = *cy2 * coordinate_scale;
                let scaled_x = *x * coordinate_scale;
                let scaled_y = *y * coordinate_scale;
                content.push_str(&format!(
                    "{} {} {} {} {} {} c\n",
                    fmt(scaled_cx1),
                    fmt(page_height_pt - scaled_cy1),
                    fmt(scaled_cx2),
                    fmt(page_height_pt - scaled_cy2),
                    fmt(scaled_x),
                    fmt(page_height_pt - scaled_y)
                ));
            }
            SvgPathCommand::ClosePath => content.push_str("h\n"),
        }
    }
}

fn add_ext_gstate(
    pdf: &mut PdfBuilder,
    resources: &mut PageResources,
    fill_alpha: Option<f32>,
    stroke_alpha: Option<f32>,
) -> String {
    let fill_alpha = fill_alpha.unwrap_or(1.0).clamp(0.0, 1.0);
    let stroke_alpha = stroke_alpha.unwrap_or(1.0).clamp(0.0, 1.0);
    let object_id = pdf.add(
        format!(
            "<< /Type /ExtGState /ca {} /CA {} >>",
            fmt(fill_alpha),
            fmt(stroke_alpha)
        )
        .into_bytes(),
    );
    let name = format!("Gs{}", resources.ext_gstates.len() + 1);
    resources.ext_gstates.push(PageExtGStateResource {
        name: name.clone(),
        object_id,
    });
    name
}

fn add_linear_gradient_pattern(
    pdf: &mut PdfBuilder,
    resources: &mut PageResources,
    gradient: &SvgLinearGradientPaint,
    page_height_pt: f32,
    coordinate_scale: f32,
) -> String {
    let function_id = build_gradient_function_object(pdf, &gradient.stops);
    let x1 = gradient.x1 * coordinate_scale;
    let y1 = gradient.y1 * coordinate_scale;
    let x2 = gradient.x2 * coordinate_scale;
    let y2 = gradient.y2 * coordinate_scale;
    let shading_id = pdf.add(
        format!(
            "<< /ShadingType 2 /ColorSpace /DeviceRGB /Coords [{} {} {} {}] /Function {function_id} 0 R /Extend [true true] >>",
            fmt(x1),
            fmt(page_height_pt - y1),
            fmt(x2),
            fmt(page_height_pt - y2)
        )
        .into_bytes(),
    );

    let pattern_id = pdf
        .add(format!("<< /Type /Pattern /PatternType 2 /Shading {shading_id} 0 R >>").into_bytes());
    let name = format!("P{}", resources.patterns.len() + 1);
    resources.patterns.push(PagePatternResource {
        name: name.clone(),
        object_id: pattern_id,
    });
    name
}

fn build_gradient_function_object(pdf: &mut PdfBuilder, stops: &[SvgGradientStop]) -> usize {
    let normalized_stops = if stops.is_empty() {
        vec![SvgGradientStop {
            offset: 0.0,
            color: Color::rgb_u8(0, 0, 0),
            opacity: 1.0,
        }]
    } else {
        stops.to_vec()
    };

    if normalized_stops.len() == 1 {
        let color = gradient_stop_color(&normalized_stops[0]);
        return pdf.add(
            format!(
                "<< /FunctionType 2 /Domain [0 1] /C0 [{} {} {}] /C1 [{} {} {}] /N 1 >>",
                fmt(color.r),
                fmt(color.g),
                fmt(color.b),
                fmt(color.r),
                fmt(color.g),
                fmt(color.b)
            )
            .into_bytes(),
        );
    }

    if normalized_stops.len() == 2 {
        let c0 = gradient_stop_color(&normalized_stops[0]);
        let c1 = gradient_stop_color(&normalized_stops[1]);
        return pdf.add(
            format!(
                "<< /FunctionType 2 /Domain [0 1] /C0 [{} {} {}] /C1 [{} {} {}] /N 1 >>",
                fmt(c0.r),
                fmt(c0.g),
                fmt(c0.b),
                fmt(c1.r),
                fmt(c1.g),
                fmt(c1.b)
            )
            .into_bytes(),
        );
    }

    let mut function_refs = Vec::new();
    let mut bounds = Vec::new();
    let mut encode = Vec::new();

    for pair in normalized_stops.windows(2) {
        let c0 = gradient_stop_color(&pair[0]);
        let c1 = gradient_stop_color(&pair[1]);
        let function_id = pdf.add(
            format!(
                "<< /FunctionType 2 /Domain [0 1] /C0 [{} {} {}] /C1 [{} {} {}] /N 1 >>",
                fmt(c0.r),
                fmt(c0.g),
                fmt(c0.b),
                fmt(c1.r),
                fmt(c1.g),
                fmt(c1.b)
            )
            .into_bytes(),
        );
        function_refs.push(format!("{function_id} 0 R"));
        encode.push("0 1".to_string());
    }

    for stop in normalized_stops
        .iter()
        .skip(1)
        .take(normalized_stops.len().saturating_sub(2))
    {
        bounds.push(fmt(stop.offset.clamp(0.0, 1.0)));
    }

    let bounds_part = if bounds.is_empty() {
        "".to_string()
    } else {
        format!(" /Bounds [{}]", bounds.join(" "))
    };

    pdf.add(
        format!(
            "<< /FunctionType 3 /Domain [0 1] /Functions [{}]{bounds_part} /Encode [{}] >>",
            function_refs.join(" "),
            encode.join(" ")
        )
        .into_bytes(),
    )
}

fn gradient_stop_color(stop: &SvgGradientStop) -> Color {
    Color {
        r: (stop.color.r * stop.opacity).clamp(0.0, 1.0),
        g: (stop.color.g * stop.opacity).clamp(0.0, 1.0),
        b: (stop.color.b * stop.opacity).clamp(0.0, 1.0),
        a: (stop.color.a * stop.opacity).clamp(0.0, 1.0),
    }
}

fn emit_image_xobject(
    pdf: &mut PdfBuilder,
    pixel_width: u32,
    pixel_height: u32,
    rgb_data: &[u8],
    alpha_data: Option<&[u8]>,
) -> usize {
    if alpha_data.is_none() {
        if let Some(encoded_jpeg) =
            encode_rgb_to_jpeg(pixel_width, pixel_height, rgb_data, JPEG_IMAGE_QUALITY)
        {
            let dict = format!(
                "/Type /XObject /Subtype /Image /Width {pixel_width} /Height {pixel_height} /ColorSpace /DeviceRGB /BitsPerComponent 8"
            );
            return pdf.add_stream_with_filter(dict, &encoded_jpeg, "/DCTDecode");
        }
    }

    let smask_id = alpha_data.map(|alpha| {
        let dict = format!(
            "/Type /XObject /Subtype /Image /Width {pixel_width} /Height {pixel_height} /ColorSpace /DeviceGray /BitsPerComponent 8"
        );
        pdf.add_stream(dict, alpha)
    });

    let smask_ref = smask_id
        .map(|id| format!(" /SMask {id} 0 R"))
        .unwrap_or_default();
    let dict = format!(
        "/Type /XObject /Subtype /Image /Width {pixel_width} /Height {pixel_height} /ColorSpace /DeviceRGB /BitsPerComponent 8{smask_ref}"
    );
    pdf.add_stream(dict, rgb_data)
}

fn emit_styled_line(
    content: &mut String,
    page_height_pt: f32,
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    color: Color,
    width_pt: f32,
    border_style: BorderStyle,
    edge: PaintLineEdge,
    pdf: &mut PdfBuilder,
    resources: &mut PageResources,
) {
    if width_pt <= 0.0 || matches!(border_style, BorderStyle::None | BorderStyle::Hidden) {
        return;
    }

    match border_style {
        BorderStyle::Double => emit_parallel_line_pair(
            pdf,
            resources,
            content,
            page_height_pt,
            x1,
            y1,
            x2,
            y2,
            color,
            width_pt,
            1.0,
            1.0,
            edge,
        ),
        BorderStyle::Groove => {
            let (outside_factor, inside_factor) = groove_bevel_factors(edge);
            emit_parallel_line_pair(
                pdf,
                resources,
                content,
                page_height_pt,
                x1,
                y1,
                x2,
                y2,
                color,
                width_pt,
                outside_factor,
                inside_factor,
                edge,
            );
        }
        BorderStyle::Ridge => {
            let (outside_factor, inside_factor) = groove_bevel_factors(edge);
            emit_parallel_line_pair(
                pdf,
                resources,
                content,
                page_height_pt,
                x1,
                y1,
                x2,
                y2,
                color,
                width_pt,
                inside_factor,
                outside_factor,
                edge,
            );
        }
        BorderStyle::Inset => emit_single_line(
            pdf,
            resources,
            content,
            page_height_pt,
            x1,
            y1,
            x2,
            y2,
            shade_color(color, inset_outset_factor(edge, true)),
            width_pt,
            None,
            false,
        ),
        BorderStyle::Outset => emit_single_line(
            pdf,
            resources,
            content,
            page_height_pt,
            x1,
            y1,
            x2,
            y2,
            shade_color(color, inset_outset_factor(edge, false)),
            width_pt,
            None,
            false,
        ),
        BorderStyle::Dashed => {
            let (dash, gap, phase) = dash_pattern_for_edge(width_pt, x1, y1, x2, y2, edge, false);
            emit_single_line(
                pdf,
                resources,
                content,
                page_height_pt,
                x1,
                y1,
                x2,
                y2,
                color,
                width_pt,
                Some((dash, gap, phase)),
                false,
            );
        }
        BorderStyle::Dotted => {
            let (dot, gap, phase) = dash_pattern_for_edge(width_pt, x1, y1, x2, y2, edge, true);
            emit_single_line(
                pdf,
                resources,
                content,
                page_height_pt,
                x1,
                y1,
                x2,
                y2,
                color,
                width_pt,
                Some((dot, gap, phase)),
                true,
            );
        }
        BorderStyle::Solid => emit_single_line(
            pdf,
            resources,
            content,
            page_height_pt,
            x1,
            y1,
            x2,
            y2,
            color,
            width_pt,
            None,
            false,
        ),
        BorderStyle::Hidden | BorderStyle::None => {}
    }
}

fn emit_parallel_line_pair(
    pdf: &mut PdfBuilder,
    resources: &mut PageResources,
    content: &mut String,
    page_height_pt: f32,
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    color: Color,
    width_pt: f32,
    outside_factor: f32,
    inside_factor: f32,
    edge: PaintLineEdge,
) {
    let dx = x2 - x1;
    let dy = y2 - y1;
    let length = (dx * dx + dy * dy).sqrt();
    if length <= 0.001 {
        return;
    }

    let normal_x = -dy / length;
    let normal_y = dx / length;
    let lane_width = (width_pt * 0.45).max(0.2);
    let offset = (width_pt * 0.28).max(0.2);

    let (near_factor, far_factor) = if near_side_is_inside(edge) {
        (inside_factor, outside_factor)
    } else {
        (outside_factor, inside_factor)
    };
    let near_color = shade_color(color, near_factor);
    let far_color = shade_color(color, far_factor);

    emit_single_line(
        pdf,
        resources,
        content,
        page_height_pt,
        x1 + normal_x * offset,
        y1 + normal_y * offset,
        x2 + normal_x * offset,
        y2 + normal_y * offset,
        near_color,
        lane_width,
        None,
        false,
    );
    emit_single_line(
        pdf,
        resources,
        content,
        page_height_pt,
        x1 - normal_x * offset,
        y1 - normal_y * offset,
        x2 - normal_x * offset,
        y2 - normal_y * offset,
        far_color,
        lane_width,
        None,
        false,
    );
}

fn emit_single_line(
    pdf: &mut PdfBuilder,
    resources: &mut PageResources,
    content: &mut String,
    page_height_pt: f32,
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    color: Color,
    width_pt: f32,
    dash_pattern: Option<(f32, f32, f32)>,
    rounded_cap: bool,
) {
    let pdf_y1 = page_height_pt - y1;
    let pdf_y2 = page_height_pt - y2;
    content.push_str("q\n");
    if color.a < 0.999 {
        let gstate_name = add_ext_gstate(pdf, resources, None, Some(color.a));
        content.push_str(&format!("/{gstate_name} gs\n"));
    }
    push_stroke_color(content, color);
    content.push_str(&format!("{} w\n", fmt(width_pt.max(0.1))));
    content.push_str(if rounded_cap { "1 J\n" } else { "0 J\n" });
    if let Some((dash, gap, phase)) = dash_pattern {
        content.push_str(&format!(
            "[{} {}] {} d\n",
            fmt(dash.max(0.05)),
            fmt(gap.max(0.05)),
            fmt(phase.max(0.0))
        ));
    } else {
        content.push_str("[] 0 d\n");
    }
    content.push_str(&format!(
        "{} {} m\n{} {} l\nS\nQ\n",
        fmt(x1),
        fmt(pdf_y1),
        fmt(x2),
        fmt(pdf_y2)
    ));
}

fn shade_color(color: Color, factor: f32) -> Color {
    Color {
        r: clamp_color(color.r * factor),
        g: clamp_color(color.g * factor),
        b: clamp_color(color.b * factor),
        a: color.a,
    }
}

fn near_side_is_inside(edge: PaintLineEdge) -> bool {
    matches!(edge, PaintLineEdge::Top | PaintLineEdge::Right)
}

fn dash_pattern_for_edge(
    width_pt: f32,
    x1: f32,
    y1: f32,
    x2: f32,
    y2: f32,
    edge: PaintLineEdge,
    dotted: bool,
) -> (f32, f32, f32) {
    let (dash, gap) = if dotted {
        ((width_pt * 0.22).max(0.15), (width_pt * 1.35).max(0.8))
    } else {
        ((width_pt * 2.9).max(1.2), (width_pt * 1.95).max(0.8))
    };
    let cycle = (dash + gap).max(0.05);

    let dx = x2 - x1;
    let dy = y2 - y1;
    let length = (dx * dx + dy * dy).sqrt().max(0.001);
    let centered_phase = ((cycle - (length % cycle)) * 0.5) % cycle;

    // Bottom/left sides visually match browser more closely when phase mirrors
    // the top/right phase, approximating clockwise side progression.
    let phase = if matches!(edge, PaintLineEdge::Bottom | PaintLineEdge::Left) {
        (cycle - centered_phase) % cycle
    } else {
        centered_phase
    };

    (dash, gap, phase)
}

fn groove_bevel_factors(edge: PaintLineEdge) -> (f32, f32) {
    match edge {
        PaintLineEdge::Top | PaintLineEdge::Left => (0.70, 1.16),
        PaintLineEdge::Bottom | PaintLineEdge::Right => (1.16, 0.70),
    }
}

fn inset_outset_factor(edge: PaintLineEdge, inset: bool) -> f32 {
    let top_left = matches!(edge, PaintLineEdge::Top | PaintLineEdge::Left);
    let bottom_right = matches!(edge, PaintLineEdge::Bottom | PaintLineEdge::Right);
    match (inset, top_left, bottom_right) {
        (true, true, _) => 0.74,
        (true, _, true) => 1.18,
        (false, true, _) => 1.18,
        (false, _, true) => 0.74,
        _ => 1.0,
    }
}

fn clamp_color(value: f32) -> f32 {
    value.clamp(0.0, 1.0)
}

fn resource_dict(
    usages: &HashMap<FontKey, FontUsage>,
    font_object_ids: &HashMap<FontKey, usize>,
    resources: &PageResources,
) -> String {
    let font_dict = resource_font_dict(usages, font_object_ids);
    let mut parts = vec![format!("/Font {font_dict}")];

    if !resources.xobjects.is_empty() {
        let xobject_dict = resources
            .xobjects
            .iter()
            .map(|resource| format!("/{} {} 0 R", resource.name, resource.object_id))
            .collect::<Vec<_>>()
            .join(" ");
        parts.push(format!("/XObject << {xobject_dict} >>"));
    }

    if !resources.patterns.is_empty() {
        let pattern_dict = resources
            .patterns
            .iter()
            .map(|resource| format!("/{} {} 0 R", resource.name, resource.object_id))
            .collect::<Vec<_>>()
            .join(" ");
        parts.push(format!("/Pattern << {pattern_dict} >>"));
    }

    if !resources.ext_gstates.is_empty() {
        let extgstate_dict = resources
            .ext_gstates
            .iter()
            .map(|resource| format!("/{} {} 0 R", resource.name, resource.object_id))
            .collect::<Vec<_>>()
            .join(" ");
        parts.push(format!("/ExtGState << {extgstate_dict} >>"));
    }

    format!("<< {} >>", parts.join(" "))
}

fn resource_font_dict(
    usages: &HashMap<FontKey, FontUsage>,
    font_object_ids: &HashMap<FontKey, usize>,
) -> String {
    let mut pairs = usages.iter().collect::<Vec<_>>();
    pairs.sort_by_key(|(key, _)| key.0);
    let inner = pairs
        .into_iter()
        .filter_map(|(font_key, usage)| {
            font_object_ids
                .get(font_key)
                .map(|object_id| format!("/{} {object_id} 0 R", usage.resource_name))
        })
        .collect::<Vec<_>>()
        .join(" ");
    format!("<< {inner} >>")
}

fn build_cid_to_gid_map(usage: &FontUsage) -> Vec<u8> {
    let mut bytes = vec![0u8, 0u8];
    for key in &usage.cids {
        bytes.extend_from_slice(&key.glyph_id.to_be_bytes());
    }
    bytes
}

fn build_width_array(font: &LoadedFont, usage: &FontUsage) -> String {
    if usage.cids.is_empty() {
        return "[]".to_string();
    }
    let widths = usage
        .cids
        .iter()
        .map(|key| font.glyph_advance_1000(key.glyph_id).max(0).to_string())
        .collect::<Vec<_>>()
        .join(" ");
    format!("[1 [{widths}]]")
}

fn build_to_unicode_cmap(usage: &FontUsage) -> String {
    let mut cmap = String::new();
    cmap.push_str("/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n");
    cmap.push_str("/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n");
    cmap.push_str("/CMapName /LynPDF-Thai-UCS def\n/CMapType 2 def\n");
    cmap.push_str("1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n");

    for chunk in usage.cids.chunks(100) {
        cmap.push_str(&format!("{} beginbfchar\n", chunk.len()));
        for key in chunk {
            let cid = usage.cid_lookup.get(key).copied().unwrap_or(0);
            cmap.push_str(&format!(
                "<{}> <{}>\n",
                hex_u16(cid),
                utf16be_hex(&key.unicode)
            ));
        }
        cmap.push_str("endbfchar\n");
    }

    cmap.push_str("endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n");
    cmap
}

struct PdfBuilder {
    objects: Vec<Option<Vec<u8>>>,
}

impl PdfBuilder {
    fn new() -> Self {
        Self {
            objects: Vec::new(),
        }
    }

    fn reserve(&mut self) -> usize {
        self.objects.push(None);
        self.objects.len()
    }

    fn set(&mut self, id: usize, bytes: Vec<u8>) {
        self.objects[id - 1] = Some(bytes);
    }

    fn add(&mut self, bytes: Vec<u8>) -> usize {
        self.objects.push(Some(bytes));
        self.objects.len()
    }

    fn add_stream(&mut self, dict: String, data: &[u8]) -> usize {
        if let Some(compressed) = flate_compress_stream(data) {
            return self.add_stream_with_filter(dict, &compressed, "/FlateDecode");
        }

        self.add_stream_uncompressed(dict, data)
    }

    fn add_stream_with_filter(&mut self, dict: String, data: &[u8], filter: &str) -> usize {
        let mut bytes =
            format!("<< {dict} /Filter {filter} /Length {} >>\nstream\n", data.len())
                .into_bytes();
        bytes.extend_from_slice(data);
        bytes.extend_from_slice(b"\nendstream");
        self.add(bytes)
    }

    fn add_stream_uncompressed(&mut self, dict: String, data: &[u8]) -> usize {
        let mut bytes = format!("<< {dict} /Length {} >>\nstream\n", data.len()).into_bytes();
        bytes.extend_from_slice(data);
        bytes.extend_from_slice(b"\nendstream");
        self.add(bytes)
    }

    fn finish(self, root_id: usize, info_id: Option<usize>) -> Vec<u8> {
        let mut output = b"%PDF-1.7\n%\xE2\xE3\xCF\xD3\n".to_vec();
        let mut offsets = vec![0usize];
        for (idx, object) in self.objects.into_iter().enumerate() {
            offsets.push(output.len());
            output.extend_from_slice(format!("{} 0 obj\n", idx + 1).as_bytes());
            output.extend_from_slice(&object.unwrap_or_else(|| b"<<>>".to_vec()));
            output.extend_from_slice(b"\nendobj\n");
        }

        let xref_offset = output.len();
        output.extend_from_slice(format!("xref\n0 {}\n", offsets.len()).as_bytes());
        output.extend_from_slice(b"0000000000 65535 f \n");
        for offset in offsets.iter().skip(1) {
            output.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
        }
        let info_entry = info_id
            .map(|id| format!(" /Info {id} 0 R"))
            .unwrap_or_default();
        output.extend_from_slice(
            format!(
                "trailer\n<< /Size {} /Root {root_id} 0 R{info_entry} >>\nstartxref\n{xref_offset}\n%%EOF\n",
                offsets.len()
            )
            .as_bytes(),
        );
        output
    }
}

fn push_fill_color(content: &mut String, color: Color) {
    content.push_str(&format!(
        "{} {} {} rg\n",
        fmt(color.r),
        fmt(color.g),
        fmt(color.b)
    ));
}

fn push_stroke_color(content: &mut String, color: Color) {
    content.push_str(&format!(
        "{} {} {} RG\n",
        fmt(color.r),
        fmt(color.g),
        fmt(color.b)
    ));
}

fn fmt(value: f32) -> String {
    let rounded = (value * 1000.0).round() / 1000.0;
    if rounded.fract().abs() < 0.0001 {
        format!("{rounded:.0}")
    } else {
        format!("{rounded:.3}")
    }
}

fn hex_u16(value: u16) -> String {
    format!("{value:04X}")
}

fn utf16be_hex(text: &str) -> String {
    text.encode_utf16()
        .map(|unit| format!("{unit:04X}"))
        .collect::<String>()
}

fn pdf_text_string(text: &str) -> String {
    let mut encoded = String::from("FEFF");
    encoded.push_str(&utf16be_hex(text));
    format!("<{encoded}>")
}

fn pdf_literal_string(text: &str) -> String {
    let mut out = String::from("(");
    for ch in text.chars() {
        match ch {
            '\\' => out.push_str("\\\\"),
            '(' => out.push_str("\\("),
            ')' => out.push_str("\\)"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            _ => out.push(ch),
        }
    }
    out.push(')');
    out
}

fn pdf_name(name: &str) -> String {
    name.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' {
                ch
            } else {
                '-'
            }
        })
        .collect()
}

fn flate_compress_stream(data: &[u8]) -> Option<Vec<u8>> {
    if data.len() < 96 {
        return None;
    }

    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::fast());
    encoder.write_all(data).ok()?;
    let compressed = encoder.finish().ok()?;

    (compressed.len() + 16 < data.len()).then_some(compressed)
}

fn encode_rgb_to_jpeg(
    pixel_width: u32,
    pixel_height: u32,
    rgb_data: &[u8],
    quality: u8,
) -> Option<Vec<u8>> {
    let expected_len = (pixel_width as usize)
        .checked_mul(pixel_height as usize)?
        .checked_mul(3)?;
    if rgb_data.len() != expected_len {
        return None;
    }

    let mut encoded = Vec::new();
    let mut encoder = JpegEncoder::new_with_quality(&mut encoded, quality);
    encoder
        .encode(rgb_data, pixel_width, pixel_height, ExtendedColorType::Rgb8)
        .ok()?;
    Some(encoded)
}
