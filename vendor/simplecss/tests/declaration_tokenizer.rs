// Copyright 2019 the SimpleCSS Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Declaration Tokenizer

use simplecss::*;

macro_rules! tokenize {
    ($name:ident, $text:expr, $( $token:expr ),*) => (
        #[test]
        fn $name() {
            let mut t = DeclarationTokenizer::from($text);
            $(
                assert_eq!(t.next().unwrap(), $token);
            )*

            assert!(t.next().is_none());
        }
    )
}

fn declare<'a>(name: &'a str, value: &'a str) -> Declaration<'a> {
    Declaration {
        name,
        value,
        important: false,
    }
}

fn declare_important<'a>(name: &'a str, value: &'a str) -> Declaration<'a> {
    Declaration {
        name,
        value,
        important: true,
    }
}

tokenize!(tokenize_01, "",);

tokenize!(tokenize_02, " ",);

tokenize!(tokenize_03, "/**/",);

tokenize!(tokenize_04, "color:red", declare("color", "red"));

tokenize!(tokenize_05, "color:red;", declare("color", "red"));

tokenize!(tokenize_06, "color:red ", declare("color", "red"));

tokenize!(tokenize_07, " color: red; ", declare("color", "red"));

tokenize!(tokenize_08, "  color  :  red  ; ", declare("color", "red"));

tokenize!(
    tokenize_09,
    "  color:red;;;;color:red; ",
    declare("color", "red"),
    declare("color", "red")
);

tokenize!(
    tokenize_10,
    "background: url(\"img.png\");",
    declare("background", "url(\"img.png\")")
);

tokenize!(
    tokenize_11,
    "background: url(\"{}\");",
    declare("background", "url(\"{}\")")
);

tokenize!(
    tokenize_12,
    "color: red ! important",
    declare_important("color", "red")
);

tokenize!(
    tokenize_13,
    "color: red !important",
    declare_important("color", "red")
);

tokenize!(
    tokenize_14,
    "color: red!important",
    declare_important("color", "red")
);

tokenize!(
    tokenize_15,
    "color: red !/**/important",
    declare_important("color", "red")
);

tokenize!(
    tokenize_16,
    "border: 1em solid blue",
    declare("border", "1em solid blue")
);

tokenize!(
    tokenize_17,
    "background: navy url(support/diamond.png) -2em -2em no-repeat",
    declare(
        "background",
        "navy url(support/diamond.png) -2em -2em no-repeat"
    )
);

tokenize!(tokenize_18, "/**/color:red", declare("color", "red"));

tokenize!(tokenize_19, "/* *\\/*/color: red;", declare("color", "red"));

tokenize!(
    tokenize_20,
    "/**/color/**/:/**/red/**/;/**/",
    declare("color", "red")
);

tokenize!(tokenize_21, "\ncolor\n:\nred\n;\n", declare("color", "red"));

tokenize!(tokenize_22, "{color:red}",);

tokenize!(tokenize_23, "(color:red)",);

tokenize!(tokenize_24, "[color:red]",);

tokenize!(tokenize_25, "color:",);

tokenize!(tokenize_26, "value:\"text\"", declare("value", "\"text\""));

tokenize!(tokenize_27, "value:'text'", declare("value", "'text'"));

tokenize!(tokenize_28, "color:#fff", declare("color", "#fff"));
tokenize!(tokenize_29, "color:0.5", declare("color", "0.5"));

tokenize!(tokenize_30, "color:.5", declare("color", ".5"));

tokenize!(tokenize_31, "color:#FFF", declare("color", "#FFF"));

tokenize!(
    tokenize_32,
    "content: counter(chapno, upper-roman) \". \"",
    declare("content", "counter(chapno, upper-roman) \". \"")
);

tokenize!(
    tokenize_33,
    "font-family:'Noto Serif','DejaVu Serif',serif",
    declare("font-family", "'Noto Serif','DejaVu Serif',serif")
);

tokenize!(tokenize_34, "*zoom:1;", declare("zoom", "1"));

//tokenize!(tokenize_, "@unsupported { splines: reticulating } color: green",
//    declare("color", "green")
//);

//tokenize!(tokenize_, "/*\\*/*/color: red;", declare("color", "red"));

//tokenize!(tokenize_, "\"this is a string]}\"\"[{\\\"'\";  /*should be parsed as a string but be ignored*/
//    {{}}[]'';                     /*should be parsed as nested blocks and a string but be ignored*/
//    color: red;", declare("color", "red"));

// skia-canvas: CSS declaration-list error recovery. A malformed declaration is skipped up to the
// next top-level `;` (stepping over strings and parens) so it no longer discards the declarations
// after it; upstream aborted the whole list at the first invalid token.

// a bad property name (e.g. a custom property `--x`) no longer poisons the rest of the list
tokenize!(recover_bad_name, "--x: red; fill: blue", declare("fill", "blue"));

// a missing colon is skipped; later declarations survive
tokenize!(recover_missing_colon, "fill blue; stroke: red", declare("stroke", "red"));

// a `;` inside a quoted string in a malformed declaration doesn't cut recovery short, and no
// phantom declaration is manufactured from the string's contents
tokenize!(
    recover_semicolon_in_string,
    "--tip: \"note; color: red\"; fill: blue",
    declare("fill", "blue")
);

// a `;` inside url()/parens in a malformed declaration is stepped over
tokenize!(
    recover_semicolon_in_parens,
    "--bg: url(a;b.png); fill: blue",
    declare("fill", "blue")
);

// recovery in the middle of a list keeps declarations on both sides
tokenize!(
    recover_between_valid,
    "fill: blue; --x: red; stroke: green",
    declare("fill", "blue"),
    declare("stroke", "green")
);

// consecutive malformed declarations all get skipped
tokenize!(recover_consecutive, "--a: 1; --b: 2; fill: blue", declare("fill", "blue"));

// a malformed declaration at the very end terminates cleanly (no hang)
tokenize!(recover_trailing_bad, "fill: blue; --x: red", declare("fill", "blue"));

// `!important` on a declaration after a recovered one is still honored
tokenize!(
    recover_preserves_important,
    "--x: red; fill: blue !important",
    declare_important("fill", "blue")
);

// a *valid* string containing `;` is untouched (recovery never runs) — regression guard
tokenize!(
    valid_semicolon_in_string_unaffected,
    "content: \"a; b\"; fill: blue",
    declare("content", "\"a; b\""),
    declare("fill", "blue")
);
