// Copyright 2016 the SimpleCSS Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

/*!
A simple [CSS 2.1](https://www.w3.org/TR/CSS21/) parser and selector.

This is not a browser-grade CSS parser. If you need one,
use [cssparser](https://crates.io/crates/cssparser) +
[selectors](https://crates.io/crates/selectors).

Since it's very simple we will start with limitations:

## Limitations

- [At-rules](https://www.w3.org/TR/CSS21/syndata.html#at-rules) are not supported.
  They will be skipped during parsing.
- Property values are not parsed.
  In CSS like `* { width: 5px }` you will get a `width` property with a `5px` value as a string.
- CDO/CDC comments are not supported.
- Parser is case sensitive. All keywords must be lowercase.
- Unicode escape, like `\26`, is not supported.

## Features

- Selector matching support.
- The rules are sorted by specificity.
- `!important` parsing support.
- Has a high-level parsers and low-level, zero-allocation tokenizers.
- No unsafe.
*/

// LINEBENDER LINT SET - lib.rs - v2
// See https://linebender.org/wiki/canonical-lints/
// These lints aren't included in Cargo.toml because they
// shouldn't apply to examples and tests
#![warn(unused_crate_dependencies)]
#![warn(clippy::print_stdout, clippy::print_stderr)]
// Targeting e.g. 32-bit means structs containing usize can give false positives for 64-bit.
#![cfg_attr(target_pointer_width = "64", warn(clippy::trivially_copy_pass_by_ref))]
// END LINEBENDER LINT SET
#![cfg_attr(docsrs, feature(doc_auto_cfg))]
#![cfg_attr(all(not(feature = "std"), not(test)), no_std)]
// The following lints are part of the Linebender standard set,
// but resolving them has been deferred for now.
// Feel free to send a PR that solves one or more of these.
#![allow(
    missing_debug_implementations,
    unreachable_pub,
    clippy::use_self,
    clippy::missing_assert_message,
    clippy::missing_panics_doc,
    clippy::exhaustive_enums,
    clippy::unseparated_literal_suffix
)]
#![cfg_attr(test, allow(unused_crate_dependencies))] // Some dev dependencies are only used in tests

extern crate alloc;

// skia-canvas: BTreeMap/String/format are used by StyleSheet::resolve_inline (below)
use alloc::{collections::BTreeMap, format, string::String, vec::Vec};
use core::fmt;

use log::warn;

mod selector;
mod stream;

pub use selector::*;
use stream::Stream;

/// A list of possible errors.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Error {
    /// The steam ended earlier than we expected.
    ///
    /// Should only appear on invalid input data.
    UnexpectedEndOfStream,

    /// An invalid ident.
    InvalidIdent(TextPos),

    /// An unclosed comment.
    InvalidComment(TextPos),

    /// An invalid declaration value.
    InvalidValue(TextPos),

    /// An invalid byte.
    #[allow(missing_docs)]
    InvalidByte {
        expected: u8,
        actual: u8,
        pos: TextPos,
    },

    /// A missing selector.
    SelectorMissing,

    /// An unexpected selector.
    UnexpectedSelector,

    /// An unexpected combinator.
    UnexpectedCombinator,

    /// An invalid or unsupported attribute selector.
    InvalidAttributeSelector,

    /// An invalid language pseudo-class.
    InvalidLanguagePseudoClass,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            Error::UnexpectedEndOfStream => {
                write!(f, "unexpected end of stream")
            }
            Error::InvalidIdent(pos) => {
                write!(f, "invalid ident at {}", pos)
            }
            Error::InvalidComment(pos) => {
                write!(f, "invalid comment at {}", pos)
            }
            Error::InvalidValue(pos) => {
                write!(f, "invalid value at {}", pos)
            }
            Error::InvalidByte {
                expected,
                actual,
                pos,
            } => {
                write!(
                    f,
                    "expected '{}' not '{}' at {}",
                    expected as char, actual as char, pos
                )
            }
            Error::SelectorMissing => {
                write!(f, "selector missing")
            }
            Error::UnexpectedSelector => {
                write!(f, "unexpected selector")
            }
            Error::UnexpectedCombinator => {
                write!(f, "unexpected combinator")
            }
            Error::InvalidAttributeSelector => {
                write!(f, "invalid or unsupported attribute selector")
            }
            Error::InvalidLanguagePseudoClass => {
                write!(f, "invalid language pseudo-class")
            }
        }
    }
}

