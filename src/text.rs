use crate::error::{LynPdfError, Result};
use crate::fonts::{FontKey, FontRegistry};
use crate::style::{
    ComputedStyle, Hyphens, LineBreak, OverflowWrap, TextAlign, TextJustify, WhiteSpace, WordBreak,
};
use crate::types::RenderOptions;
use rustybuzz::{script, Direction, Face, Feature, Language, UnicodeBuffer, Variation};
use std::collections::BTreeSet;
use std::str::FromStr;
use unicode_normalization::UnicodeNormalization;
use unicode_segmentation::UnicodeSegmentation;

const THAI_WORD_DICTIONARY: &[&str] = &[
    "การจัดรูปแบบข้อความ",
    "การจัดรูปแบบ",
    "การตัดคำภาษาไทย",
    "การตัดคำ",
    "การกระจายช่องไฟ",
    "การเว้นวรรค",
    "การแยกแยะ",
    "การขึ้นบรรทัดใหม่",
    "การจัดหน้า",
    "การทำงาน",
    "การทดสอบ",
    "ตะวันออกเฉียงใต้",
    "สมเด็จพระเจ้าอยู่หัว",
    "พระมหากรุณาธิคุณ",
    "สัมมาสัมพุทธัสสะ",
    "ธรฺมจกฺรปฺรวรฺตนสูตฺร",
    "ปฺรตีตฺยสมุทฺปาท",
    "สมฺปชัญญะ",
    "มุกฎาภิเษก",
    "จุฬาราชมนตรี",
    "จุฬาลงกรณ์",
    "กรุงเทพมหานคร",
    "ตะวันออกเฉียงใต้",
    "สมเด็จพระเจ้าอยู่หัว",
    "พระมหากรุณาธิคุณ",
    "กรุงเทพมหานคร",
    "ชาติตะวันตก",
    "ล่าอาณานิคม",
    "เอกลักษณ์",
    "โดดเด่น",
    "ประเทศไทย",
    "ทั้งประเทศ",
    "ทางทะเล",
    "ทางบก",
    "เพื่อนบ้าน",
    "พรมแดน",
    "วัฒนธรรม",
    "เก่าแก่",
    "ยาวนาน",
    "หลายพันปี",
    "อาณานิคม",
    "ภูมิภาค",
    "ติดต่อ",
    "ตั้งอยู่",
    "ประชากร",
    "เมืองหลวง",
    "ใหญ่ที่สุด",
    "หนาแน่น",
    "เขตเมือง",
    "พื้นที่",
    "คุณภาพสูง",
    "เชื่อถือได้",
    "ทำงานร่วมกัน",
    "ยากลำบาก",
    "ทั้งประเทศ",
    "ประเทศเพื่อนบ้าน",
    "ติดต่อกับ",
    "หลายประเทศ",
    "ล่าอาณานิคม",
    "ไม่เคย",
    "ตกเป็น",
    "ถือเป็น",
    "โดยเฉพาะ",
    "ชนบท",
    "ห่างไกล",
    "ประชากร",
    "เมืองหลวง",
    "ใหญ่ที่สุด",
    "ล้านคน",
    "เกือบ",
    "วรรณยุกต์ซ้อน",
    "สระลอย",
    "สระบน",
    "สระล่าง",
    "พยัญชนะต้น",
    "ตัวเลขไทย",
    "คำทดสอบ",
    "พื้นฐาน",
    "น้ำพริก",
    "ผู้ใหญ่",
    "บ้านเกิด",
    "ลูกศิษย์",
    "ครู่หนึ่ง",
    "ดุ๊กดิ๊ก",
    "มูลค่า",
    "ครุฑ",
    "ปวงชน",
    "ชาวไทย",
    "เอเชีย",
    "พรมแดน",
    "เพื่อนบ้าน",
    "วัฒนธรรม",
    "เก่าแก่",
    "ยาวนาน",
    "หลายพันปี",
    "อาณานิคม",
    "ภูมิภาค",
    "เอกลักษณ์",
    "โดดเด่น",
    "ธรรมชาติ",
    "อ่านง่าย",
    "มาตรฐาน",
    "ข้อความ",
    "เปรียบเทียบ",
    "โปรแกรม",
    "จัดการ",
    "ถูกต้อง",
    "ประสิทธิภาพ",
    "เอกสาร",
    "คลัสเตอร์",
    "เทคนิค",
    "กระจาย",
    "ช่องไฟ",
    "บรรทัด",
    "วรรณยุกต์",
    "พยัญชนะ",
    "ตัวอักษร",
    "ภาษาไทย",
    "ภาษาอังกฤษ",
    "ข้อความผสม",
    "นะโม",
    "ตัสสะ",
    "ภะคะวะโต",
    "อะระหะโต",
    "สพฺเพ",
    "ธมฺมา",
    "อนตฺตา",
    "สงฺขารา",
    "อนิจฺจา",
    "ทุกฺขา",
    "กฺริยา",
    "ปฺรชฺญา",
    "สมฺภว",
    "สมฺภวามิ",
    "ยุเค",
    "ธรฺม",
    "กรฺม",
    "จิตฺต",
    "สุตฺต",
    "นิพฺพาน",
    "นมัสฺการ",
    "สฺวากฺขาโต",
    "สาวะกะสังโฆ",
    "อุชุปะฏิปันโน",
    "ญายะปะฏิปันโน",
    "สามีจิปะฏิปันโน",
    "วิชชาจะระณะสัมปันโน",
    "ปุริสะทัมมะสาระถิ",
    "เทวะมะนุสสานัง",
    "นิคหิต",
    "พินทุ",
    "ทัณฑฆาต",
    "ยามักการ",
    "อังคั่น",
    "พยัญชนะเฉพาะ",
    "สันสกฤต",
    "บาลี",
    "อักขระพิเศษ",
    "ราชฎุมภ์",
    "กฎหมาย",
    "กฏิกา",
    "ฐานะ",
    "ฐาปนา",
    "อิฐ",
    "ญาณ",
    "ญาติ",
    "ปัญญา",
    "วิญญาณ",
    "สัญญา",
    "มัชฌิมา",
    "ฬ่อ",
    "กิฬา",
    "มฤคฬา",
    "ปรากฏ",
    "กฎ",
    "ฐิติ",
    "ฐานันดร",
    "อัฏฐ",
    "ทิฏฐิ",
    "กฏุก",
    "วิวัฒน์",
    "ปรัชญา",
    "จังหวัด",
    "ขนาด",
    "เปรียบเทียบฟอนต์",
    "ครอบคลุม",
    "ประโยค",
    "บทสวด",
    "ยาว",
    "ต่อเนื่อง",
    "ประมาณ",
    "โดยเฉพาะ",
    "ยากลำบาก",
    "ห่างไกล",
    "ปวงชน",
    "ชาวไทย",
    "ภาษาไทย",
    "ตัวอักษร",
    "วรรณยุกต์",
    "พยัญชนะ",
    "การตัดคำ",
    "การจัดรูปแบบ",
    "กระจาย",
    "บรรทัด",
    "เทคนิค",
    "คลัสเตอร์",
    "เอกสาร",
    "คุณภาพสูง",
    "ประสิทธิภาพ",
    "เชื่อถือได้",
    "ทำงานร่วมกัน",
    "ปัจจุบัน",
    "ข้อความ",
    "เปรียบเทียบ",
    "ธรรมชาติ",
    "ถูกต้อง",
    "อ่านง่าย",
    "มาตรฐาน",
    "สระลอย",
    "พื้นฐาน",
    "ประเทศ",
    "ทรงมี",
    "เป็นล้นพ้น",
    "ใน",
    "เป็น",
    "ที่",
    "มี",
    "และ",
    "ของ",
    "ไม่",
    "กับ",
    "ทั้ง",
    "โดย",
    "จาก",
    "หรือ",
    "ซึ่ง",
    "จะ",
    "ต้อง",
    "ใช้",
    "ให้",
    "ได้",
    "ต่อ",
    "ตาม",
    "ผู้",
    "ผู้ที่",
    "ภาษา",
    "ไทย",
    "ระบบ",
    "พัฒนา",
    "รองรับ",
    "จัดการ",
    "การ",
    "คำ",
    "หนึ่ง",
    "สอง",
    "สาม",
    "สี่",
    "ห้า",
    "หก",
    "เจ็ด",
    "แปด",
    "เก้า",
    "สิบ",
];

