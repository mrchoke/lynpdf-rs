# LynPDF RS Architecture Notes

Companion example repository for clone-and-try workflows:

- `https://github.com/mrchoke/lynpdf-rs-example`

```bash
git clone https://github.com/mrchoke/lynpdf-rs-example
cd lynpdf-rs-example
cargo run
```

This crate starts from the Thai text rendering report and keeps the pipeline split into stages that can be replaced independently:

1. HTML parsing: `html5ever` via `markup5ever_rcdom`.
2. CSS parsing: `lightningcss` is used as the standards parser gate; an MVP declaration parser currently extracts the supported subset.
3. Style/layout: a compact block layout engine ships first so examples can render now. `taffy` is kept behind the `layout-taffy` feature for the next flex/table implementation pass.
4. Thai text shaping: direct `rustybuzz` shaping with NFC normalization, cluster-aware line construction, and runtime-configured external fonts.
5. PDF output: a pure-Rust CID Type0 PDF emitter embeds full TrueType/OpenType fonts, writes a CID-to-GID map, and emits a ToUnicode CMap.

## Backend Decision

`skia-safe` remains a useful future backend for high-fidelity painting, but it is a native Skia binding and has a much heavier build story. The first Rust milestone therefore uses a low-level PDF emitter so Thai glyph ids, cluster mappings, font embedding, and deterministic object ordering are under our control.

`cosmic-text` is a strong candidate for the next text backend because it brings font fallback and text system ergonomics. The MVP keeps `rustybuzz` direct so the shaping output is transparent while we build golden tests.

## Current MVP Scope

Supported now:

- Basic HTML5 parsing and body extraction.
- Tag/class/id/descendant CSS selectors for common examples.
- `@page size` and `@page margin` subset.
- Block flow, margins, padding, borders, backgrounds.
- Thai shaping with runtime-configured Sarabun/Prompt/Kanit/Mitr/Chakra Petch fonts from local font directories or `@font-face`.
- Cluster-safe line wrapping fallback.
- PDF generation with embedded CID fonts.

Next steps:

- Add dictionary-based Thai word segmentation before cluster fallback.
- Promote `taffy` from feature dependency to the box layout backend for flex/table sizing.
- Build table row/cell fragmentation and repeated header support.
- Add PDF semantic tests for ToUnicode copy/paste round-trip.
- Add visual regression outputs for all `tests/fixtures` and `examples`.