#[cfg(feature = "std")]
impl std::error::Error for Error {}

/// A position in text.
///
/// Position indicates a row/line and a column in the original text. Starting from 1:1.
#[derive(Clone, Copy, PartialEq, Debug)]
#[allow(missing_docs)]
pub struct TextPos {
    pub row: u32,
    pub col: u32,
}

impl TextPos {
    /// Constructs a new `TextPos`.
    ///
    /// Should not be invoked manually, but rather via `Stream::gen_text_pos`.
    pub fn new(row: u32, col: u32) -> TextPos {
        TextPos { row, col }
    }
}

impl fmt::Display for TextPos {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.row, self.col)
    }
}

/// A declaration.
#[derive(Clone, Copy, PartialEq, Debug)]
#[allow(missing_docs)]
pub struct Declaration<'a> {
    pub name: &'a str,
    pub value: &'a str,
    pub important: bool,
}

/// A rule.
#[derive(Clone, Debug)]
pub struct Rule<'a> {
    /// A rule selector.
    pub selector: Selector<'a>,
    /// A rule declarations.
    pub declarations: Vec<Declaration<'a>>,
}

/// A style sheet.
#[derive(Clone, Debug)]
pub struct StyleSheet<'a> {
    /// A list of rules.
    pub rules: Vec<Rule<'a>>,
}

impl<'a> StyleSheet<'a> {
    /// Creates an empty style sheet.
    pub fn new() -> Self {
        StyleSheet { rules: Vec::new() }
    }

    /// Parses a style sheet from text.
    ///
    /// At-rules are not supported and will be skipped.
    ///
    /// # Errors
    ///
    /// Doesn't produce any errors. In worst case scenario will return an empty stylesheet.
    ///
    /// All warnings will be logged.
    pub fn parse(text: &'a str) -> Self {
        let mut sheet = StyleSheet::new();
        sheet.parse_more(text);
        sheet
    }

    /// Parses a style sheet from a text to the current style sheet.
    pub fn parse_more(&mut self, text: &'a str) {
        let mut s = Stream::from(text);

        if s.skip_spaces_and_comments().is_err() {
            return;
        }

        while !s.at_end() {
            if s.skip_spaces_and_comments().is_err() {
                break;
            }

            let _ = consume_statement(&mut s, &mut self.rules);
        }

        if !s.at_end() {
            warn!("{} bytes were left.", s.slice_tail().len());
        }

        // Remove empty rules.
        self.rules.retain(|rule| !rule.declarations.is_empty());

        // Sort the rules by specificity.
        self.rules
            .sort_by_cached_key(|rule| rule.selector.specificity());
    }
}

// skia-canvas: cascade key for one declaration; the higher tuple wins. Origin/importance
// simplified for a static render (no cascade layers / animations / transitions): `!important`
// beats normal regardless of specificity; within a tier an element's own inline style beats
// selector rules; then specificity; then source order (later wins).
type CascadeKey = (bool, bool, [u8; 3], usize);

fn consider<'a>(
    winners: &mut BTreeMap<&'a str, (CascadeKey, &'a str)>,
    name: &'a str,
    value: &'a str,
    key: CascadeKey,
) {
    winners
        .entry(name)
        .and_modify(|w| {
            if key > w.0 {
                *w = (key, value);
            }
        })
        .or_insert((key, value));
}

