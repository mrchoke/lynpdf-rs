use image::{ImageBuffer, Rgba, RgbaImage};
use lynpdf_rs::{render_file_to_pdf, RenderOptions};
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const DEFAULT_DIFF_THRESHOLD: f32 = 0.02;
const PIXEL_DIFF_THRESHOLD: u8 = 20;

#[derive(Debug, Clone)]
struct Config {
    update_baseline: bool,
    strict: bool,
    diff_threshold: f32,
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
struct ExampleCase {
    slug: &'static str,
    html: &'static str,
}

const CASES: &[ExampleCase] = &[
    ExampleCase {
        slug: "book-lynpdf-guide",
        html: "examples/book-lynpdf-guide.html",
    },
    ExampleCase {
        slug: "demo-certificate",
        html: "examples/demo-certificate.html",
    },
    ExampleCase {
        slug: "demo-certificate-portrait",
        html: "examples/demo-certificate-portrait.html",
    },
    ExampleCase {
        slug: "demo-certificate-landscape",
        html: "examples/demo-certificate-landscape.html",
    },
    ExampleCase {
        slug: "demo-invoice",
        html: "examples/demo-invoice.html",
    },
    ExampleCase {
        slug: "invoice",
        html: "examples/invoice.html",
    },
    ExampleCase {
        slug: "demo-font-showcase",
        html: "examples/demo-font-showcase.html",
    },
    ExampleCase {
        slug: "demo-thai-mark-positioning",
        html: "examples/demo-thai-mark-positioning.html",
    },
    ExampleCase {
        slug: "layout",
        html: "examples/layout.html",
    },
];

fn main() {
    if let Err(err) = run() {
        eprintln!("visual-diff-examples: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let config = parse_args()?;
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    let output_dir = root.join("tests/output/visual-diff/examples");
    let baseline_dir = root.join("tests/baseline/rust-examples");
    fs::create_dir_all(&output_dir)?;
    fs::create_dir_all(&baseline_dir)?;

    let mut summary_lines = Vec::new();
    summary_lines.push("# Example Visual Diff Summary".to_string());
    summary_lines.push("".to_string());
    summary_lines.push(format!("- threshold: {:.4}", config.diff_threshold));
    summary_lines.push(format!(
        "- strict: {}",
        if config.strict { "on" } else { "off" }
    ));
    summary_lines.push(format!(
        "- update_baseline: {}",
        if config.update_baseline { "yes" } else { "no" }
    ));
    summary_lines.push("".to_string());
    summary_lines.push("| Case | Diff Ratio | Mean Abs | RMSE | Pixels | Result |".to_string());
    summary_lines.push("|---|---:|---:|---:|---:|---|".to_string());

    let mut failed_cases = Vec::new();

    for case in CASES {
        let html_path = root.join(case.html);
        let css_path = root.join("examples/styles.css");

        let rust_pdf = output_dir.join(format!("{}-rust.pdf", case.slug));
        let rust_png = output_dir.join(format!("{}-rust-page1.png", case.slug));
        let baseline_png = baseline_dir.join(format!("{}-reference.png", case.slug));
        let diff_png = output_dir.join(format!("{}-rust-vs-baseline-diff.png", case.slug));

        let render = render_file_to_pdf(
            &html_path,
            Some(&css_path),
            &rust_pdf,
            RenderOptions::default(),
        )?;
        println!(
            "rendered {} page(s) for {} -> {}",
            render.pages,
            case.slug,
            rel(&rust_pdf, &root)
        );

        rasterize_pdf_first_page(&rust_pdf, &rust_png)?;
        let current_image = image::open(&rust_png)?.to_rgba8();

        if config.update_baseline || !baseline_png.exists() {
            fs::copy(&rust_png, &baseline_png)?;
            println!("baseline updated: {}", rel(&baseline_png, &root));
        }

        let baseline_original = image::open(&baseline_png)?.to_rgba8();
        let baseline = if baseline_original.dimensions() != current_image.dimensions() {
            image::imageops::resize(
                &baseline_original,
                current_image.width(),
                current_image.height(),
                image::imageops::FilterType::Triangle,
            )
        } else {
            baseline_original
        };

        let (diff_image, stats) = diff_images(&current_image, &baseline, PIXEL_DIFF_THRESHOLD);
        diff_image.save(&diff_png)?;

        let pass = stats.diff_ratio <= config.diff_threshold;
        if !pass {
            failed_cases.push(case.slug.to_string());
        }

        summary_lines.push(format!(
            "| {} | {:.6} | {:.6} | {:.6} | {} / {} | {} |",
            case.slug,
            stats.diff_ratio,
            stats.mean_abs,
            stats.rmse,
            stats.differing_pixels,
            stats.total_pixels,
            if pass { "PASS" } else { "FAIL" }
        ));
    }

    summary_lines.push("".to_string());
    summary_lines.push("## Artifacts".to_string());
    summary_lines.push(format!("- baseline_dir: {}", rel(&baseline_dir, &root)));
    summary_lines.push(format!("- output_dir: {}", rel(&output_dir, &root)));

    let summary_path = output_dir.join("visual-diff-examples-summary.md");
    fs::write(&summary_path, summary_lines.join("\n"))?;
    println!("summary: {}", rel(&summary_path, &root));

    if config.strict && !failed_cases.is_empty() {
        return Err(format!("visual diff failed for cases: {}", failed_cases.join(", ")).into());
    }

    Ok(())
}

fn parse_args() -> Result<Config, Box<dyn Error>> {
    let mut config = Config {
        update_baseline: false,
        strict: true,
        diff_threshold: DEFAULT_DIFF_THRESHOLD,
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
        "Visual diff runner for refreshed examples\n\nUsage:\n  cargo run --bin visual-diff-examples -- [--update-baseline] [--threshold <0..1>] [--strict|--no-strict]\n\nDefaults:\n  --strict\n  --threshold 0.02"
    );
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

fn rel(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}
