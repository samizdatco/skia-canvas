// Copyright 2019 the SimpleCSS Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

//! Select

use simplecss::*;

struct XmlNode<'a, 'input: 'a>(roxmltree::Node<'a, 'input>);

impl<'a, 'input: 'a> XmlNode<'a, 'input> {
    fn select(&self, text: &str) -> Vec<roxmltree::Node<'a, 'input>> {
        let selectors = Selector::parse(text).unwrap();
        let mut nodes = Vec::new();
        for node in self.0.descendants().filter(|n| n.is_element()) {
            if selectors.matches(&XmlNode(node)) {
                nodes.push(node);
            }
        }

        nodes
    }
}

impl Element for XmlNode<'_, '_> {
    fn parent_element(&self) -> Option<Self> {
        self.0.parent_element().map(XmlNode)
    }

    fn prev_sibling_element(&self) -> Option<Self> {
        self.0.prev_sibling_element().map(XmlNode)
    }

    fn next_sibling_element(&self) -> Option<Self> {
        self.0.next_sibling_element().map(XmlNode)
    }

    fn has_local_name(&self, local_name: &str) -> bool {
        self.0.tag_name().name() == local_name
    }

    fn local_name(&self) -> &str {
        self.0.tag_name().name()
    }

    fn attribute_matches(&self, local_name: &str, operator: AttributeOperator<'_>) -> bool {
        match self.0.attribute(local_name) {
            Some(value) => operator.matches(value),
            None => false,
        }
    }

    // structural pseudos (:*-child, :*-of-type, :nth-*, :not) are now matched generically by the
    // crate; only state-dependent ones reach here, and none match under static rendering.
    fn pseudo_class_matches(&self, _class: PseudoClass<'_>) -> bool {
        false
    }
}

macro_rules! match_single {
    ($doc:expr, $selector:expr) => {{
        let nodes = XmlNode($doc.root_element()).select($selector);
        assert_eq!(nodes.len(), 1);
        nodes[0].attribute("id").unwrap()
    }};
}

macro_rules! match_none {
    ($doc:expr, $selector:expr) => {{
        assert_eq!(XmlNode($doc.root_element()).select($selector).len(), 0);
    }};
}

#[test]
fn select_01() {
    let doc = roxmltree::Document::parse("<div id='div1'/>").unwrap();
    assert_eq!(match_single!(doc, "*"), "div1");
}

#[test]
fn select_02() {
    let doc = roxmltree::Document::parse("<div id='div1'/>").unwrap();
    assert_eq!(match_single!(doc, "div"), "div1");
    match_none!(doc, "p");
}

#[test]
fn select_03() {
    let doc = roxmltree::Document::parse("<div id='div1'/>").unwrap();
    assert_eq!(match_single!(doc, "#div1"), "div1");
    match_none!(doc, "#d1");
}

#[test]
fn select_04() {
    let doc = roxmltree::Document::parse("<div id='div1'/>").unwrap();
    match_none!(doc, "p#div1");
}

#[test]
fn select_05() {
    let doc = roxmltree::Document::parse(
        "\
<div id='div1'>
    <p id='p1'/>
</div>
",
    )
    .unwrap();

    assert_eq!(match_single!(doc, "div p"), "p1");
}

#[test]
fn select_06() {
    let doc = roxmltree::Document::parse(
        "\
<div id='div1'>
    <g id='g1'>
        <p id='p1'/>
    </g>
</div>
",
    )
    .unwrap();

    assert_eq!(match_single!(doc, "div p"), "p1");
}

#[test]
fn select_07() {
    let doc = roxmltree::Document::parse(
        "\
<div id='div1'>
    <div id='div2'>
        <g id='g1'>
            <p id='p1'/>
        </g>
    </div>
</div>
",
    )
    .unwrap();

    assert_eq!(match_single!(doc, "div p"), "p1");
}

#[test]
fn select_08() {
    let doc = roxmltree::Document::parse(
        "\
<div id='div1'>
    <g id='g1'>
        <p id='p1'>
            <div/>
        </p>
    </g>
</div>
",
    )
    .unwrap();

    assert_eq!(match_single!(doc, "div p"), "p1");
}

#[test]
fn select_09() {
    let doc = roxmltree::Document::parse(
        "\
<div id='div1'>
    <g id='g1'>
        <p id='p1'/>
    </g>
</div>
",
    )
    .unwrap();

    assert_eq!(match_single!(doc, "div g p"), "p1");
}