// skia-canvas: cascade resolution — see resolve_inline.
impl StyleSheet<'_> {
    /// Resolve the cascade for `element` into a keyword-stripped `prop:value;` string, folding
    /// the element's existing inline `style=` value (`inline`, or `""`) in as the highest author
    /// tier. Returns an empty string when nothing applies; otherwise one declaration per property,
    /// sorted by property name (deterministic).
    ///
    /// This resolves specificity, source order, and `!important` itself — rather than relying on a
    /// downstream last-declaration-wins — and strips `!important` from the emitted values. It's for
    /// callers that splice the result into a renderer which ignores `!important` (e.g. skia-canvas's
    /// SVG `<style>` support, where Skia discards any declaration whose value carries the keyword).
    pub fn resolve_inline<E: Element>(&self, element: &E, inline: &str) -> String {
        let winners = self.cascade(element, inline);

        // skia-canvas: assemble the custom-property scope for var() resolution — the document
        // root's custom properties, overridden by the element's own. Root scope is walked via the
        // Element trait (parent_element), so only the root's *rule*-defined `--*` are visible; a
        // `style="--x"` inline on the root element isn't reachable here. No inheritance from
        // intermediate ancestors (that's the deliberate low-complexity cut).
        let mut vars: BTreeMap<&str, &str> = BTreeMap::new();
        let mut ancestor = element.parent_element();
        let mut root = None;
        while let Some(node) = ancestor {
            ancestor = node.parent_element();
            root = Some(node);
        }
        if let Some(root) = root {
            for (name, (_, value)) in self.cascade(&root, "") {
                if name.starts_with("--") {
                    vars.insert(name, value);
                }
            }
        }
        for (name, (_, value)) in &winners {
            if name.starts_with("--") {
                vars.insert(name, value); // element's own custom props win over the root's
            }
        }

        // skia-canvas: emit non-custom declarations, substituting var() from `vars`; `--*` are
        // consumed (resolved away), not emitted.
        let mut budget = 64u32; // runaway guard for var() expansion
        winners
            .iter()
            .filter(|(name, _)| !name.starts_with("--"))
            .map(|(name, (_, value))| {
                if value.contains("var(") {
                    format!("{}:{};", name, substitute_vars(value, &vars, &mut budget))
                } else {
                    format!("{}:{};", name, value)
                }
            })
            .collect()
    }

    // skia-canvas: compute the winning declaration per property for `element` — matched rules by
    // specificity/source-order, then the element's own inline style as the top author tier. Shared
    // by resolve_inline (for the element and, via the root walk, for the document root).
    fn cascade<'m, E: Element>(
        &'m self,
        element: &E,
        inline: &'m str,
    ) -> BTreeMap<&'m str, (CascadeKey, &'m str)> {
        let mut winners: BTreeMap<&str, (CascadeKey, &str)> = BTreeMap::new();
        let mut order = 0usize; // strict source order → last of equal priority wins

        for rule in self.rules.iter().filter(|r| r.selector.matches(element)) {
            let spec = rule.selector.specificity();
            for d in &rule.declarations {
                consider(&mut winners, d.name, d.value, (d.important, false, spec, order));
                order += 1;
            }
        }

        // the element's own inline style outranks any selector of equal importance
        for d in DeclarationTokenizer::from(inline) {
            consider(&mut winners, d.name, d.value, (d.important, true, [u8::MAX; 3], order));
            order += 1;
        }

        winners
    }
}

// skia-canvas: expand `var(--name)` / `var(--name, fallback)` in `value` using `vars`. A missing
// name with a fallback expands the fallback (which may itself contain var()); missing with no
// fallback is left verbatim so the renderer drops just that declaration. Map values are literal —
// never re-expanded — so custom-property chains don't resolve but also can't form cycles. `budget`
// caps total expansions as a runaway guard.
fn substitute_vars(value: &str, vars: &BTreeMap<&str, &str>, budget: &mut u32) -> String {
    let mut out = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(i) = rest.find("var(") {
        out.push_str(&rest[..i]);
        let args_and_tail = &rest[i + 4..]; // past "var("

        // find the matching ')' for this var(, balancing nested parens
        let mut depth = 1usize;
        let mut close = None;
        for (j, b) in args_and_tail.bytes().enumerate() {
            match b {
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        close = Some(j);
                        break;
                    }
                }
                _ => {}
            }
        }
        let Some(close) = close else {
            // unbalanced — emit the remainder verbatim and stop
            out.push_str(&rest[i..]);
            return out;
        };
        let args = &args_and_tail[..close];
        rest = &args_and_tail[close + 1..];

        let (name, fallback) = match split_top_comma(args) {
            Some((n, f)) => (n.trim(), Some(f.trim())),
            None => (args.trim(), None),
        };

        if *budget == 0 {
            // guard exhausted: emit verbatim
            out.push_str("var(");
            out.push_str(args);
            out.push(')');
        } else if let Some(v) = vars.get(name) {
            *budget -= 1;
            out.push_str(v);
        } else if let Some(fb) = fallback {
            *budget -= 1;
            let sub = substitute_vars(fb, vars, budget);
            out.push_str(&sub);
        } else {
            // guaranteed-invalid: leave verbatim so the renderer drops just this declaration
            out.push_str("var(");
            out.push_str(args);
            out.push(')');
        }
    }
    out.push_str(rest);
    out
}

