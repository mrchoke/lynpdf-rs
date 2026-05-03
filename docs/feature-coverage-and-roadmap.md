# LynPDF RS: Feature Coverage And Development Roadmap

This document maps what the current test corpus is exercising and defines a phased plan to support all functions, with font correctness as the top priority.

## 1) Test Fixtures Coverage (tests/fixtures)

### Core language and typography
- `test-01-thai-typography.html`: Thai mark stacking, Thai line breaking, text alignment (left/center/right/justify), mixed Thai+Latin+emoji, font fallback.
- `test-09-kerning-ligature.html`: kerning, ligatures, Thai glyph positioning, letter spacing stress.
- `test-14-text-justify.html`: justify behavior in Thai, English, and mixed text.
- `test-15-pali-sanskrit.html`: advanced Thai script combinations (pali/sanskrit, pindu/nikhahit, stacked marks) across many families.
- `test-28-syntax-highlight.html`: HTML code-block syntax highlighting coverage for Rust, TypeScript, and SQL.

### Paged media and fragmentation
- `test-02-paged-media.html`: page size, margins, forced page breaks, break-inside avoid, orphans/widows scenario.
- `test-16-page-margin.html`: @page margin parsing and multi-page flow.
- `test-17-page-orientation.html`: @page size orientation (landscape).
- `test-pagebreak-after-table.html`: table overflow + section continuation after forced page break.

### Tables
- `test-03-tables.html`: long tables, repeated thead/tfoot, colspan/rowspan, border-collapse variants.
- `test-08-thead-repeat.html`: repeated table headers across pages in long datasets.
- `test-20-border-style-parity.html`: mixed border styles in collapse/separate mode (conflict precedence and stroke appearance).

### Graphics, media, and visual styling
- `test-04-box-model.html`: flexbox, positioning, overflow clipping, box model composition.
- `test-05-graphics-colors.html`: rgb/rgba transparency, SVG inline, raster image, border styles/radius, color emoji.
- `test-07-certificate.html`: full-page composition, inline SVG logos/signature/QR, multi-font poster-like layout.
- `test-21-svg-transform-chain.html`: nested SVG transform stack regression with gradient fill and per-shape opacity.
- `test-22-svg-gradient-stroke.html`: SVG linear gradient interpolation for stroke/fill paints with opacity controls.

### Fonts and metadata
- `test-10-otf-font.html`: OTF support, kerning/ligatures, OTF vs TTF comparison.
- `test-11-variable-font.html`: variable font usage, weight/style behavior, mixed type comparison.
- `test-13-tlwg-fonts.html`: broad TLWG OTF family coverage (sans/serif/mono/handwriting).
- `test-06-links-metadata.html`: internal/external links and document metadata scenarios.
- `test-12-metadata.html`: HTML metadata to PDF metadata mapping expectations.

### Markdown inputs
- `test-18-markdown.md`: baseline markdown features (headings, lists, tables, code, footnotes, thai text).
- `test-19-enhanced-markdown.md`: syntax-highlighted code blocks, containers/alerts/cards, richer markdown extensions.

## 2) Examples Coverage (examples)

- `demo-thai-fonts.html`: Thai font family showcase across variants and sizes.
- `demo-font-showcase.html`: TTF/OTF/variable comparison for shaping quality.
- `demo-typography.html` and `typography.html`: multilingual text, emojis, mixed scripts, list structures.
- `demo-multipage.html` and `multipage.html`: natural + forced pagination flow.
- `demo-tables.html`: table-heavy document rendering.
- `demo-images.html`: image and media rendering.
- `demo-invoice.html` and `invoice.html`: business layout, totals, table styling.
- `demo-certificate.html`: highly styled certificate output.
- `book-lynpdf-guide.html`: long-form, multi-section, table of contents style content.
- `demo-single-page.html` and `layout.html`: basic baseline and layout behavior.

## 3) Current Engine Snapshot (Rust)

Current code is a focused MVP:
- CSS parser + style subset with `@font-face` url parsing in `src/style.rs`.
- Font registration and URL path resolution in `src/fonts.rs`.
- Thai shaping and cluster-safe wrapping in `src/text.rs`.
- Block-flow layout and glyph paint ops in `src/layout.rs`.
- PDF emission with embedded CID fonts + ToUnicode in `src/pdf.rs`.

This is enough for early typography validation, but not yet full parity with the fixture matrix above.

## 4) Font Policy (Required)