#[test]
fn select_10() {
    let doc = roxmltree::Document::parse(
        "\
<div id='div1'>
    <q id='g1'>
        <p id='p1'/>
    </q>
</div>
",
    )
    .unwrap();

    match_none!(doc, "div g p");
}

#[test]
fn select_11() {
    let doc = roxmltree::Document::parse(
        "\
<div id='div1'>
    <g id='g1'>
        <p id='p1'/>
    </g>
</div>
",
    )
    .unwrap();

    assert_eq!(match_single!(doc, "div * p"), "p1");
}

#[test]
fn select_12() {
    let doc = roxmltree::Document::parse(
        "\
<div id='div1'>
    <p id='p1'>
        <rect id='rect1'/>
        <rect id='rect2' color='green'/>
    </p>
</div>
",
    )
    .unwrap();

    assert_eq!(match_single!(doc, "div p *[color]"), "rect2");
    assert_eq!(match_single!(doc, "div p [color]"), "rect2");
}

#[test]
fn select_13() {
    let doc = roxmltree::Document::parse(
        "\
<div id='div1'>
    <p id='p1'/>
</div>
",
    )
    .unwrap();

    assert_eq!(match_single!(doc, "div > p"), "p1");
}

#[test]
fn select_14() {
    let doc = roxmltree::Document::parse(
        "\
<p id='p1'/>
",
    )
    .unwrap();

    match_none!(doc, "div > p");
}

#[test]
fn select_15() {
    let doc = roxmltree::Document::parse(
        "\
<div id='div1'>
    <g id='g1'>
        <p id='p1'/>
    </g>
</div>
",
    )
    .unwrap();

    match_none!(doc, "div > p");
}

#[test]
fn select_16() {
    let doc = roxmltree::Document::parse(
        "\
<div id='div1'>
    <p>
        <ol>
            <li>
                <g>
                    <p id='p1'/>
                </g>
            </li>
        </ol>
    </p>
</div>
",
    )
    .unwrap();

    assert_eq!(match_single!(doc, "div ol>li p"), "p1");
}

#[test]
fn select_17() {
    let doc = roxmltree::Document::parse(
        "\
<div id='div1'>
    <p>
        <ol>
            <g>
                <li>
                    <g>
                        <p id='p1'/>
                    </g>
                </li>
            </g>
        </ol>
    </p>
</div>
",
    )
    .unwrap();

    match_none!(doc, "div ol>li p");
}

#[test]
fn select_18() {
    let doc = roxmltree::Document::parse(
        "\
<div id='div1'>
    <g/>
    <p id='p1'/>
</div>
",
    )
    .unwrap();

    assert_eq!(match_single!(doc, "g + p"), "p1");
}

#[test]
fn select_19() {
    let doc = roxmltree::Document::parse(
        "\
<div id='div1'>
    <test/>
    <g/>
    <p id='p1'/>
</div>
",
    )
    .unwrap();

    assert_eq!(match_single!(doc, "g + p"), "p1");
}

#[test]
fn select_20() {
    let doc = roxmltree::Document::parse(
        "\
<div id='div1'>
    <p id='p1'/>
    <g/>
</div>
",
    )
    .unwrap();

    match_none!(doc, "g + p");
}

#[test]
fn select_21() {
    let doc = roxmltree::Document::parse(
        "\
<div id='div1'>
    <p id='p1'/>
</div>
",
    )
    .unwrap();

    match_none!(doc, "div + p");
}

#[test]
fn select_22() {
    let doc = roxmltree::Document::parse(
        "\
<div id='div1'>
    <p id='p1'/>
</div>
",
    )
    .unwrap();

    assert_eq!(match_single!(doc, "[id=p1]"), "p1");
}

#[test]
fn select_23() {
    let doc = roxmltree::Document::parse(
        "\
<div id='div1'>
    <p id='p1' class='test warn'/>
</div>
",
    )
    .unwrap();

    assert_eq!(match_single!(doc, "[class~=warn]"), "p1");
}

#[test]
fn select_24() {
    let doc = roxmltree::Document::parse(
        "\
<div id='div1'>
    <p id='p1' class='test warn'/>
</div>
",
    )
    .unwrap();

    match_none!(doc, "[class~='test warn']");
}