// skia-canvas: split a var() argument list on its first top-level comma (parens balanced), giving
// (name, fallback). Returns None when there's no top-level comma (name only).
fn split_top_comma(s: &str) -> Option<(&str, &str)> {
    let mut depth = 0i32;
    for (i, b) in s.bytes().enumerate() {
        match b {
            b'(' => depth += 1,
            b')' => depth -= 1,
            b',' if depth == 0 => return Some((&s[..i], &s[i + 1..])),
            _ => {}
        }
    }
    None
}

impl fmt::Display for StyleSheet<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (i, rule) in self.rules.iter().enumerate() {
            write!(f, "{} {{ ", rule.selector)?;
            for dec in &rule.declarations {
                write!(f, "{}:{}", dec.name, dec.value)?;
                if dec.important {
                    write!(f, " !important")?;
                }
                write!(f, ";")?;
            }
            write!(f, " }}")?;

            if i != self.rules.len() - 1 {
                writeln!(f)?;
            }
        }

        Ok(())
    }
}

impl Default for StyleSheet<'_> {
    fn default() -> Self {
        Self::new()
    }
}

fn consume_statement<'a>(s: &mut Stream<'a>, rules: &mut Vec<Rule<'a>>) -> Result<(), Error> {
    if s.curr_byte() == Ok(b'@') {
        s.advance(1);
        consume_at_rule(s)
    } else {
        consume_rule_set(s, rules)
    }
}

fn consume_at_rule(s: &mut Stream<'_>) -> Result<(), Error> {
    let ident = s.consume_ident()?;
    warn!("The @{} rule is not supported. Skipped.", ident);

    s.skip_bytes(|c| c != b';' && c != b'{');

    match s.curr_byte()? {
        b';' => s.advance(1),
        b'{' => consume_block(s),
        _ => {}
    }

    Ok(())
}

fn consume_rule_set<'a>(s: &mut Stream<'a>, rules: &mut Vec<Rule<'a>>) -> Result<(), Error> {
    let start_rule_idx = rules.len();

    while s.curr_byte()? == b',' || start_rule_idx == rules.len() {
        if s.curr_byte()? == b',' {
            s.advance(1);
        }

        let (selector, offset) = parse(s.slice_tail());
        s.advance(offset);
        s.skip_spaces();

        if let Some(selector) = selector {
            rules.push(Rule {
                selector,
                declarations: Vec::new(),
            });
        }

        match s.curr_byte()? {
            b'{' => break,
            b',' => {}
            _ => {
                s.skip_bytes(|c| c != b'{');
                break;
            }
        }
    }

    s.try_consume_byte(b'{');

    let declarations = consume_declarations(s)?;
    for rule in rules.iter_mut().skip(start_rule_idx) {
        rule.declarations = declarations.clone();
    }

    s.try_consume_byte(b'}');

    Ok(())
}

fn consume_block(s: &mut Stream<'_>) {
    s.try_consume_byte(b'{');
    consume_until_block_end(s);
}

