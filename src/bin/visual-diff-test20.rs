use image::{ImageBuffer, Rgba, RgbaImage};
use lynpdf_rs::{render_file_to_pdf, RenderOptions};
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const DEFAULT_DIFF_THRESHOLD: f32 = 0.18;
const PIXEL_DIFF_THRESHOLD: u8 = 24;
const AA_PIXEL_DIFF_THRESHOLD: u8 = 14;
const DEFAULT_AA_SIGMA: f32 = 0.85;
const DEFAULT_AA_WEIGHT: f32 = 0.35;
const EDGE_GRADIENT_THRESHOLD: i16 = 48;

#[derive(Debug, Clone)]
struct Config {
    update_baseline: bool,
    strict: bool,
    diff_threshold: f32,
    browser_reference_png: Option<PathBuf>,
    aa_sigma: f32,
    aa_weight: f32,
}

#[derive(Debug, Clone, Copy)]
struct DiffStats {
    diff_ratio: f32,
    mean_abs: f32,
    rmse: f32,
    differing_pixels: u64,
    total_pixels: u64,
}

#[derive(Debug, Clone, Copy)]
struct EdgeMismatchStats {
    mismatch_ratio: f32,
    mismatch_edges: u64,
    union_edges: u64,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("visual-diff-test20: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let config = parse_args()?;

    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixture_html = root.join("tests/fixtures/test-20-border-style-parity.html");
    let fixture_css = root.join("tests/fixtures/test-styles.css");

    let output_dir = root.join("tests/output/visual-diff/test-20");
    let baseline_dir = root.join("tests/baseline/browser");
    fs::create_dir_all(&output_dir)?;
    fs::create_dir_all(&baseline_dir)?;

    let rust_pdf = output_dir.join("test-20-rust.pdf");
    let rust_png = output_dir.join("test-20-rust-page1.png");
    let browser_html = output_dir.join("test-20-browser-input.html");
    let browser_current_png = output_dir.join("test-20-browser-current.png");
    let baseline_png = baseline_dir.join("test-20-border-style-parity-reference.png");
    let diff_png = output_dir.join("test-20-rust-vs-browser-diff.png");
    let aa_diff_png = output_dir.join("test-20-rust-vs-browser-diff-aa.png");
    let summary_md = output_dir.join("test-20-visual-diff-summary.md");

    let render = render_file_to_pdf(
        &fixture_html,
        Some(&fixture_css),
        &rust_pdf,
        RenderOptions::default(),
    )?;
    println!(
        "Rust render: {} page(s), {} bytes -> {}",
        render.pages,
        render.bytes.len(),
        rel(&rust_pdf, &root)
    );

    rasterize_pdf_first_page(&rust_pdf, &rust_png)?;
    let rust_image = image::open(&rust_png)?.to_rgba8();
    let (width, height) = rust_image.dimensions();

    let browser_input = inject_css_into_fixture(&fixture_html, &fixture_css)?;
    fs::write(&browser_html, browser_input)?;

    let baseline_existed = baseline_png.exists();
    let (browser_source_label, captured_browser_snapshot) = if let Some(
        external_browser_reference,
    ) = &config.browser_reference_png
    {
        fs::copy(external_browser_reference, &browser_current_png)?;
        (
            format!("external ({})", external_browser_reference.display()),
            true,
        )
    } else {
        match capture_browser_screenshot(&browser_html, &browser_current_png, width, height) {
            Ok(()) => ("captured-local-browser".to_string(), true),
            Err(err) => {
                if config.update_baseline {
                    return Err(err);
                }
                if baseline_existed {
                    eprintln!(
                        "warning: browser capture unavailable ({}), using stored baseline for comparison",
                        err
                    );
                    fs::copy(&baseline_png, &browser_current_png)?;
                    ("baseline-fallback".to_string(), false)
                } else {
                    return Err(err);
                }
            }
        }
    };

    if config.update_baseline || !baseline_existed {
        if !browser_current_png.exists() {
            return Err("cannot update baseline without a browser reference snapshot".into());
        }
        fs::copy(&browser_current_png, &baseline_png)?;
        println!("Baseline updated: {}", rel(&baseline_png, &root));
    }

    let baseline_original = image::open(&baseline_png)?.to_rgba8();
    let (baseline_w, baseline_h) = baseline_original.dimensions();
    let baseline_image = if baseline_w != width || baseline_h != height {
        eprintln!(
            "warning: baseline dimensions {}x{} differ from rust {}x{}; resizing baseline for comparison",
            baseline_w, baseline_h, width, height
        );
        image::imageops::resize(
            &baseline_original,
            width,
            height,
            image::imageops::FilterType::Triangle,
        )
    } else {
        baseline_original
    };

    let (diff_image, rust_vs_baseline_raw) =
        diff_images(&rust_image, &baseline_image, PIXEL_DIFF_THRESHOLD);
    diff_image.save(&diff_png)?;

    let rust_blurred = image::imageops::blur(&rust_image, config.aa_sigma.max(0.0));
    let baseline_blurred = image::imageops::blur(&baseline_image, config.aa_sigma.max(0.0));
    let (aa_diff_image, rust_vs_baseline_aa) =
        diff_images(&rust_blurred, &baseline_blurred, AA_PIXEL_DIFF_THRESHOLD);
    aa_diff_image.save(&aa_diff_png)?;
    let edge_phase_stats =
        edge_phase_mismatch(&rust_blurred, &baseline_blurred, EDGE_GRADIENT_THRESHOLD);

    let rust_vs_baseline_effective = blend_diff_ratio(
        rust_vs_baseline_raw.diff_ratio,
        rust_vs_baseline_aa.diff_ratio,
        config.aa_weight,
    );

    let browser_drift = if baseline_existed && captured_browser_snapshot {
        let current_browser_raw = image::open(&browser_current_png)?.to_rgba8();
        let current_browser = if current_browser_raw.dimensions() != baseline_image.dimensions() {
            image::imageops::resize(
                &current_browser_raw,
                baseline_image.width(),
                baseline_image.height(),
                image::imageops::FilterType::Triangle,
            )
        } else {
            current_browser_raw
        };
        let (_, drift) = diff_images(&current_browser, &baseline_image, PIXEL_DIFF_THRESHOLD);
        Some(drift)
    } else {
        None
    };

    let pass = rust_vs_baseline_effective <= config.diff_threshold;
    write_summary(
        &summary_md,
        &root,
        &config,
        &rust_pdf,
        &rust_png,
        &browser_current_png,
        &browser_source_label,
        &baseline_png,
        &diff_png,
        &aa_diff_png,
        rust_vs_baseline_raw,
        rust_vs_baseline_aa,
        rust_vs_baseline_effective,
        edge_phase_stats,
        browser_drift,
        pass,
    )?;

    println!(
        "Rust vs baseline diff ratio: raw {:.4}, aa {:.4}, effective {:.4} (threshold {:.4}) -> {}",
        rust_vs_baseline_raw.diff_ratio,
        rust_vs_baseline_aa.diff_ratio,
        rust_vs_baseline_effective,
        config.diff_threshold,
        if pass { "PASS" } else { "FAIL" }
    );
    println!(
        "Edge-phase mismatch ratio: {:.4} ({} / {} edge pixels)",
        edge_phase_stats.mismatch_ratio,
        edge_phase_stats.mismatch_edges,
        edge_phase_stats.union_edges
    );
    println!("Summary: {}", rel(&summary_md, &root));

    if !pass && config.strict {
        return Err(format!(
            "visual diff failed (effective ratio {:.4} > {:.4})",
            rust_vs_baseline_effective, config.diff_threshold
        )
        .into());
    }

    Ok(())
}

fn parse_args() -> Result<Config, Box<dyn Error>> {
    let mut config = Config {
        update_baseline: false,
        strict: true,
        diff_threshold: DEFAULT_DIFF_THRESHOLD,
        browser_reference_png: None,
        aa_sigma: DEFAULT_AA_SIGMA,
        aa_weight: DEFAULT_AA_WEIGHT,
    };

    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--update-baseline" => config.update_baseline = true,
            "--strict" => config.strict = true,
            "--no-strict" => config.strict = false,
            "--threshold" => {
                let raw = args.next().ok_or("--threshold requires a numeric value")?;
                let value = raw.parse::<f32>()?;
                if !(0.0..=1.0).contains(&value) {
                    return Err("--threshold must be in [0.0, 1.0]".into());
                }
                config.diff_threshold = value;
            }
            "--browser-reference-png" => {
                let raw = args
                    .next()
                    .ok_or("--browser-reference-png requires a file path")?;
                config.browser_reference_png = Some(PathBuf::from(raw));
            }
            "--aa-sigma" => {
                let raw = args.next().ok_or("--aa-sigma requires a numeric value")?;
                let value = raw.parse::<f32>()?;
                if value < 0.0 {
                    return Err("--aa-sigma must be >= 0".into());
                }
                config.aa_sigma = value;
            }
            "--aa-weight" => {
                let raw = args.next().ok_or("--aa-weight requires a numeric value")?;
                let value = raw.parse::<f32>()?;
                if !(0.0..=1.0).contains(&value) {
                    return Err("--aa-weight must be in [0.0, 1.0]".into());
                }
                config.aa_weight = value;
            }
            "--help" | "-h" => {
                print_help();
                std::process::exit(0);
            }
            other => return Err(format!("unknown argument: {other}").into()),
        }
    }

    Ok(config)
}