#[test]
fn select_25() {
    let doc = roxmltree::Document::parse(
        "\
<div id='div1'>
    <p id='p1' lang='en'/>
</div>
",
    )
    .unwrap();

    assert_eq!(match_single!(doc, "[lang=en]"), "p1");
    assert_eq!(match_single!(doc, "[lang|=en]"), "p1");
}

#[test]
fn select_26() {
    let doc = roxmltree::Document::parse(
        "\
<div id='div1'>
    <p id='p1' lang='en-US'/>
</div>
",
    )
    .unwrap();

    assert_eq!(match_single!(doc, "[lang='en-US']"), "p1");
    assert_eq!(match_single!(doc, "[lang|=en]"), "p1");
}

#[test]
fn select_27() {
    let doc = roxmltree::Document::parse(
        "\
<div id='div1'>
    <p id='p1' class='pastoral blue aqua marine'/>
</div>
",
    )
    .unwrap();

    assert_eq!(match_single!(doc, ".marine.pastoral"), "p1");
}

#[test]
fn select_28() {
    let doc = roxmltree::Document::parse(
        "\
<div id='div1'>
    <p id='p1'/>
</div>
",
    )
    .unwrap();

    assert_eq!(match_single!(doc, "p:first-child"), "p1");
}

#[test]
fn select_29() {
    let doc = roxmltree::Document::parse(
        "\
<div id='div1'>
    <rect/>
    <p id='p1'/>
</div>
",
    )
    .unwrap();

    match_none!(doc, "p:first-child");
}

#[test]
fn select_30() {
    let doc = roxmltree::Document::parse(
        "\
<div id='div1'>
    <p id='p1'/>
    <p id='p2'/>
</div>
",
    )
    .unwrap();

    let nodes = XmlNode(doc.root_element()).select(":first-child");
    assert_eq!(nodes.len(), 2);
    assert_eq!(nodes[0].attribute("id").unwrap(), "div1");
    assert_eq!(nodes[1].attribute("id").unwrap(), "p1");
}

#[test]
fn to_string() {
    let selectors = Selector::parse("a > b").unwrap();
    assert_eq!(selectors.to_string(), "a > b");
}

// skia-canvas: added structural pseudo-classes, :not(), and the general sibling combinator.

fn ids(doc: &roxmltree::Document<'_>, selector: &str) -> Vec<String> {
    let mut v: Vec<String> = XmlNode(doc.root_element())
        .select(selector)
        .iter()
        .filter_map(|n| n.attribute("id"))
        .map(|s| s.to_string())
        .collect();
    v.sort();
    v
}

#[test]
fn structural_type_pseudos() {
    let doc = roxmltree::Document::parse(
        "<div><p id='p1'/><p id='p2'/><rect id='r1'/><p id='p3'/></div>",
    )
    .unwrap();
    assert_eq!(match_single!(doc, "p:nth-child(2)"), "p2");
    assert_eq!(match_single!(doc, "p:first-of-type"), "p1");
    assert_eq!(match_single!(doc, "p:last-of-type"), "p3");
    assert_eq!(match_single!(doc, "p:nth-of-type(2)"), "p2");
    assert_eq!(match_single!(doc, "rect:only-of-type"), "r1");
    assert_eq!(match_single!(doc, "p:last-child"), "p3");
    match_none!(doc, "p:only-child");
}

#[test]
fn nth_child_anb() {
    let doc = roxmltree::Document::parse(
        "<div><i id='i1'/><i id='i2'/><i id='i3'/><i id='i4'/></div>",
    )
    .unwrap();
    assert_eq!(ids(&doc, "i:nth-child(odd)"), ["i1", "i3"]);
    assert_eq!(ids(&doc, "i:nth-child(even)"), ["i2", "i4"]);
    assert_eq!(ids(&doc, "i:nth-child(2n+1)"), ["i1", "i3"]);
    assert_eq!(ids(&doc, "i:nth-child(-n+2)"), ["i1", "i2"]);
    assert_eq!(ids(&doc, "i:nth-last-child(1)"), ["i4"]);
}

#[test]
fn not_and_general_sibling() {
    let doc = roxmltree::Document::parse(
        "<div><a id='a1'/><b id='b1'/><b id='b2' class='skip'/><b id='b3'/></div>",
    )
    .unwrap();
    // general sibling: all b's after a; adjacent: only the immediate one
    assert_eq!(ids(&doc, "a ~ b"), ["b1", "b2", "b3"]);
    assert_eq!(match_single!(doc, "a + b"), "b1");
    // :not() with a simple inner
    assert_eq!(ids(&doc, "b:not(.skip)"), ["b1", "b3"]);
    // :not() with a complex inner (adjacent combinator): excludes the b right after a
    assert_eq!(ids(&doc, "b:not(a + b)"), ["b2", "b3"]);
    // :not() with a comma-list is deferred → never matches (graceful skip)
    match_none!(doc, "b:not(a, b)");
}