fn consume_until_block_end(s: &mut Stream<'_>) {
    // Block can have nested blocks, so we have to check for matching braces.
    // We simply counting the number of opening braces, which is incorrect,
    // since `{` can be inside a string, but it's fine for majority of the cases.

    let mut braces = 0;
    while !s.at_end() {
        match s.curr_byte_unchecked() {
            b'{' => {
                braces += 1;
            }
            b'}' => {
                if braces == 0 {
                    break;
                } else {
                    braces -= 1;
                }
            }
            _ => {}
        }

        s.advance(1);
    }

    s.try_consume_byte(b'}');
}

fn consume_declarations<'a>(s: &mut Stream<'a>) -> Result<Vec<Declaration<'a>>, Error> {
    let mut declarations = Vec::new();

    while !s.at_end() && s.curr_byte() != Ok(b'}') {
        match consume_declaration(s) {
            Ok(declaration) => declarations.push(declaration),
            // skia-canvas: recover past the malformed declaration to the next `;` (staying inside
            // the block) instead of abandoning the rest of the rule. Upstream did
            // `consume_until_block_end(s); break;`, discarding every later declaration.
            Err(_) => recover_declaration(s, true),
        }
    }

    Ok(declarations)
}

// skia-canvas: CSS declaration-list error recovery. Skips past a malformed declaration to the next
// top-level `;`. Quoted strings are consumed whole, and `()`/`[]`/`{}` are balanced (the brace
// nesting matches upstream's `consume_until_block_end`, extended with string/paren awareness), so a
// `;` inside a value, url(...), or a stray nested block doesn't end recovery early. When
// `stop_at_brace` is set, the rule's own closing `}` (nesting depth 0) stops recovery without being
// consumed, so the caller can close the block. Leaves the stream just past the `;`, at that `}`, or
// at EOF. Both call sites previously aborted the whole declaration list on the first invalid token.
fn recover_declaration(s: &mut Stream<'_>, stop_at_brace: bool) {
    let mut depth = 0u32;
    while let Ok(c) = s.curr_byte() {
        match c {
            b'"' | b'\'' => {
                if s.consume_string().is_err() {
                    return; // unterminated string: consume_string already ran to EOF
                }
                continue; // it advanced past the closing quote; don't double-advance
            }
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' => depth = depth.saturating_sub(1),
            b'}' if depth > 0 => depth -= 1,   // closes a nested block
            b'}' if stop_at_brace => return,   // the rule's own block end (depth 0)
            b';' if depth == 0 => {
                s.advance(1);
                return;
            }
            _ => {}
        }
        s.advance(1);
    }
}

/// A declaration tokenizer.
///
/// skia-canvas: a malformed declaration is skipped (up to the next `;`) and iteration continues, so
/// one invalid declaration no longer discards the declarations after it (CSS declaration-list
/// recovery). Upstream stopped at the first invalid token.
///
/// # Example
///
/// ```
/// use simplecss::{DeclarationTokenizer, Declaration};
///
/// let mut t = DeclarationTokenizer::from("background: url(\"img.png\"); color:red !important");
/// assert_eq!(t.next().unwrap(), Declaration { name: "background", value: "url(\"img.png\")", important: false });
/// assert_eq!(t.next().unwrap(), Declaration { name: "color", value: "red", important: true });
/// ```
pub struct DeclarationTokenizer<'a> {
    stream: Stream<'a>,
}

impl<'a> From<&'a str> for DeclarationTokenizer<'a> {
    fn from(text: &'a str) -> Self {
        DeclarationTokenizer {
            stream: Stream::from(text),
        }
    }
}

impl<'a> Iterator for DeclarationTokenizer<'a> {
    type Item = Declaration<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        // skia-canvas: loop so a malformed declaration is skipped (see recover_declaration) and
        // iteration continues, instead of upstream's `jump_to_end(); None` which discarded every
        // declaration after the first invalid one.
        loop {
            let _ = self.stream.skip_spaces_and_comments();

            if self.stream.at_end() {
                return None;
            }

            match consume_declaration(&mut self.stream) {
                Ok(v) => return Some(v),
                // no block context here, so stop_at_brace = false
                Err(_) => recover_declaration(&mut self.stream, false),
            }
        }
    }
}