Target font source priority:
1. User custom fonts (configured by API or CSS `@font-face`) — highest priority.
2. Runtime font directories from environment (`LYNPDF_FONT_DIRS`) and project-local `fonts/`.
3. Optional fallback chain (if enabled) for missing glyph coverage.

Repository/publish policy:
- Do not publish bundled font binaries with the crate package.
- Bootstrap runtime fonts via scripts in `scripts/` and deployment documentation.

Required format support:
- TTF
- OTF

## 5) Development Plan To Reach Full Support

### Phase A: Font Reliability First (highest priority)

Current status: In progress (core implementation completed, full runtime corpus verification pending).

1. Expand runtime font discovery
- Recursively load `fonts/**/*.ttf` and `fonts/**/*.otf` plus configured external font directories.
- Keep discovery path deterministic for CI/service environments.

2. Strengthen `@font-face` handling
- Parse multiple `src` candidates and pick first supported local TTF/OTF source.
- Keep diagnostics for unsupported sources (`http`, `data`, unsupported format).

3. Implement explicit user font registration API
- Add `RenderOptions`/builder support for custom font dirs and direct file mappings.
- Ensure user-provided mappings override bundled mappings for same family+style+weight.

4. Improve family fallback behavior
- Parse full `font-family` stack, not only first token.
- Resolve by variant, then by coverage fallback.

Phase A progress snapshot
- Done: recursive bundled font discovery for TTF/OTF.
- Done: multi-source `@font-face` parsing with usable-source selection and diagnostics.
- Done: user font dirs/mappings API with deterministic override priority.
- Done: family stack parsing and coverage-aware font resolution in text shaping path.
- Done: full corpus runtime verification (`render-all-cases --all --verbose`) completed successfully (37/37 pass, including newly added SVG fixtures test-21 and test-22).

Acceptance criteria (Phase A)
- All fonts referenced by `examples/styles.css` and fixture CSS are loaded and embedded.
- TLWG family names in `test-13-tlwg-fonts.html` resolve without manual patching.
- User-specified fonts override bundled fonts deterministically.

### Phase B: Text Engine Parity

Current status: Started (cluster fallback + shaping toggles baseline implemented).

1. Glyph coverage fallback per run/cluster
- If selected font lacks glyphs, fallback at cluster boundary while preserving Thai combining correctness.

2. Advanced shaping toggles
- Enable kerning/ligature control and variable axis handling where available.

3. Thai break quality upgrades
- Add dictionary-based Thai segmentation before cluster fallback.

Phase B progress snapshot
- Done: cluster-level fallback routing in shaping path (font can switch at grapheme cluster boundaries).
- Done: kerning/ligature toggles exposed in `RenderOptions` and CLI.
- Done: variable font axis input (`AXIS=VALUE`) passed into shaping runs.
- Done: dictionary-based Thai segmentation is applied before cluster fallback to improve Thai line-break opportunities.
- Done: targeted Phase B regression (`test-01`, `test-09`, `test-10`, `test-11`, `test-14`, `test-15`) renders successfully (6/6 pass, artifact generation complete).
- Pending: visual QA comparison for Thai line-break quality and typography parity against reference expectations.

Acceptance criteria (Phase B)
- `test-01`, `test-09`, `test-10`, `test-11`, `test-14`, `test-15` produce stable Thai and Latin shaping.

### Phase C: Layout And Pagination Parity

Current status: Started (table width sizing upgraded with browser-like intrinsic constraints).

1. Introduce full layout backend
- Integrate `taffy` for flex/grid-like behavior and block layout consistency.

2. Table model and fragmentation
- Proper table layout, long-table pagination, repeated thead/tfoot, colspan/rowspan.

3. Paged media
- Solid @page margin/size/orientation handling and break-inside semantics.

