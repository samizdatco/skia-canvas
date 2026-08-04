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

All local changes are confined to `src/selector.rs` (the matching semantics for the new
pseudo-classes live in the downstream `simplecss::Element` impl in skia-canvas's
`src/image.rs`, not here). Added tests in `tests/select.rs` and `tests/specificity.rs`.

- **Structural pseudo-classes.** New `PseudoClass` variants `LastChild`, `OnlyChild`,
  `FirstOfType`, `LastOfType`, `OnlyOfType`, and the functional `NthChild`/`NthLastChild`/
  `NthOfType`/`NthLastOfType` (carrying a new `Nth { a, b }` with `Nth::matches(index)` and
  a `parse_nth` for the `An+B`/`odd`/`even` micro-syntax).
- **`:not()`** as `Not(&str)` — stores the argument's source (re-parsed on match) so
  `PseudoClass` stays `Copy`; accepts a single (possibly complex) inner selector, rejects
  comma-lists; contributes its argument's specificity in `Selector::specificity()`.
- **General sibling combinator `~`** — new `Combinator::GeneralSibling` +
  `SelectorToken::SiblingCombinator`, wired through the tokenizer, parser, `matches_impl`,
  and `Display`.
- **Functional-pseudo tokenizing.** The tokenizer now consumes a `(...)` argument for any
  functional pseudo (not just `:lang`), emitting `SelectorToken::FunctionalPseudoClass`, so
  the argument can't leak into the stream and corrupt later tokens.
- **Graceful skip.** An unknown or malformed pseudo becomes `PseudoClass::Unsupported(name)`
  (never matches) instead of dropping the whole selector — so grouped siblings survive.
- **Declaration-list error recovery.** A malformed declaration is now skipped up to the next
  top-level `;` and parsing continues, instead of aborting the whole list at the first invalid
  token (upstream's `DeclarationTokenizer::next` did `jump_to_end()`; `consume_declarations` did
  `consume_until_block_end(); break`). A single unparseable declaration — e.g. a custom property
  (`--x: …`), a `var()` fallback with nested parens, or a `font: 16px/1.4` shorthand — no longer
  discards the valid declarations after it (matters for both inline `style="…"` re-parsing and
  `<style>` rule bodies). New `recover_declaration(stop_at_brace)` helper steps over strings and
  balanced `()`/`[]`/`{}` so a `;` inside a value or nested block doesn't cut recovery short.
  Leaves `jump_to_end` unused (kept, `#[allow(dead_code)]`). Added tests in
  `tests/declaration_tokenizer.rs` and `tests/stylesheet.rs`; `style_15`'s pre-existing
  `// TODO` outcome is now achieved.
- **Nested parens in values.** `consume_term`'s function handling now balances nested parens
  (and steps over quoted args) instead of skipping to the first `)`, so `calc((a) - (b))`,
  `var(--x, rgb(...))`, gradients with color-function args, and `url("a)b")` parse whole rather
  than truncating.
- **Custom-property declarations.** `consume_declaration` now accepts `--foo` names (which
  `consume_ident` rejects at the second dash), capturing `--` + name-chars directly. Custom
  properties are **not** interpreted — they're parsed and passed along like any other declaration
  so they stop tripping the parser. The value is still parsed as a normal declaration value
  (adequate now that nested parens parse); an exotic custom-property value can still fall to
  declaration-list recovery. Because `--x`/`--bg` are now valid, the recovery tests that used them
  as their "malformed" example were re-pointed at still-malformed triggers (missing colon,
  digit-leading name). Added tests in `tests/declaration_tokenizer.rs` / `tests/stylesheet.rs`.
- **`var()` substitution (root-only scoping).** `resolve_inline` now expands
  `var(--name[, fallback])` in emitted values and consumes `--*` declarations instead of passing
  them through. The custom-property scope is the **document root's** custom properties (walked via
  the `Element` trait's `parent_element`, so `:root`/`svg`/matching-rule `--*` — see the new
  `PseudoClass::Root`) overlaid with the element's own; there is **no** inheritance from
  intermediate ancestors and a `style="--x"` on the root element isn't reachable (deliberate
  low-complexity cut). Custom-property values are literal — no chained `var()` and thus no cycles;
  a `budget` caps expansion. `resolve_inline`'s winner computation was factored into a private
  `cascade` shared with the root walk; helpers `substitute_vars`/`split_top_comma` added. `:root`
  matches an element with no parent element. Added tests in `tests/select.rs`.