#[derive(Debug, Clone)]
pub struct TextLine {
    pub glyphs: Vec<PositionedGlyph>,
    pub width_pt: f32,
}

#[derive(Debug, Clone)]
pub struct PositionedGlyph {
    pub font_key: FontKey,
    pub glyph_id: u16,
    pub cluster_text: String,
    pub cluster_start: usize,
    pub cluster_end: usize,
    pub x_pt: f32,
    pub y_offset_pt: f32,
}

#[derive(Debug, Clone)]
struct ShapedGlyph {
    font_key: FontKey,
    glyph_id: u16,
    cluster_start: usize,
    x_pt: f32,
    x_offset_pt: f32,
    y_offset_pt: f32,
    advance_pt: f32,
}

#[derive(Debug, Clone)]
struct LogicalCluster {
    start: usize,
    text: String,
}

#[derive(Debug, Clone)]
struct ShapedCluster {
    start: usize,
    end: usize,
    text: String,
    glyph_start: usize,
    glyph_end: usize,
    x_start_pt: f32,
    x_end_pt: f32,
    break_after: bool,
}

pub fn shape_lines(
    text: &str,
    style: &ComputedStyle,
    max_width_pt: f32,
    fonts: &FontRegistry,
    options: &RenderOptions,
) -> Result<Vec<TextLine>> {
    let prepared = normalize_text_for_white_space(text, style.white_space);
    let normalized = prepared.nfc().collect::<String>();
    let logical_clusters = split_logical_clusters(&normalized);
    if logical_clusters.is_empty() {
        return Ok(Vec::new());
    }

    let mut glyphs = Vec::new();
    let mut pen_x = 0.0f32;
    let mut run_start = 0usize;
    let mut run_font = None;

    for (idx, cluster) in logical_clusters.iter().enumerate() {
        if cluster.text == "\n" {
            if let Some(current_font) = run_font {
                shape_cluster_run(
                    &logical_clusters,
                    run_start,
                    idx,
                    current_font,
                    style,
                    fonts,
                    options,
                    &mut pen_x,
                    &mut glyphs,
                )?;
                run_font = None;
            }
            run_start = idx + 1;
            pen_x = 0.0;
            continue;
        }

        let cluster_font = fonts.resolve_for_cluster(
            &style.font_families,
            style.font_weight,
            style.font_style,
            &cluster.text,
        );

        match run_font {
            None => {
                run_font = Some(cluster_font);
                run_start = idx;
            }
            Some(current_font) if current_font == cluster_font => {}
            Some(current_font) => {
                shape_cluster_run(
                    &logical_clusters,
                    run_start,
                    idx,
                    current_font,
                    style,
                    fonts,
                    options,
                    &mut pen_x,
                    &mut glyphs,
                )?;
                run_start = idx;
                run_font = Some(cluster_font);
            }
        }
    }

    if let Some(current_font) = run_font {
        shape_cluster_run(
            &logical_clusters,
            run_start,
            logical_clusters.len(),
            current_font,
            style,
            fonts,
            options,
            &mut pen_x,
            &mut glyphs,
        )?;
    }

    if glyphs.is_empty() {
        return Ok(Vec::new());
    }

    let clusters = build_clusters(
        &normalized,
        &glyphs,
        style.word_break,
        style.overflow_wrap,
        style.line_break,
        style.hyphens,
    );
    compose_lines(
        &normalized,
        &glyphs,
        &clusters,
        max_width_pt,
        style.text_align,
        style.text_justify,
        style.white_space,
        style.word_break,
        style.overflow_wrap,
        style.line_break,
    )
}

