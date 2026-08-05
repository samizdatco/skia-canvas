// Copyright 2019 the SimpleCSS Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Stylesheet

use simplecss::*;

#[test]
fn style_01() {
    let style = StyleSheet::parse("");
    assert_eq!(style.to_string(), "");
}

#[test]
fn style_02() {
    let style = StyleSheet::parse("a {}");
    assert_eq!(style.to_string(), "");
}

#[test]
fn style_03() {
    let style = StyleSheet::parse("a { color:red }");
    assert_eq!(style.to_string(), "a { color:red; }");
}

#[test]
fn style_04() {
    let style = StyleSheet::parse("/**/");
    assert_eq!(style.to_string(), "");
}

#[test]
fn style_05() {
    let style = StyleSheet::parse("a { color:red } /**/");
    assert_eq!(style.to_string(), "a { color:red; }");
}

#[test]
fn style_06() {
    let style = StyleSheet::parse("a, b { color:red }");
    assert_eq!(style.to_string(), "a { color:red; }\nb { color:red; }");
}

#[test]
fn style_07() {
    let style = StyleSheet::parse("a, { color:red }");
    assert_eq!(style.to_string(), "a { color:red; }");
}

#[test]
fn style_08() {
    let style = StyleSheet::parse("a,, { color:red }");
    assert_eq!(style.to_string(), "a { color:red; }");
}

#[test]
fn style_09() {
    let style = StyleSheet::parse("a,,b { color:red }");
    assert_eq!(style.to_string(), "a { color:red; }\nb { color:red; }");
}

#[test]
fn style_10() {
    let style = StyleSheet::parse(",a { color:red }");
    assert_eq!(style.to_string(), "a { color:red; }");
}

#[test]
fn style_11() {
    let style = StyleSheet::parse("@import \"subs.css\";\na { color:red }");
    assert_eq!(style.to_string(), "a { color:red; }");
}

#[test]
fn style_12() {
    let style = StyleSheet::parse(
        "\
@media screen {
    p:before { content: 'Hello'; }
}
a { color:red }",
    );
    assert_eq!(style.to_string(), "a { color:red; }");
}

#[test]
fn style_13() {
    let style = StyleSheet::parse("a > { color:red }");
    assert_eq!(style.to_string(), "");
}

#[test]
fn style_14() {
    let style = StyleSheet::parse("p { color:green; color }");
    assert_eq!(style.to_string(), "p { color:green; }");
}

#[test]
fn style_15() {
    let style = StyleSheet::parse("p { color; color:green }");
    // skia-canvas: declaration-list recovery now keeps `color:green` after the malformed `color`
    // (was "" upstream — the pre-existing `// TODO: should be 'p { color:green; }'`).
    assert_eq!(style.to_string(), "p { color:green; }");
}

#[test]
fn style_16() {
    let style = StyleSheet::parse("p { color:green; color: }");
    assert_eq!(style.to_string(), "p { color:green; }");
}

#[test]
fn style_17() {
    let style = StyleSheet::parse("p { color:green; color:; color:red; }");
    assert_eq!(style.to_string(), "p { color:green; }");
}

#[test]
fn style_18() {
    let style = StyleSheet::parse("p { color:green; color{;color:maroon} }");
    assert_eq!(style.to_string(), "p { color:green; }");
}

#[test]
fn style_19() {
    let style = StyleSheet::parse("p { color{;color:maroon} color:green; }");
    assert_eq!(style.to_string(), ""); // TODO: should be 'p { color:green; }'
}

#[test]
fn style_20() {
    let style = StyleSheet::parse(
        "\
        h1 { color: green }
        h2 & h3 { color: red }
        h4 { color: black }
    ",
    );
    assert_eq!(
        style.to_string(),
        "h1 { color:green; }\nh4 { color:black; }"
    );
}

#[test]
fn style_21() {
    let style = StyleSheet::parse(":le>*");
    assert_eq!(style.to_string(), "");
}

// skia-canvas: CSS declaration-list error recovery inside a rule body. A malformed declaration is
// skipped to the next `;` (staying within the block) instead of discarding the rest of the rule;
// upstream bailed the whole block at the first invalid token.

#[test]
fn recover_bad_name_in_rule() {
    let style = StyleSheet::parse("a { --x:red; fill:blue }");
    assert_eq!(style.to_string(), "a { fill:blue; }");
}

#[test]
fn recover_missing_colon_in_rule() {
    let style = StyleSheet::parse("a { fill blue; stroke:red }");
    assert_eq!(style.to_string(), "a { stroke:red; }");
}

#[test]
fn recover_semicolon_in_string_in_rule() {
    let style = StyleSheet::parse("a { --x:\"y; z\"; fill:blue }");
    assert_eq!(style.to_string(), "a { fill:blue; }");
}

#[test]
fn recover_semicolon_in_parens_in_rule() {
    let style = StyleSheet::parse("a { --bg:url(a;b.png); fill:blue }");
    assert_eq!(style.to_string(), "a { fill:blue; }");
}

#[test]
fn recover_between_valid_in_rule() {
    let style = StyleSheet::parse("a { fill:blue; --x:red; stroke:green }");
    assert_eq!(style.to_string(), "a { fill:blue;stroke:green; }");
}

#[test]
fn recover_stays_within_block() {
    // recovery must stop at the rule's own `}` and not bleed into the next rule
    let style = StyleSheet::parse("a { --x:red } b { color:blue }");
    assert_eq!(style.to_string(), "b { color:blue; }");
}
