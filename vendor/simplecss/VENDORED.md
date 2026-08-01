# Vendored `simplecss` (local fork)

This is a local, in-tree fork of the [`simplecss`](https://github.com/linebender/simplecss)
crate, referenced from the root `Cargo.toml` as a path dependency
(`simplecss = { path = "vendor/simplecss" }`).

- **Upstream:** https://github.com/linebender/simplecss
- **Vendored version:** `0.2.2` (the latest *published* release, Jan 2025 — verified the
  latest crates.io release; upstream `main` was only cosmetically ahead at vendoring time:
  a `@font-face` parse addition, a Rust/CI bump, and clippy fixes — none in our scope).
- **Vendored on:** 2026-08-01
- **License:** `Apache-2.0 OR MIT` (unchanged; see `LICENSE-APACHE` / `LICENSE-MIT`).

## Why fork

Skia's SVG DOM ignores `<style>` elements, so skia-canvas resolves CSS to inline styles
itself (`src/image.rs`). Upstream simplecss can't parse several selector constructions we
want to support for hand-authored SVGs (structural pseudo-classes, `:not()`, general
sibling `~`) — the gap is in its parser, not something we can bolt on via the
`simplecss::Element` adapter. Extending the parser requires editing the crate, hence the
fork. See `_cc/svg-selectors.md` (untracked design doc) for the full plan.

## Local changes vs. pristine 0.2.2

Kept **byte-identical to upstream except for the additions below**, so a future
`diff` against a fresh 0.2.2 checkout (or a newer upstream) stays legible. Each local
hunk is marked with a `// skia-canvas:` comment. Only cargo's registry-extraction
artifacts were dropped when vendoring (`Cargo.toml.orig`, `Cargo.lock`, `.cargo-ok`,
`.cargo_vcs_info.json`); the crate's own `src/`, `tests/`, and `examples/` are intact
(the standalone suite passes: `cargo test --manifest-path vendor/simplecss/Cargo.toml`).

_(none yet — this commit is the pristine import; modifications land in the following
change.)_