fn normalize_text_for_white_space(text: &str, white_space: WhiteSpace) -> String {
    match white_space {
        WhiteSpace::Pre | WhiteSpace::PreWrap => text.to_string(),
        WhiteSpace::Normal | WhiteSpace::NoWrap => collapse_text_whitespace(text, false),
        WhiteSpace::PreLine => collapse_text_whitespace(text, true),
    }
}

fn collapse_text_whitespace(text: &str, keep_newlines: bool) -> String {
    let mut output = String::new();
    let mut previous_space = false;

    for ch in text.chars() {
        if ch == '\n' {
            if keep_newlines {
                if !output.ends_with('\n') {
                    output.push('\n');
                }
                previous_space = false;
            } else if !previous_space {
                output.push(' ');
                previous_space = true;
            }
            continue;
        }

        if ch.is_whitespace() {
            if !previous_space {
                output.push(' ');
                previous_space = true;
            }
        } else {
            output.push(ch);
            previous_space = false;
        }
    }

    output.trim().to_string()
}

fn split_logical_clusters(text: &str) -> Vec<LogicalCluster> {
    let mut clusters = Vec::new();
    let mut idx = 0usize;

    while idx < text.len() {
        let Some(ch) = text[idx..].chars().next() else {
            break;
        };

        if ch == '\n' {
            clusters.push(LogicalCluster {
                start: idx,
                text: ch.to_string(),
            });
            idx += ch.len_utf8();
            continue;
        }

        if is_thai_cluster_starter(ch) {
            let start = idx;
            idx += ch.len_utf8();

            if is_thai_preposed_vowel(ch) {
                if let Some(next) = text[idx..].chars().next() {
                    if next != '\n' && is_thai_script_char(next) {
                        idx += next.len_utf8();
                    }
                }
            }

            while idx < text.len() {
                let Some(next) = text[idx..].chars().next() else {
                    break;
                };
                if next == '\n' || is_thai_preposed_vowel(next) || is_thai_base_char(next) {
                    break;
                }
                if !is_thai_cluster_extension(next) {
                    break;
                }
                idx += next.len_utf8();
            }

            clusters.push(LogicalCluster {
                start,
                text: text[start..idx].to_string(),
            });
            continue;
        }

        let grapheme = text[idx..]
            .graphemes(true)
            .next()
            .unwrap_or_else(|| &text[idx..]);
        clusters.push(LogicalCluster {
            start: idx,
            text: grapheme.to_string(),
        });
        idx += grapheme.len();
    }

    clusters
}

