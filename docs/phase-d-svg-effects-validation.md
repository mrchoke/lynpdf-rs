# Phase D SVG Effects Validation Report

## Scope

This report validates the Phase D SVG effects parity work for:
- transform stacks
- native linear-gradient interpolation
- per-shape opacity compositing

## Commands Executed

- `cargo test --lib --tests`
- `cargo run --bin render-all-cases -- --all --verbose`
- PDF signature checks on:
  - `tests/output/batch/test-21-svg-transform-chain-rust.pdf`
  - `tests/output/batch/test-22-svg-gradient-stroke-rust.pdf`

## Corpus Result

- Total: 37
- Pass: 37
- Fail: 0

This includes newly added SVG fixtures:
- `test-21-svg-transform-chain.html`
- `test-22-svg-gradient-stroke.html`

## New Regression Coverage

1. Fixture: `test-21-svg-transform-chain.html`
- Exercises nested group transforms (`translate`, `rotate`, `scale`, `skewX`).
- Uses linear-gradient fill with `gradientTransform`.
- Uses `fill-opacity`, `stroke-opacity`, and shape-level `opacity`.

2. Fixture: `test-22-svg-gradient-stroke.html`
- Exercises linear-gradient interpolation for both fill and stroke paints.
- Validates multiple gradient stops with non-trivial offsets and stop opacity.
- Validates opacity compositing for fill and stroke independently.

3. Tests
- Integration tests added in `tests/svg_effects_regression.rs`.
- Layout unit tests added in `src/layout.rs` for transform parsing/order and inherited opacity composition.

## PDF Signature Validation

Observed signatures in generated PDFs confirm expected Phase D behavior:
- Pattern resources present: `/Pattern << ... >>`
- Axial shading objects present: `/ShadingType 2`
- Multi-stop interpolation function present: `/FunctionType 3`
- Pattern fill and stroke operators present: `/Pattern cs`, `/Pattern CS`, `scn`, `SCN`
- Opacity graphics states present: `/ExtGState << ... >>`, `gs`

## Parity Gap Summary

No functional regressions were observed in corpus execution for the targeted Phase D features.

Residual parity risks (outside this report's automated checks):
- Browser pixel-diff parity is not yet automated for Phase D SVG fixtures (current visual-diff automation is specialized for test-20 border parity).
- The new regression fixtures focus on linear gradients and transform stacks; additional SVG effects (for example radial gradients, masks, clip-path filters) remain out of current Phase D fixture scope.