fn print_help() {
    println!(
        "Visual diff runner for test-20\n\nUsage:\n  cargo run --bin visual-diff-test20 -- [--update-baseline] [--threshold <0..1>] [--strict|--no-strict] [--browser-reference-png <path>] [--aa-sigma <value>] [--aa-weight <0..1>]\n\nDefaults:\n  --strict\n  --threshold 0.18\n  --aa-sigma 0.85\n  --aa-weight 0.35\n\nNotes:\n  - If no local headless browser (Chromium/Firefox) is available, provide --browser-reference-png or keep a stored baseline image."
    );
}

fn inject_css_into_fixture(
    fixture_html: &Path,
    fixture_css: &Path,
) -> Result<String, Box<dyn Error>> {
    let html = fs::read_to_string(fixture_html)?;
    let css = fs::read_to_string(fixture_css)?;
    let style_tag = format!("<style>\n{}\n</style>\n", css);
    if html.contains("</head>") {
        Ok(html.replacen("</head>", &format!("{}\n</head>", style_tag), 1))
    } else {
        Ok(format!("{}\n{}", style_tag, html))
    }
}

fn rasterize_pdf_first_page(pdf_path: &Path, png_out: &Path) -> Result<(), Box<dyn Error>> {
    let prefix = png_out.with_extension("");
    let output = Command::new("pdftoppm")
        .arg("-f")
        .arg("1")
        .arg("-singlefile")
        .arg("-png")
        .arg(pdf_path)
        .arg(&prefix)
        .output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "pdftoppm failed (is poppler-utils installed?): {}",
            stderr.trim()
        )
        .into());
    }

    if !png_out.exists() {
        return Err(format!("expected rasterized file not found: {}", png_out.display()).into());
    }
    Ok(())
}