fn shape_cluster_run(
    clusters: &[LogicalCluster],
    start: usize,
    end: usize,
    font_key: FontKey,
    style: &ComputedStyle,
    fonts: &FontRegistry,
    options: &RenderOptions,
    pen_x: &mut f32,
    output: &mut Vec<ShapedGlyph>,
) -> Result<()> {
    if start >= end || start >= clusters.len() {
        return Ok(());
    }

    let font = fonts.get(font_key);
    let mut face = Face::from_slice(&font.data, 0)
        .ok_or_else(|| LynPdfError::FontParse(format!("{}", font.path.display())))?;
    apply_font_variations(&mut face, &options.font_variations);

    let mut run_text = String::new();
    let mut run_cluster_map = Vec::new();
    for cluster in &clusters[start..end] {
        if cluster.text == "\n" {
            continue;
        }
        run_cluster_map.push((run_text.len(), cluster.start));
        run_text.push_str(&shape_text_for_cluster(&cluster.text));
    }

    if run_text.is_empty() {
        return Ok(());
    }

    let mut buffer = UnicodeBuffer::new();
    buffer.push_str(&run_text);
    if run_text.chars().any(is_thai_script_char) {
        buffer.set_direction(Direction::LeftToRight);
        buffer.set_script(script::THAI);
        if let Ok(language) = Language::from_str("th") {
            buffer.set_language(language);
        }
    }
    buffer.guess_segment_properties();
    let features = build_shaping_features(options);
    let glyph_buffer = rustybuzz::shape(&face, &features, buffer);
    let infos = glyph_buffer.glyph_infos();
    let positions = glyph_buffer.glyph_positions();

    let scale = style.font_size_pt / font.units_per_em;
    for (info, position) in infos.iter().zip(positions.iter()) {
        let x_offset_pt = position.x_offset as f32 * scale;
        let y_offset_pt = position.y_offset as f32 * scale;
        let advance_pt = position.x_advance as f32 * scale;
        output.push(ShapedGlyph {
            font_key,
            glyph_id: info.glyph_id as u16,
            cluster_start: resolve_cluster_start(info.cluster as usize, &run_cluster_map),
            x_pt: *pen_x + x_offset_pt,
            x_offset_pt,
            y_offset_pt,
            advance_pt,
        });
        *pen_x += advance_pt;
    }

    Ok(())
}

fn resolve_cluster_start(run_cluster_start: usize, run_cluster_map: &[(usize, usize)]) -> usize {
    if run_cluster_map.is_empty() {
        return 0;
    }
    let index = run_cluster_map.partition_point(|(run_start, _)| *run_start <= run_cluster_start);
    let resolved = index.saturating_sub(1);
    run_cluster_map[resolved].1
}

fn shaping_feature_specs(options: &RenderOptions) -> [&'static str; 3] {
    [
        if options.enable_kerning {
            "kern"
        } else {
            "-kern"
        },
        if options.enable_ligatures {
            "liga"
        } else {
            "-liga"
        },
        if options.enable_ligatures {
            "clig"
        } else {
            "-clig"
        },
    ]
}

fn build_shaping_features(options: &RenderOptions) -> Vec<Feature> {
    shaping_feature_specs(options)
        .into_iter()
        .filter_map(|spec| Feature::from_str(spec).ok())
        .collect::<Vec<_>>()
}

fn parse_font_variations(raw_variations: &[String]) -> Vec<Variation> {
    raw_variations
        .iter()
        .filter_map(|value| Variation::from_str(value).ok())
        .collect::<Vec<_>>()
}

fn apply_font_variations(face: &mut Face<'_>, raw_variations: &[String]) {
    let variations = parse_font_variations(raw_variations);
    if !variations.is_empty() {
        face.set_variations(&variations);
    }
}

fn thai_dictionary_break_positions(text: &str) -> BTreeSet<usize> {
    let mut breaks = BTreeSet::new();
    let mut idx = 0usize;

    while idx < text.len() {
        let Some(ch) = text[idx..].chars().next() else {
            break;
        };
        if !is_thai_script_char(ch) {
            idx += ch.len_utf8();
            continue;
        }

        let run_start = idx;
        idx += ch.len_utf8();
        while idx < text.len() {
            let Some(next) = text[idx..].chars().next() else {
                break;
            };
            if !is_thai_script_char(next) {
                break;
            }
            idx += next.len_utf8();
        }

        segment_thai_run_with_dictionary(&text[run_start..idx], run_start, &mut breaks);
    }

    breaks
}