#[test]
fn resolve_inline_cascade() {
    let doc = roxmltree::Document::parse("<div><rect id='r' class='x'/></div>").unwrap();
    let rect = doc.descendants().find(|n| n.has_tag_name("rect")).unwrap();
    let el = XmlNode(rect);

    // higher specificity wins (#r over .x)
    assert_eq!(
        StyleSheet::parse(".x{fill:red} #r{fill:green}").resolve_inline(&el, ""),
        "fill:green;"
    );
    // the element's own inline style beats a normal stylesheet rule
    assert_eq!(
        StyleSheet::parse(".x{fill:red}").resolve_inline(&el, "fill:green"),
        "fill:green;"
    );
    // a stylesheet !important beats a normal inline; the keyword is stripped from the output
    assert_eq!(
        StyleSheet::parse(".x{fill:green !important}").resolve_inline(&el, "fill:red"),
        "fill:green;"
    );
    // one declaration per property, sorted by name (deterministic)
    assert_eq!(
        StyleSheet::parse("rect{fill:green; stroke:blue}").resolve_inline(&el, ""),
        "fill:green;stroke:blue;"
    );
    // nothing applies → empty string
    assert_eq!(StyleSheet::parse("circle{fill:green}").resolve_inline(&el, ""), "");
}

// skia-canvas: var() substitution with root-only scoping. References resolve against the document
// root's custom properties (`:root`/`svg`/matching rules) overlaid with the element's own.
#[test]
fn resolve_inline_vars() {
    let doc = roxmltree::Document::parse("<svg><rect class='x'/></svg>").unwrap();
    let rect = doc.descendants().find(|n| n.has_tag_name("rect")).unwrap();
    let el = XmlNode(rect);

    // a :root-defined custom property resolves for a descendant
    assert_eq!(
        StyleSheet::parse(":root{--c:red} .x{fill:var(--c)}").resolve_inline(&el, ""),
        "fill:red;"
    );
    // an svg type-selector-defined property works too
    assert_eq!(
        StyleSheet::parse("svg{--c:blue} .x{fill:var(--c)}").resolve_inline(&el, ""),
        "fill:blue;"
    );
    // same-element inline define-and-use; the custom property is consumed, not emitted
    assert_eq!(
        StyleSheet::parse("").resolve_inline(&el, "--c:green;fill:var(--c)"),
        "fill:green;"
    );
    // fallback is used when the referenced property is undefined
    assert_eq!(
        StyleSheet::parse("").resolve_inline(&el, "fill:var(--missing, orange)"),
        "fill:orange;"
    );
    // a var() nested in the fallback slot is resolved
    assert_eq!(
        StyleSheet::parse(":root{--c:teal} .x{fill:var(--missing, var(--c))}").resolve_inline(&el, ""),
        "fill:teal;"
    );
    // the element's own custom property overrides the root's
    assert_eq!(
        StyleSheet::parse(":root{--c:red} .x{--c:blue;fill:var(--c)}").resolve_inline(&el, ""),
        "fill:blue;"
    );
    // multiple / partial var() in a single value
    assert_eq!(
        StyleSheet::parse(":root{--w:2px;--c:navy} .x{stroke:var(--w) solid var(--c)}")
            .resolve_inline(&el, ""),
        "stroke:2px solid navy;"
    );
    // an undefined var() with no fallback is left verbatim (Skia drops just that declaration)
    assert_eq!(
        StyleSheet::parse("").resolve_inline(&el, "fill:var(--nope)"),
        "fill:var(--nope);"
    );
}

// skia-canvas: :root matches the document root element, not its descendants
#[test]
fn root_pseudo() {
    let doc = roxmltree::Document::parse("<svg><rect/></svg>").unwrap();
    let svg = doc.root_element();
    let rect = doc.descendants().find(|n| n.has_tag_name("rect")).unwrap();
    let sel = Selector::parse(":root").unwrap();
    assert!(sel.matches(&XmlNode(svg)));
    assert!(!sel.matches(&XmlNode(rect)));
}