fn capture_browser_screenshot(
    html_path: &Path,
    screenshot_path: &Path,
    width: u32,
    height: u32,
) -> Result<(), Box<dyn Error>> {
    let browser = find_browser_binary().ok_or(
        "no supported headless browser found (checked: chromium, chromium-browser, google-chrome, google-chrome-stable, firefox)",
    )?;

    let html_url = format!("file://{}", html_path.canonicalize()?.display());
    let mut cmd = Command::new(browser);
    if browser == "firefox" {
        cmd.arg("--headless")
            .arg("--screenshot")
            .arg(screenshot_path)
            .arg("--window-size")
            .arg(format!("{},{}", width, height))
            .arg(&html_url);
    } else {
        cmd.arg("--headless")
            .arg("--disable-gpu")
            .arg("--hide-scrollbars")
            .arg("--force-device-scale-factor=1")
            .arg("--run-all-compositor-stages-before-draw")
            .arg("--virtual-time-budget=2000")
            .arg(format!("--window-size={},{}", width, height))
            .arg(format!("--screenshot={}", screenshot_path.display()))
            .arg(&html_url);
    }

    let output = cmd.output()?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("browser screenshot command failed: {}", stderr.trim()).into());
    }

    if !screenshot_path.exists() {
        return Err(format!(
            "browser screenshot not found: {}",
            screenshot_path.display()
        )
        .into());
    }

    Ok(())
}