fn segment_thai_run_with_dictionary(run: &str, run_start: usize, breaks: &mut BTreeSet<usize>) {
    let mut offset = 0usize;

    while offset < run.len() {
        if let Some(end) = longest_dictionary_match_at(run, offset) {
            offset = end;
            if offset < run.len() {
                breaks.insert(run_start + offset);
            }
            continue;
        }

        let advanced = advance_grapheme_offset(run, offset);
        if advanced <= offset {
            break;
        }
        offset = advanced;
    }
}

fn longest_dictionary_match_at(run: &str, offset: usize) -> Option<usize> {
    let suffix = &run[offset..];
    THAI_WORD_DICTIONARY
        .iter()
        .filter_map(|word| suffix.starts_with(word).then_some(offset + word.len()))
        .max()
}

fn advance_grapheme_offset(text: &str, offset: usize) -> usize {
    text[offset..]
        .grapheme_indices(true)
        .nth(1)
        .map(|(delta, _)| offset + delta)
        .unwrap_or(text.len())
}

fn is_thai_script_char(ch: char) -> bool {
    ('\u{0E00}'..='\u{0E7F}').contains(&ch)
}

fn is_thai_cluster_starter(ch: char) -> bool {
    is_thai_preposed_vowel(ch) || is_thai_base_char(ch) || is_thai_cluster_extension(ch)
}

fn is_thai_base_char(ch: char) -> bool {
    ('\u{0E01}'..='\u{0E2E}').contains(&ch)
}

fn is_thai_preposed_vowel(ch: char) -> bool {
    ('\u{0E40}'..='\u{0E44}').contains(&ch)
}

fn is_thai_cluster_extension(ch: char) -> bool {
    matches!(
        ch,
        '\u{0E30}'
            | '\u{0E31}'
            | '\u{0E32}'
            | '\u{0E33}'
            | '\u{0E45}'
            | '\u{0E34}'..='\u{0E3A}'
            | '\u{0E47}'..='\u{0E4E}'
    )
}

fn is_thai_tone_mark(ch: char) -> bool {
    ('\u{0E48}'..='\u{0E4B}').contains(&ch)
}

fn shape_text_for_cluster(text: &str) -> String {
    if text.contains('\u{0E33}') {
        expand_thai_sara_am_for_shaping(text)
    } else {
        text.to_string()
    }
}

fn expand_thai_sara_am_for_shaping(text: &str) -> String {
    let mut chars = Vec::new();
    let mut sara_am_count = 0usize;
    for ch in text.chars() {
        if ch == '\u{0E33}' {
            sara_am_count += 1;
        } else {
            chars.push(ch);
        }
    }

    if sara_am_count == 0 {
        return text.to_string();
    }

    let insert_at = chars
        .iter()
        .position(|ch| is_thai_tone_mark(*ch))
        .unwrap_or(chars.len());
    let mut output = String::new();
    for (idx, ch) in chars.iter().enumerate() {
        if idx == insert_at {
            for _ in 0..sara_am_count {
                output.push('\u{0E4D}');
            }
        }
        output.push(*ch);
    }
    if insert_at == chars.len() {
        for _ in 0..sara_am_count {
            output.push('\u{0E4D}');
        }
    }
    for _ in 0..sara_am_count {
        output.push('\u{0E32}');
    }
    output
}

