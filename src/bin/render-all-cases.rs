use lynpdf_rs::{render_file_to_pdf, RenderOptions};
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Scope {
    All,
    Tests,
    Examples,
}

#[derive(Debug, Clone)]
struct Case {
    group: &'static str,
    input: PathBuf,
    css: Option<PathBuf>,
    output: PathBuf,
    fit_to_page: bool,
}

fn main() {
    if let Err(err) = run() {
        eprintln!("render-all-cases: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn Error>> {
    let mut scope = Scope::All;
    let mut verbose = false;
    let mut stop_on_error = false;

    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "--all" => scope = Scope::All,
            "--tests" => scope = Scope::Tests,
            "--examples" => scope = Scope::Examples,
            "--verbose" | "-v" => verbose = true,
            "--stop-on-error" => stop_on_error = true,
            "--help" | "-h" => {
                print_help();
                return Ok(());
            }
            _ => {
                return Err(format!("unknown argument: {arg}").into());
            }
        }
    }

    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let cases = collect_cases(&root, scope)?;
    if cases.is_empty() {
        println!("No cases found.");
        return Ok(());
    }

    let mut passed = 0usize;
    let mut failed = 0usize;

    for case in cases {
        let start = Instant::now();
        let options = RenderOptions {
            verbose,
            fit_to_page: case.fit_to_page,
            ..RenderOptions::default()
        };
        match render_file_to_pdf(&case.input, case.css.as_ref(), &case.output, options) {
            Ok(pdf) => {
                passed += 1;
                println!(
                    "PASS [{group}] {input} -> {output} ({pages} page(s), {bytes} bytes, {elapsed_ms} ms)",
                    group = case.group,
                    input = rel(&case.input, &root),
                    output = rel(&case.output, &root),
                    pages = pdf.pages,
                    bytes = pdf.bytes.len(),
                    elapsed_ms = start.elapsed().as_millis(),
                );
            }
            Err(err) => {
                failed += 1;
                eprintln!(
                    "FAIL [{group}] {input} -> {output}: {err}",
                    group = case.group,
                    input = rel(&case.input, &root),
                    output = rel(&case.output, &root),
                );
                if stop_on_error {
                    break;
                }
            }
        }
    }

    let total = passed + failed;
    println!("\nSummary: total={total}, pass={passed}, fail={failed}");

    if failed > 0 {
        Err("one or more cases failed".into())
    } else {
        Ok(())
    }
}

fn collect_cases(root: &Path, scope: Scope) -> Result<Vec<Case>, Box<dyn Error>> {
    let mut cases = Vec::new();

    if matches!(scope, Scope::All | Scope::Tests) {
        let fixture_dir = root.join("tests/fixtures");
        let output_dir = root.join("tests/output/batch");
        let css = fixture_dir.join("test-styles.css");
        let css = css.exists().then_some(css);
        cases.extend(collect_dir_cases(
            "tests",
            &fixture_dir,
            css.as_deref(),
            &output_dir,
        )?);
    }

    if matches!(scope, Scope::All | Scope::Examples) {
        let example_dir = root.join("examples");
        let output_dir = root.join("examples/output/batch");
        let css = example_dir.join("styles.css");
        let css = css.exists().then_some(css);
        cases.extend(collect_dir_cases(
            "examples",
            &example_dir,
            css.as_deref(),
            &output_dir,
        )?);
    }

    cases.sort_by(|a, b| a.input.cmp(&b.input));
    Ok(cases)
}

fn collect_dir_cases(
    group: &'static str,
    input_dir: &Path,
    css_path: Option<&Path>,
    output_dir: &Path,
) -> Result<Vec<Case>, Box<dyn Error>> {
    fs::create_dir_all(output_dir)?;

    let mut inputs = fs::read_dir(input_dir)?
        .filter_map(|entry| entry.ok().map(|value| value.path()))
        .filter(|path| path.is_file())
        .filter(|path| {
            matches!(
                path.extension().and_then(|ext| ext.to_str()).map(|ext| ext.to_ascii_lowercase()),
                Some(ext) if ext == "html" || ext == "md"
            )
        })
        .collect::<Vec<_>>();

    inputs.sort();

    let mut cases = Vec::with_capacity(inputs.len());
    for input in inputs {
        let stem = input
            .file_stem()
            .and_then(|name| name.to_str())
            .ok_or("invalid case file name")?
            .to_string();
        let output = output_dir.join(format!("{stem}-rust.pdf"));
        cases.push(Case {
            group,
            input,
            css: css_path.map(Path::to_path_buf),
            output,
            fit_to_page: group == "tests" && stem == "test-27-fit-to-page-single",
        });
    }

    Ok(cases)
}

fn rel<'a>(path: &'a Path, root: &'a Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string()
}

fn print_help() {
    println!(
        "Render all fixtures/examples to PDF\n\nUsage:\n  cargo run --bin render-all-cases -- [--all|--tests|--examples] [--verbose] [--stop-on-error]\n\nDefaults:\n  --all"
    );
}