fn find_browser_binary() -> Option<&'static str> {
    for candidate in [
        "chromium",
        "chromium-browser",
        "google-chrome",
        "google-chrome-stable",
        "firefox",
    ] {
        let status = Command::new(candidate)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
        if status.as_ref().is_ok_and(|value| value.success()) {
            return Some(candidate);
        }
    }
    None
}

fn diff_images(
    left: &RgbaImage,
    right: &RgbaImage,
    pixel_diff_threshold: u8,
) -> (RgbaImage, DiffStats) {
    let width = left.width().min(right.width());
    let height = left.height().min(right.height());
    let mut diff = ImageBuffer::new(width, height);

    let mut differing_pixels = 0u64;
    let mut sum_abs = 0.0f64;
    let mut sum_sq = 0.0f64;

    for y in 0..height {
        for x in 0..width {
            let a = left.get_pixel(x, y).0;
            let b = right.get_pixel(x, y).0;

            let dr = u8::abs_diff(a[0], b[0]);
            let dg = u8::abs_diff(a[1], b[1]);
            let db = u8::abs_diff(a[2], b[2]);
            let da = u8::abs_diff(a[3], b[3]);
            let max_diff = dr.max(dg).max(db).max(da);

            let avg_abs = (dr as f64 + dg as f64 + db as f64 + da as f64) / (4.0 * 255.0);
            sum_abs += avg_abs;

            let norm_dr = dr as f64 / 255.0;
            let norm_dg = dg as f64 / 255.0;
            let norm_db = db as f64 / 255.0;
            let norm_da = da as f64 / 255.0;
            sum_sq +=
                (norm_dr * norm_dr + norm_dg * norm_dg + norm_db * norm_db + norm_da * norm_da)
                    / 4.0;

            if max_diff > pixel_diff_threshold {
                differing_pixels += 1;
                let intensity = (max_diff as f32 / 255.0).clamp(0.0, 1.0);
                let heat = (64.0 + 191.0 * intensity) as u8;
                diff.put_pixel(x, y, Rgba([255, 255 - heat, 0, 255]));
            } else {
                let base = ((a[0] as u16 + a[1] as u16 + a[2] as u16) / 3) as u8;
                diff.put_pixel(x, y, Rgba([base / 3, base / 3, base / 3, 255]));
            }
        }
    }

    let total_pixels = (width as u64).saturating_mul(height as u64).max(1);
    let diff_ratio = differing_pixels as f32 / total_pixels as f32;
    let mean_abs = (sum_abs / total_pixels as f64) as f32;
    let rmse = (sum_sq / total_pixels as f64).sqrt() as f32;

    (
        diff,
        DiffStats {
            diff_ratio,
            mean_abs,
            rmse,
            differing_pixels,
            total_pixels,
        },
    )
}

