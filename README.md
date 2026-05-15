# LynPDF RS v0.1.2

Pure Rust HTML/CSS to PDF renderer focused on Thai documents.

Production policy: this repository does not need to publish bundled font binaries. Runtime fonts are configured externally.

The first milestone is intentionally small but end-to-end: parse HTML/CSS, resolve a practical style subset, shape Thai text with `rustybuzz`, embed Thai fonts, and emit a PDF with CID Type0 fonts and ToUnicode data.

## Companion Example Repository

Try the full Thai checkout -> receipt PDF flow in:

- `https://github.com/mrchoke/lynpdf-rs-example`

```sh
git clone https://github.com/mrchoke/lynpdf-rs-example
cd lynpdf-rs-example
cargo run
```

## Try It

```toml
[dependencies]
lynpdf-rs = "0.1.2"
```

```sh
./scripts/download-fonts.sh --dest ./fonts

cargo run -- tests/fixtures/test-01-thai-typography.html \
  -c tests/fixtures/test-styles.css \
  -o tests/output/test-01-thai-typography-rust.pdf \
  --font-dir ./fonts \
  --font-map "MyBrand=./fonts/MyBrand-Regular.ttf" \
  --font-map-bold "MyBrand=./fonts/MyBrand-Bold.otf" \
  --font-variation "wght=450" \
  --verbose
```

```sh
cargo run --example render_fixture
```

```sh
cargo run --bin render-all-cases -- --all --verbose
```

```sh
cargo run --example api-certificate-single-page
```

```sh
cargo run --example generate-api-guide-pdf
```

```sh
cargo test
```

Batch output folders:

- `tests/output/batch`
- `examples/output/batch`

## Current Scope

- HTML5 parsing through `html5ever`.
- CSS validation through `lightningcss` plus an MVP declaration extractor.
- `@font-face` support for local `url(...)` sources, including multiple `src` candidates.
- Basic block layout with margins, padding, borders, backgrounds, page size, and page margin.
- Thai shaping through `rustybuzz` with NFC normalization.
- Font-family fallback stack resolution with coverage-aware font selection.
- Cluster-safe line wrapping fallback.
- Embedded fonts from CSS `@font-face`, user custom fonts, and configured runtime font directories.
- Pure Rust PDF output with full embedded fonts, CID-to-GID maps, and ToUnicode CMaps.

## Font Priority

LynPDF RS resolves fonts in this order:

1. User custom fonts from `RenderOptions.user_font_mappings` and `RenderOptions.user_font_dirs` (highest priority)
2. CSS `@font-face` registrations
3. `LYNPDF_FONT_DIRS` environment variable
4. Local `fonts/` directory

If no usable fonts are found, LynPDF returns a `FontNotFound` error with setup guidance.

Supported custom font file formats:

- TTF
- OTF

## Production Font Setup

- Full guide: [docs/font-setup.md](docs/font-setup.md)
- Linux/macOS script: `./scripts/download-fonts.sh --dest ./fonts` (includes TLWG OTF)
- Windows PowerShell: `./scripts/download-fonts.ps1 -Dest ./fonts` (includes TLWG OTF)
- Service runtime: set `LYNPDF_FONT_DIRS` or pass explicit `--font-dir`

## Custom Font API

```rust
use lynpdf_rs::{RenderFontStyle, RenderOptions, UserFontMapping};

let options = RenderOptions::default()
  .with_user_font_dir("./my-fonts")
  .with_user_font_mapping(
    UserFontMapping::new("MyThai", "./brand/MyThai-Regular.ttf")
      .with_weight(400)
      .with_style(RenderFontStyle::Normal),
  )
  .with_user_font_mapping(
    UserFontMapping::new("MyThai", "./brand/MyThai-Bold.ttf")
      .with_weight(700)
      .with_style(RenderFontStyle::Normal),
  )
  .with_kerning(true)
  .with_ligatures(true)
  .with_font_variation("wght=500");
```

## Phase B Shaping Controls

- `RenderOptions.enable_kerning` and CLI `--no-kerning`
- `RenderOptions.enable_ligatures` and CLI `--no-ligatures`
- `RenderOptions.font_variations` and CLI `--font-variation AXIS=VALUE`

Current Phase B behavior:

- Cluster-level fallback is active in the shaping path: text is segmented at grapheme boundaries and fonts can switch at cluster boundaries when coverage is missing.
- Thai dictionary-based segmentation is applied first to mark preferred Thai word break opportunities before cluster fallback.
- Kerning/ligature toggles are forwarded to OpenType shaping features (`kern`, `liga`, `clig`).
- Variable font axes are applied through rustybuzz variations for each shaped run.

## Syntax Highlighting (HTML + Markdown)

- Enabled by default in v0.1.2 for `<pre><code>` blocks.
- HTML and Markdown share the same preprocessing path (`render_html_to_pdf` calls `apply_syntax_highlighting_to_html` first).
- API: `RenderOptions.enable_syntax_highlighting` and `RenderOptions.syntax_highlight_theme`.
- Builder helpers: `with_syntax_highlighting(...)` and `with_syntax_highlight_theme(...)`.
- CLI: `--no-syntax-highlight` and `--syntax-theme NAME`.
- If `<pre>`/`<code>` has no `style`, LynPDF injects safe default block styles (padding, border, wrap, monospace sizing); existing inline styles are preserved.

## Single-Page Certificate API

v0.1.2 includes certificate template APIs for full-page A4 portrait/landscape output:

- `CertificateTemplateOptions`
- `CertificateOrientation::{Portrait, Landscape}`
- `build_fullpage_certificate_html(...)`
- `render_fullpage_certificate_pdf(...)`
- `render_fullpage_certificate_pdf_to_file(...)`

See example files:

- `examples/demo-certificate-single-page-landscape.html`
- `examples/demo-certificate-single-page-portrait.html`
- `examples/api-certificate-single-page.rs`

See [docs/rust-architecture.md](docs/rust-architecture.md) for backend decisions and the next implementation steps.

For detailed fixture/example coverage and phased implementation planning, see [docs/feature-coverage-and-roadmap.md](docs/feature-coverage-and-roadmap.md).

Primary API guide: [docs/api-guide-v0.1.2.md](docs/api-guide-v0.1.2.md).

Public GitHub Pages guide: https://mrchoke.github.io/lynpdf-rs/