fn build_clusters(
    text: &str,
    glyphs: &[ShapedGlyph],
    word_break: WordBreak,
    overflow_wrap: OverflowWrap,
    line_break: LineBreak,
    hyphens: Hyphens,
) -> Vec<ShapedCluster> {
    let thai_dictionary_breaks = if word_break == WordBreak::KeepAll {
        BTreeSet::new()
    } else {
        thai_dictionary_break_positions(text)
    };
    let mut starts = BTreeSet::new();
    starts.insert(0usize);
    starts.insert(text.len());
    for glyph in glyphs {
        starts.insert(glyph.cluster_start.min(text.len()));
    }
    for (idx, _) in text.char_indices() {
        if text[idx..].starts_with('\n') {
            starts.insert(idx);
            starts.insert(idx + 1);
        }
    }

    let starts = starts.into_iter().collect::<Vec<_>>();
    let mut clusters = Vec::new();
    for pair in starts.windows(2) {
        let start = pair[0];
        let end = pair[1];
        if start == end {
            continue;
        }
        let glyph_indices = glyphs
            .iter()
            .enumerate()
            .filter_map(|(idx, glyph)| {
                (glyph.cluster_start >= start && glyph.cluster_start < end).then_some(idx)
            })
            .collect::<Vec<_>>();

        let slice = text[start..end].to_string();
        if glyph_indices.is_empty() && slice == "\n" {
            clusters.push(ShapedCluster {
                start,
                end,
                text: slice,
                glyph_start: 0,
                glyph_end: 0,
                x_start_pt: 0.0,
                x_end_pt: 0.0,
                break_after: true,
            });
            continue;
        }

        if glyph_indices.is_empty() {
            continue;
        }
        let glyph_start = *glyph_indices.first().unwrap();
        let glyph_end = glyph_indices.last().unwrap() + 1;
        let x_start_pt = glyphs[glyph_start].x_pt - glyphs[glyph_start].x_offset_pt;
        let x_end_pt = glyphs[glyph_start..glyph_end]
            .iter()
            .map(|glyph| glyph.x_pt - glyph.x_offset_pt + glyph.advance_pt)
            .fold(x_start_pt, f32::max);
        let has_soft_break = slice
            .chars()
            .any(|ch| ch.is_whitespace() || is_common_break_punctuation(ch));
        let has_hyphen_break = match hyphens {
            Hyphens::None => false,
            Hyphens::Manual => slice.contains('\u{00AD}') || slice.contains('-'),
            Hyphens::Auto => slice.contains('\u{00AD}') || slice.contains('-'),
        };

        let mut break_after = match word_break {
            WordBreak::BreakAll => !slice.chars().all(char::is_whitespace),
            WordBreak::KeepAll => has_soft_break || has_hyphen_break,
            WordBreak::Normal | WordBreak::BreakWord => {
                has_soft_break || has_hyphen_break || thai_dictionary_breaks.contains(&end)
            }
        };

        if line_break == LineBreak::Anywhere || overflow_wrap == OverflowWrap::Anywhere {
            break_after = !slice.chars().all(char::is_whitespace);
        }

        clusters.push(ShapedCluster {
            start,
            end,
            text: slice.clone(),
            glyph_start,
            glyph_end,
            x_start_pt,
            x_end_pt,
            break_after,
        });
    }
    clusters
}

fn compose_lines(
    text: &str,
    glyphs: &[ShapedGlyph],
    clusters: &[ShapedCluster],
    max_width_pt: f32,
    align: TextAlign,
    text_justify: TextJustify,
    white_space: WhiteSpace,
    word_break: WordBreak,
    overflow_wrap: OverflowWrap,
    line_break: LineBreak,
) -> Result<Vec<TextLine>> {
    let mut lines = Vec::new();
    let mut line_start = 0usize;
    let mut idx = 0usize;
    let mut last_break = None;
    let wraps_enabled = !matches!(white_space, WhiteSpace::NoWrap | WhiteSpace::Pre);
    let allows_hard_wrap = allows_hard_wrap(word_break, overflow_wrap, line_break);

    while idx < clusters.len() {
        let cluster = &clusters[idx];
        if cluster.text == "\n" {
            push_line(
                text,
                glyphs,
                clusters,
                line_start,
                idx,
                max_width_pt,
                align,
                text_justify,
                false,
                &mut lines,
            );
            idx += 1;
            line_start = idx;
            last_break = None;
            continue;
        }

        let width = cluster.x_end_pt - clusters[line_start].x_start_pt;
        if wraps_enabled && width > max_width_pt && idx > line_start {
            let break_at = if let Some(value) = last_break.filter(|value| *value > line_start) {
                value
            } else if allows_hard_wrap {
                idx
            } else {
                idx += 1;
                continue;
            };
            push_line(
                text,
                glyphs,
                clusters,
                line_start,
                break_at,
                max_width_pt,
                align,
                text_justify,
                true,
                &mut lines,
            );
            line_start = skip_leading_spaces(clusters, break_at);
            idx = line_start;
            last_break = None;
            continue;
        }

        if cluster.break_after {
            last_break = Some(idx + 1);
        }
        idx += 1;
    }

    if line_start < clusters.len() {
        push_line(
            text,
            glyphs,
            clusters,
            line_start,
            clusters.len(),
            max_width_pt,
            align,
            text_justify,
            false,
            &mut lines,
        );
    }

    Ok(lines)
}

fn allows_hard_wrap(
    word_break: WordBreak,
    overflow_wrap: OverflowWrap,
    line_break: LineBreak,
) -> bool {
    matches!(word_break, WordBreak::BreakAll | WordBreak::BreakWord)
        || matches!(
            overflow_wrap,
            OverflowWrap::BreakWord | OverflowWrap::Anywhere
        )
        || line_break == LineBreak::Anywhere
}