fn consume_declaration<'a>(s: &mut Stream<'a>) -> Result<Declaration<'a>, Error> {
    s.skip_spaces_and_comments()?;

    // Parse name.

    // https://snook.ca/archives/html_and_css/targetting_ie7
    if s.curr_byte() == Ok(b'*') {
        s.advance(1);
    }

    // skia-canvas: accept custom-property names (`--foo`). consume_ident rejects the second dash,
    // so capture `--` + name-chars directly. The value is still parsed as a normal declaration
    // value (fine now that nested parens parse); we don't interpret custom properties, just keep
    // them from tripping the parser and pass them along like any other declaration.
    let name = if s.slice_tail().starts_with("--") {
        let start = s.pos();
        s.advance(2); // the `--`
        s.skip_bytes(|c| c == b'-' || c == b'_' || c.is_ascii_alphanumeric());
        s.slice_range(start, s.pos())
    } else {
        s.consume_ident()?
    };

    s.skip_spaces_and_comments()?;
    s.consume_byte(b':')?;
    s.skip_spaces_and_comments()?;

    // Parse value.
    let start = s.pos();
    let mut end = s.pos();
    while consume_term(s).is_ok() {
        end = s.pos();
        s.skip_spaces_and_comments()?;
    }
    let value = s.slice_range(start, end).trim();

    s.skip_spaces_and_comments()?;

    // Check for `important`.
    let mut important = false;
    if s.curr_byte() == Ok(b'!') {
        s.advance(1);
        s.skip_spaces_and_comments()?;
        if s.slice_tail().starts_with("important") {
            s.advance(9);
            important = true;
        }
    }

    s.skip_spaces_and_comments()?;

    while s.curr_byte() == Ok(b';') {
        s.advance(1);
        s.skip_spaces_and_comments()?;
    }

    s.skip_spaces_and_comments()?;

    if value.is_empty() {
        return Err(Error::InvalidValue(s.gen_text_pos_from(start)));
    }

    Ok(Declaration {
        name,
        value,
        important,
    })
}

fn consume_term(s: &mut Stream<'_>) -> Result<(), Error> {
    fn consume_digits(s: &mut Stream<'_>) {
        while let Ok(b'0'..=b'9') = s.curr_byte() {
            s.advance(1);
        }
    }

    match s.curr_byte()? {
        b'#' => {
            s.advance(1);
            match s.consume_ident() {
                Ok(_) => {}
                Err(_) => {
                    // Try consume as a hex color.
                    while let Ok(c) = s.curr_byte() {
                        match c {
                            b'0'..=b'9' | b'a'..=b'f' | b'A'..=b'F' => s.advance(1),
                            _ => break,
                        }
                    }
                }
            }
        }
        b'+' | b'-' | b'0'..=b'9' | b'.' => {
            // Consume number.

            s.advance(1);
            consume_digits(s);
            if s.curr_byte() == Ok(b'.') {
                s.advance(1);
                consume_digits(s);
            }

            if s.curr_byte() == Ok(b'%') {
                s.advance(1);
            } else {
                // Consume suffix if any.
                let _ = s.consume_ident();
            }
        }
        b'\'' | b'"' => {
            s.consume_string()?;
        }
        b',' => {
            s.advance(1);
        }
        _ => {
            let _ = s.consume_ident()?;

            // Consume function.
            if s.curr_byte() == Ok(b'(') {
                // skia-canvas: balance nested parens (and step over strings) so values like
                // calc((a) - (b)), var(--x, rgb(...)), or a gradient with color-function args
                // aren't truncated at the first ')'. Upstream skipped to the first ')' only.
                s.advance(1); // past '('
                let mut depth = 1u32;
                while depth > 0 {
                    match s.curr_byte()? {
                        b'(' => depth += 1,
                        b')' => depth -= 1,
                        b'"' | b'\'' => {
                            s.consume_string()?;
                            continue;
                        }
                        _ => {}
                    }
                    s.advance(1);
                }
            }
        }
    }

    Ok(())
}
