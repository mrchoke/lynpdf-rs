use lynpdf_rs::LYNPDF_RS_VERSION;
use lynpdf_rs::{render_file_to_pdf, RenderFontStyle, RenderOptions, UserFontMapping};
use std::path::PathBuf;

fn main() {
    if let Err(err) = run() {
        eprintln!("lynpdf-rs: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let mut input = None;
    let mut css = None;
    let mut output = None;
    let mut verbose = false;
    let mut user_font_dirs = Vec::<PathBuf>::new();
    let mut user_font_mappings = Vec::<UserFontMapping>::new();
    let mut enable_kerning = true;
    let mut enable_ligatures = true;
    let mut font_variations = Vec::<String>::new();
    let mut page_scale_percent = 100.0f32;
    let mut fit_to_page = false;
    let mut enable_syntax_highlighting = true;
    let mut syntax_highlight_theme = "lynpdf-light".to_string();

    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-h" | "--help" => {
                print_help();
                return Ok(());
            }
            "-i" | "--input" => input = args.next().map(PathBuf::from),
            "-c" | "--css" => css = args.next().map(PathBuf::from),
            "-o" | "--output" => output = args.next().map(PathBuf::from),
            "--version" => {
                println!("lynpdf-rs {}", LYNPDF_RS_VERSION);
                return Ok(());
            }
            "-v" | "--verbose" => verbose = true,
            "--font-dir" => {
                let value = args.next().ok_or("--font-dir requires a path")?;
                user_font_dirs.push(PathBuf::from(value));
            }
            "--font-map" => {
                let value = args.next().ok_or("--font-map requires FAMILY=PATH")?;
                user_font_mappings.push(parse_font_map(&value, 400, RenderFontStyle::Normal)?);
            }
            "--font-map-bold" => {
                let value = args.next().ok_or("--font-map-bold requires FAMILY=PATH")?;
                user_font_mappings.push(parse_font_map(&value, 700, RenderFontStyle::Normal)?);
            }
            "--font-map-italic" => {
                let value = args
                    .next()
                    .ok_or("--font-map-italic requires FAMILY=PATH")?;
                user_font_mappings.push(parse_font_map(&value, 400, RenderFontStyle::Italic)?);
            }
            "--font-map-bold-italic" => {
                let value = args
                    .next()
                    .ok_or("--font-map-bold-italic requires FAMILY=PATH")?;
                user_font_mappings.push(parse_font_map(&value, 700, RenderFontStyle::Italic)?);
            }
            "--no-kerning" => enable_kerning = false,
            "--no-ligatures" => enable_ligatures = false,
            "--font-variation" => {
                let value = args.next().ok_or("--font-variation requires AXIS=VALUE")?;
                font_variations.push(value);
            }
            "--no-syntax-highlight" => enable_syntax_highlighting = false,
            "--syntax-theme" => {
                let value = args.next().ok_or("--syntax-theme requires a theme name")?;
                syntax_highlight_theme = value;
            }
            "--page-scale" => {
                let value = args
                    .next()
                    .ok_or("--page-scale requires a numeric percent")?;
                page_scale_percent = value
                    .parse::<f32>()
                    .map_err(|_| "--page-scale must be a number")?;
            }
            "--fit-to-page" => fit_to_page = true,
            value if input.is_none() => input = Some(PathBuf::from(value)),
            value => return Err(format!("unexpected argument: {value}").into()),
        }
    }

    let input = input.ok_or("missing input HTML path")?;
    let output = output.unwrap_or_else(|| input.with_extension("pdf"));
    let options = RenderOptions {
        verbose,
        user_font_dirs,
        user_font_mappings,
        enable_kerning,
        enable_ligatures,
        font_variations,
        page_scale_percent,
        fit_to_page,
        enable_syntax_highlighting,
        syntax_highlight_theme,
        ..RenderOptions::default()
    };
    let result = render_file_to_pdf(&input, css.as_ref(), &output, options)?;
    eprintln!(
        "wrote {} ({} page(s), {} bytes)",
        output.display(),
        result.pages,
        result.bytes.len()
    );
    for diagnostic in result.diagnostics {
        if verbose {
            eprintln!(
                "{:?} {}: {}",
                diagnostic.level, diagnostic.code, diagnostic.message
            );
        }
    }
    Ok(())
}

fn parse_font_map(
    raw: &str,
    weight: u16,
    style: RenderFontStyle,
) -> Result<UserFontMapping, Box<dyn std::error::Error>> {
    let (family, path) = raw
        .split_once('=')
        .ok_or("font mapping must be in FAMILY=PATH format")?;
    let family = family.trim();
    let path = path.trim();
    if family.is_empty() || path.is_empty() {
        return Err("font mapping must include both family and path".into());
    }
    Ok(UserFontMapping::new(family, PathBuf::from(path))
        .with_weight(weight)
        .with_style(style))
}

fn print_help() {
    println!(
        "LynPDF RS\n\nUsage:\n  lynpdf-rs <input.html> [-c styles.css] [-o output.pdf] [--verbose]\n    [--font-dir DIR]\n    [--font-map FAMILY=PATH]\n    [--font-map-bold FAMILY=PATH]\n    [--font-map-italic FAMILY=PATH]\n    [--font-map-bold-italic FAMILY=PATH]\n    [--no-kerning]\n    [--no-ligatures]\n    [--font-variation AXIS=VALUE]\n    [--no-syntax-highlight]\n    [--syntax-theme NAME]\n    [--page-scale PERCENT]\n    [--fit-to-page]\n    [--version]\n\nEnvironment:\n  LYNPDF_FONT_DIRS=dir1:dir2   Additional runtime font directories"
    );
}