fn push_line(
    _text: &str,
    glyphs: &[ShapedGlyph],
    clusters: &[ShapedCluster],
    start: usize,
    end: usize,
    max_width_pt: f32,
    align: TextAlign,
    text_justify: TextJustify,
    allow_justify: bool,
    lines: &mut Vec<TextLine>,
) {
    if start >= end || start >= clusters.len() {
        lines.push(TextLine {
            glyphs: Vec::new(),
            width_pt: 0.0,
        });
        return;
    }

    let end = trim_trailing_spaces(clusters, start, end);
    if start >= end {
        return;
    }

    let x_base = clusters[start].x_start_pt;
    let line_width_pt = clusters[end - 1].x_end_pt - x_base;
    let justification_gaps = if allow_justify && align == TextAlign::Justify {
        justification_gap_indices(clusters, start, end, text_justify)
    } else {
        Vec::new()
    };
    let justify_extra_pt = if justification_gaps.is_empty() {
        0.0
    } else {
        ((max_width_pt - line_width_pt).max(0.0)) / justification_gaps.len() as f32
    };
    let align_offset = match align {
        TextAlign::Center => ((max_width_pt - line_width_pt) / 2.0).max(0.0),
        TextAlign::Right => (max_width_pt - line_width_pt).max(0.0),
        _ => 0.0,
    };

    let mut positioned = Vec::new();
    let mut justification_offset_pt = 0.0f32;
    let mut gap_idx = 0usize;
    for (relative_idx, cluster) in clusters[start..end].iter().enumerate() {
        for glyph in &glyphs[cluster.glyph_start..cluster.glyph_end] {
            positioned.push(PositionedGlyph {
                font_key: glyph.font_key,
                glyph_id: glyph.glyph_id,
                cluster_text: cluster.text.clone(),
                cluster_start: cluster.start,
                cluster_end: cluster.end,
                x_pt: glyph.x_pt - x_base + align_offset + justification_offset_pt,
                y_offset_pt: glyph.y_offset_pt,
            });
        }

        let cluster_idx = start + relative_idx;
        if gap_idx < justification_gaps.len() && justification_gaps[gap_idx] == cluster_idx {
            justification_offset_pt += justify_extra_pt;
            gap_idx += 1;
        }
    }

    lines.push(TextLine {
        glyphs: positioned,
        width_pt: if justification_gaps.is_empty() {
            line_width_pt
        } else {
            max_width_pt.max(line_width_pt)
        },
    });
}

fn justification_gap_indices(
    clusters: &[ShapedCluster],
    start: usize,
    end: usize,
    text_justify: TextJustify,
) -> Vec<usize> {
    if end <= start + 1 || text_justify == TextJustify::None {
        return Vec::new();
    }

    match text_justify {
        TextJustify::InterWord => inter_word_gap_indices(clusters, start, end),
        TextJustify::InterCharacter => inter_character_gap_indices(clusters, start, end),
        TextJustify::Auto => {
            let word_gaps = inter_word_gap_indices(clusters, start, end);
            if word_gaps.is_empty() {
                inter_character_gap_indices(clusters, start, end)
            } else {
                word_gaps
            }
        }
        TextJustify::None => Vec::new(),
    }
}

fn inter_word_gap_indices(clusters: &[ShapedCluster], start: usize, end: usize) -> Vec<usize> {
    (start..end - 1)
        .filter(|idx| {
            clusters[*idx].text.chars().any(char::is_whitespace)
                && !clusters[*idx + 1].text.chars().all(char::is_whitespace)
        })
        .collect::<Vec<_>>()
}

fn inter_character_gap_indices(clusters: &[ShapedCluster], start: usize, end: usize) -> Vec<usize> {
    (start..end - 1)
        .filter(|idx| is_inter_character_gap(&clusters[*idx], &clusters[*idx + 1]))
        .collect::<Vec<_>>()
}

fn is_inter_character_gap(left: &ShapedCluster, right: &ShapedCluster) -> bool {
    !left.text.chars().all(char::is_whitespace) && !right.text.chars().all(char::is_whitespace)
}

fn skip_leading_spaces(clusters: &[ShapedCluster], mut idx: usize) -> usize {
    while idx < clusters.len() && clusters[idx].text.chars().all(char::is_whitespace) {
        idx += 1;
    }
    idx
}

fn trim_trailing_spaces(clusters: &[ShapedCluster], start: usize, mut end: usize) -> usize {
    while end > start && clusters[end - 1].text.chars().all(char::is_whitespace) {
        end -= 1;
    }
    end
}

