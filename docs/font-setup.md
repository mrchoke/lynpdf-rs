# Font Setup (No Bundled Fonts)

LynPDF RS is prepared for production without shipping font binaries in the repository or crate package.

Companion example repository for end-to-end Thai invoice testing:

- `https://github.com/mrchoke/lynpdf-rs-example`

```bash
git clone https://github.com/mrchoke/lynpdf-rs-example
cd lynpdf-rs-example
cargo run
```

## Policy

- Do not commit runtime font files to this repository.
- Download fonts with script during setup/CI/deploy.
- Configure runtime paths with CLI (`--font-dir`) or env (`LYNPDF_FONT_DIRS`).

## 1) Quick Start (Project Local)

From repository root:

```bash
./scripts/download-fonts.sh --dest ./fonts
```

This downloads the Google/emoji set used by fixtures and also the TLWG OTF bundle into `./fonts/tlwg/otf`.

Windows PowerShell:

```powershell
./scripts/download-fonts.ps1 -Dest ./fonts
```

Then run LynPDF:

```bash
cargo run -- examples/demo-single-page.html -c examples/styles.css -o examples/output/demo-single-page.pdf --font-dir ./fonts
```

## 2) Runtime Font Discovery Order

LynPDF tries these font sources in order:

1. `--font-dir` and `--font-map` options from CLI/API
2. CSS `@font-face` local `url(...)`
3. `LYNPDF_FONT_DIRS` environment variable (multi-path)
4. Local `./fonts` (project relative)

If no usable fonts are found, render returns `FontNotFound` with setup hints.

## 3) Linux/macOS/Windows Install Notes

### Linux (Debian/Ubuntu)

```bash
sudo apt-get update
sudo apt-get install -y fontconfig
./scripts/download-fonts.sh --dest /usr/local/share/fonts/lynpdf
sudo fc-cache -f -v
```

Optional Thai TLWG package:

```bash
sudo apt-get install -y fonts-thai-tlwg
```

### macOS

```bash
./scripts/download-fonts.sh --dest "$HOME/Library/Fonts/lynpdf"
```

### Windows

Use PowerShell script to download fonts to a folder such as `C:\lynpdf\fonts`, then pass that folder with `--font-dir` or set `LYNPDF_FONT_DIRS`.

## 4) Docker Example

```dockerfile
FROM rust:1.86-bookworm

RUN apt-get update \
    && apt-get install -y --no-install-recommends curl ca-certificates fontconfig \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY . .

RUN chmod +x ./scripts/download-fonts.sh \
    && ./scripts/download-fonts.sh --dest /usr/local/share/fonts/lynpdf \
    && fc-cache -f -v

ENV LYNPDF_FONT_DIRS=/usr/local/share/fonts/lynpdf
```

## 5) Service Deployment Pattern

### Environment variable

```bash
export LYNPDF_FONT_DIRS="/opt/lynpdf/fonts:/usr/local/share/fonts/lynpdf"
```

### CLI for explicit path

```bash
lynpdf-rs input.html -o output.pdf --font-dir /opt/lynpdf/fonts
```

### Rust API

```rust
use lynpdf_rs::RenderOptions;

let options = RenderOptions::default()
    .with_user_font_dir("/opt/lynpdf/fonts");
```

## 6) Recommended CI Bootstrap

```bash
./scripts/download-fonts.sh --dest ./fonts
cargo test --lib --tests
```

Keep `fonts/` out of source control and artifact bundles.