fn blend_diff_ratio(raw_ratio: f32, aa_ratio: f32, aa_weight: f32) -> f32 {
    let aa_weight = aa_weight.clamp(0.0, 1.0);
    let aa_relaxed = aa_ratio.min(raw_ratio);
    raw_ratio * (1.0 - aa_weight) + aa_relaxed * aa_weight
}

fn edge_phase_mismatch(
    left: &RgbaImage,
    right: &RgbaImage,
    gradient_threshold: i16,
) -> EdgeMismatchStats {
    let width = left.width().min(right.width());
    let height = left.height().min(right.height());
    if width < 3 || height < 3 {
        return EdgeMismatchStats {
            mismatch_ratio: 0.0,
            mismatch_edges: 0,
            union_edges: 0,
        };
    }

    let width_usize = width as usize;
    let height_usize = height as usize;
    let mut left_edges = vec![false; width_usize * height_usize];
    let mut right_edges = vec![false; width_usize * height_usize];

    let mut left_count = 0u64;
    let mut right_count = 0u64;
    for y in 1..(height - 1) {
        for x in 1..(width - 1) {
            let left_edge = is_edge_pixel(left, x, y, gradient_threshold);
            let right_edge = is_edge_pixel(right, x, y, gradient_threshold);

            let idx = y as usize * width_usize + x as usize;
            left_edges[idx] = left_edge;
            right_edges[idx] = right_edge;
            if left_edge {
                left_count += 1;
            }
            if right_edge {
                right_count += 1;
            }
        }
    }

    let mut unmatched_left = 0u64;
    let mut unmatched_right = 0u64;
    for y in 1..(height_usize - 1) {
        for x in 1..(width_usize - 1) {
            let idx = y * width_usize + x;
            if left_edges[idx]
                && !has_edge_neighbor(&right_edges, width_usize, height_usize, x, y, 1)
            {
                unmatched_left += 1;
            }
            if right_edges[idx]
                && !has_edge_neighbor(&left_edges, width_usize, height_usize, x, y, 1)
            {
                unmatched_right += 1;
            }
        }
    }

    let mismatch_edges = unmatched_left + unmatched_right;
    let union_edges = left_count + right_count;
    let mismatch_ratio = if union_edges > 0 {
        mismatch_edges as f32 / union_edges as f32
    } else {
        0.0
    };
    EdgeMismatchStats {
        mismatch_ratio,
        mismatch_edges,
        union_edges,
    }
}

fn is_edge_pixel(image: &RgbaImage, x: u32, y: u32, gradient_threshold: i16) -> bool {
    let l = luminance(image.get_pixel(x - 1, y).0);
    let r = luminance(image.get_pixel(x + 1, y).0);
    let t = luminance(image.get_pixel(x, y - 1).0);
    let b = luminance(image.get_pixel(x, y + 1).0);
    let gx = (r - l).abs();
    let gy = (b - t).abs();
    gx + gy >= gradient_threshold
}

fn has_edge_neighbor(
    edges: &[bool],
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    radius: usize,
) -> bool {
    let min_x = x.saturating_sub(radius);
    let min_y = y.saturating_sub(radius);
    let max_x = (x + radius).min(width - 1);
    let max_y = (y + radius).min(height - 1);
    for yy in min_y..=max_y {
        for xx in min_x..=max_x {
            if edges[yy * width + xx] {
                return true;
            }
        }
    }
    false
}

fn luminance(px: [u8; 4]) -> i16 {
    // Weighted integer approximation of Rec.601 luma for edge extraction.
    (px[0] as i16 * 30 + px[1] as i16 * 59 + px[2] as i16 * 11) / 100
}