fn is_common_break_punctuation(ch: char) -> bool {
    matches!(
        ch,
        '-' | '–' | '—' | '/' | ',' | '.' | ':' | ';' | ')' | ']' | '}'
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fonts::FontRegistry;
    use crate::style::{ComputedStyle, WhiteSpace};
    use crate::types::RenderOptions;

    #[test]
    fn splits_newline_into_dedicated_cluster() {
        let clusters = split_logical_clusters("a\nข");
        let texts = clusters
            .into_iter()
            .map(|value| value.text)
            .collect::<Vec<_>>();
        assert_eq!(texts, vec!["a", "\n", "ข"]);
    }

    #[test]
    fn keeps_thai_sara_am_and_tone_marks_in_one_cluster() {
        let clusters = split_logical_clusters("น้ำ ก่ำ เก๋า");
        let texts = clusters
            .into_iter()
            .map(|value| value.text)
            .collect::<Vec<_>>();
        assert_eq!(texts, vec!["น้ำ", " ", "ก่ำ", " ", "เก๋า"]);
    }

    #[test]
    fn expands_sara_am_before_tone_marks_for_shaping() {
        let expanded = shape_text_for_cluster("น้ำ");
        assert_eq!(expanded, "นํ้า");
        let expanded = shape_text_for_cluster("ก่ำ");
        assert_eq!(expanded, "กํ่า");
    }

    #[test]
    fn shaping_feature_specs_follow_options() {
        let mut options = RenderOptions::default();
        options.enable_kerning = false;
        options.enable_ligatures = false;
        assert_eq!(shaping_feature_specs(&options), ["-kern", "-liga", "-clig"]);
    }

    #[test]
    fn ignores_invalid_font_variations() {
        let values = vec!["wght=700".to_string(), "invalid-axis".to_string()];
        let variations = parse_font_variations(&values);
        assert_eq!(variations.len(), 1);
    }

    #[test]
    fn thai_dictionary_breaks_prefer_word_boundaries() {
        let text = "ประเทศไทยเป็นประเทศ";
        let breaks = thai_dictionary_break_positions(text);
        assert!(breaks.contains(&"ประเทศไทย".len()));
        assert!(breaks.contains(&"ประเทศไทยเป็น".len()));
    }

    #[test]
    fn thai_dictionary_matching_is_longest_prefix() {
        let text = "ประเทศไทย";
        assert_eq!(longest_dictionary_match_at(text, 0), Some(text.len()));
    }

    #[test]
    fn shapes_thai_when_fonts_are_available() {
        let Ok(fonts) = FontRegistry::discover("Sarabun") else {
            return;
        };
        let style = ComputedStyle::root("Sarabun", 14.0);
        let lines = shape_lines(
            "น้ำพริก ผู้ใหญ่",
            &style,
            300.0,
            &fonts,
            &RenderOptions::default(),
        )
        .unwrap();
        assert!(!lines.is_empty());
        assert!(lines.iter().flat_map(|line| &line.glyphs).count() > 4);
    }

    #[test]
    fn shaped_thai_marks_keep_original_cluster_text() {
        let Ok(fonts) = FontRegistry::discover("Sarabun") else {
            return;
        };
        let style = ComputedStyle::root("Sarabun", 14.0);
        let lines = shape_lines("น้ำ", &style, 300.0, &fonts, &RenderOptions::default()).unwrap();
        let glyphs = lines
            .iter()
            .flat_map(|line| &line.glyphs)
            .collect::<Vec<_>>();
        assert!(glyphs.len() >= 3);
        assert!(glyphs.iter().all(|glyph| glyph.cluster_text == "น้ำ"));
    }

    #[test]
    fn pre_wrap_preserves_hard_newlines_and_indentation() {
        let Ok(fonts) = FontRegistry::discover("Sarabun") else {
            return;
        };
        let mut style = ComputedStyle::root("Sarabun", 10.0);
        style.white_space = WhiteSpace::PreWrap;

        let lines = shape_lines(
            "def run():\n    return 1",
            &style,
            1_000.0,
            &fonts,
            &RenderOptions::default(),
        )
        .unwrap();

        assert_eq!(lines.len(), 2);
        assert_eq!(line_cluster_text(&lines[0]), "def run():");
        assert_eq!(line_cluster_text(&lines[1]), "    return 1");
    }

    fn line_cluster_text(line: &TextLine) -> String {
        let mut text = String::new();
        let mut active_cluster = (usize::MAX, usize::MAX);
        for glyph in &line.glyphs {
            let cluster = (glyph.cluster_start, glyph.cluster_end);
            if cluster == active_cluster {
                continue;
            }
            active_cluster = cluster;
            text.push_str(&glyph.cluster_text);
        }
        text
    }
}