Phase C progress snapshot
- Done: table width resolution now uses intrinsic min/preferred sizing with colspan distribution instead of equal-width fallback.
- Done: percentage widths for block/table styles (for example `width: 100%`) are resolved against container width.
- Done: rowspan-aware placement is active in table row layout (occupied-column tracking + row-span height reconciliation).
- Done: `border-collapse: collapse` now renders with single-pass border lines (avoids doubled inner borders from per-cell rectangle strokes).
- Done: collapsed-border conflict precedence is segment-aware and browser-oriented (`hidden` suppression, width/style ranking, source priority, top/left tie-break rules).
- Done: `border-style` parsing now affects computed border visibility (`none`/`hidden` no longer emit strokes).
- Done: style-specific border stroke appearance is applied in table rendering (solid/dashed/dotted/double and tonal variants for ridge/groove/inset/outset).
- Done: ridge/groove/inset/outset bevel direction is side-aware (`top/right/bottom/left`) for closer browser-like depth cues.
- Done: `border-spacing` parsing is wired for separate border model behavior.
- Done: expanded visual QA fixture for mixed border styles (`test-20-border-style-parity.html`) is in place for Phase C regression runs.
- Done: automated visual diff runner + browser baseline workflow is available for `test-20`.
- Done: visual diff baseline report now includes AA-compensated metrics and edge-phase mismatch indicators to reduce anti-aliasing noise bias.
- Done: dashed/dotted line phase is side-aware and centered to improve parity against browser border rasterization.
- Done: targeted table regressions render successfully for `test-03-tables.html`, `test-08-thead-repeat.html`, `test-pagebreak-after-table.html`, and `test-20-border-style-parity.html`.
- Pending: further tune corner joins and subpixel stroke compositing parity for specific browser engines.

Acceptance criteria (Phase C)
- `test-02`, `test-03`, `test-08`, `test-16`, `test-17`, `test-pagebreak-after-table`, `test-20-border-style-parity` pass visual review.

### Phase D: Rich Document Features

Current status: In progress (core media + advanced SVG rendering effects implemented; broader visual QA pass pending).

1. Graphics/media
- SVG/raster image rendering and color/opacity parity.

2. Links and metadata
- Internal/external links, PDF metadata mapping.

3. Markdown pipeline
- Markdown parsing to HTML with extension parity for `test-18` and `test-19`.

Phase D progress snapshot
- Done: HTML `<title>` and `<meta name=...>` are now extracted and mapped into PDF Info dictionary fields (`/Title`, `/Author`, `/Subject`, `/Keywords`, `/Creator`, `/Producer`).
- Done: clickable PDF link annotations are emitted for `<a href="...">` in text flow, including external URI actions and internal `#anchor` GoTo destinations.
- Done: element `id` anchors are captured during layout and resolved to page-level PDF destinations.
- Done: Markdown input (`.md`) now runs through a first-class conversion path before rendering, with GFM-style tables/task-lists/footnotes/strikethrough enabled.
- Done: enhanced markdown containers/alerts baseline (`:::...` and `> [!NOTE]` patterns) now maps to styled HTML container blocks in the Rust pipeline.
- Done: core renderer now emits raster image paint ops (`<img>` to PDF `/Image` XObject with alpha mask support) and inline SVG vector paint ops (`rect/circle/line/polyline/polygon/path/text`) used in `test-05` and `test-07`.
- Done: SVG transform stacks are now propagated through nested groups/elements and applied to geometry/path/text emission.
- Done: SVG linear gradients now map to native PDF axial shading patterns instead of flat stop averaging.
- Done: per-shape SVG opacity attributes (`opacity`, `fill-opacity`, `stroke-opacity`) now emit PDF ExtGState compositing controls.
- Done: focused SVG regression fixtures were added for transform chains and gradient stroke parity (`test-21-svg-transform-chain.html`, `test-22-svg-gradient-stroke.html`) with integration assertions for `/Pattern`, shading, and `/ExtGState` emission.
- Done: full fixture/example corpus rerender completed after SVG effects work (`render-all-cases --all --verbose`: total=37, pass=37, fail=0).

Acceptance criteria (Phase D)
- `test-04`, `test-05`, `test-06`, `test-07`, `test-12`, `test-18`, `test-19`, `test-21`, `test-22` pass expected behavior checks.

## 6) Regression Execution

Use batch runner to produce PDFs for every fixture and example, then compare output sets between commits:
- `cargo run --bin render-all-cases -- --all --verbose`

Latest Phase D SVG effects validation snapshot and parity-gap notes are tracked in:
- `docs/phase-d-svg-effects-validation.md`

For Phase C table border parity on `test-20`, run automated visual diff against browser baseline:
- `cargo run --bin visual-diff-test20 -- --threshold 0.18`
- First-time baseline or deliberate baseline refresh: `cargo run --bin visual-diff-test20 -- --update-baseline`
- If no local headless browser (Chromium/Firefox) is available, supply an external browser screenshot: `cargo run --bin visual-diff-test20 -- --browser-reference-png <path-to-browser-png>`
- Optional AA sensitivity tuning: `cargo run --bin visual-diff-test20 -- --aa-sigma 0.85 --aa-weight 0.35`

Store outputs under:
- `tests/output/batch`
- `examples/output/batch`
- `tests/output/visual-diff/test-20`