#[allow(clippy::too_many_arguments)]
fn write_summary(
    summary_path: &Path,
    root: &Path,
    config: &Config,
    rust_pdf: &Path,
    rust_png: &Path,
    browser_current_png: &Path,
    browser_source_label: &str,
    baseline_png: &Path,
    diff_png: &Path,
    aa_diff_png: &Path,
    rust_vs_baseline_raw: DiffStats,
    rust_vs_baseline_aa: DiffStats,
    rust_vs_baseline_effective: f32,
    edge_phase_stats: EdgeMismatchStats,
    browser_drift: Option<DiffStats>,
    pass: bool,
) -> Result<(), Box<dyn Error>> {
    let mut lines = Vec::new();
    lines.push("# Test-20 Visual Diff Summary".to_string());
    lines.push("".to_string());
    lines.push(format!("- Result: {}", if pass { "PASS" } else { "FAIL" }));
    lines.push(format!(
        "- Threshold (diff_ratio): {:.4}",
        config.diff_threshold
    ));
    lines.push(format!(
        "- Strict mode: {}",
        if config.strict { "on" } else { "off" }
    ));
    lines.push("".to_string());
    lines.push("## Metrics (Rust vs Browser Baseline)".to_string());
    lines.push(format!(
        "- diff_ratio_raw: {:.6}",
        rust_vs_baseline_raw.diff_ratio
    ));
    lines.push(format!(
        "- diff_ratio_aa_compensated: {:.6}",
        rust_vs_baseline_aa.diff_ratio
    ));
    lines.push(format!(
        "- diff_ratio_effective: {:.6}",
        rust_vs_baseline_effective
    ));
    lines.push(format!(
        "- mean_abs_raw: {:.6}",
        rust_vs_baseline_raw.mean_abs
    ));
    lines.push(format!("- rmse_raw: {:.6}", rust_vs_baseline_raw.rmse));
    lines.push(format!(
        "- mean_abs_aa_compensated: {:.6}",
        rust_vs_baseline_aa.mean_abs
    ));
    lines.push(format!(
        "- rmse_aa_compensated: {:.6}",
        rust_vs_baseline_aa.rmse
    ));
    lines.push(format!(
        "- differing_pixels_raw: {} / {}",
        rust_vs_baseline_raw.differing_pixels, rust_vs_baseline_raw.total_pixels
    ));
    lines.push(format!(
        "- edge_phase_mismatch_ratio: {:.6} ({} / {})",
        edge_phase_stats.mismatch_ratio,
        edge_phase_stats.mismatch_edges,
        edge_phase_stats.union_edges
    ));
    lines.push(format!("- aa_sigma: {:.3}", config.aa_sigma));
    lines.push(format!("- aa_weight: {:.3}", config.aa_weight));
    lines.push("".to_string());

    if let Some(drift) = browser_drift {
        lines.push("## Metrics (Current Browser Snapshot vs Stored Baseline)".to_string());
        lines.push(format!("- diff_ratio: {:.6}", drift.diff_ratio));
        lines.push(format!("- mean_abs: {:.6}", drift.mean_abs));
        lines.push(format!("- rmse: {:.6}", drift.rmse));
        lines.push("".to_string());
    }

    lines.push("## Artifacts".to_string());
    lines.push(format!("- Rust PDF: {}", rel(rust_pdf, root)));
    lines.push(format!("- Rust page PNG: {}", rel(rust_png, root)));
    lines.push(format!(
        "- Browser snapshot (current): {}",
        rel(browser_current_png, root)
    ));
    lines.push(format!(
        "- Browser snapshot source: {}",
        browser_source_label
    ));
    lines.push(format!(
        "- Browser baseline (stored): {}",
        rel(baseline_png, root)
    ));
    lines.push(format!("- Diff heatmap: {}", rel(diff_png, root)));
    lines.push(format!(
        "- Diff heatmap (AA-compensated): {}",
        rel(aa_diff_png, root)
    ));
    lines.push("".to_string());

    fs::write(summary_path, lines.join("\n"))?;
    Ok(())
}

fn rel(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}
