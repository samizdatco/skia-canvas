// Copyright 2019 the SimpleCSS Authors
// SPDX-License-Identifier: Apache-2.0 OR MIT

use alloc::{string::String, vec, vec::Vec};
use core::fmt;

use log::warn;

use crate::stream::Stream;
use crate::Error;

/// An attribute selector operator.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum AttributeOperator<'a> {
    /// `[attr]`
    Exists,
    /// `[attr=value]`
    Matches(&'a str),
    /// `[attr~=value]`
    Contains(&'a str),
    /// `[attr|=value]`
    StartsWith(&'a str),
}

impl AttributeOperator<'_> {
    /// Checks that value is matching the operator.
    pub fn matches(&self, value: &str) -> bool {
        match *self {
            AttributeOperator::Exists => true,
            AttributeOperator::Matches(v) => value == v,
            AttributeOperator::Contains(v) => value.split(' ').any(|s| s == v),
            AttributeOperator::StartsWith(v) => {
                // exactly `v` or beginning with `v` immediately followed by `-`
                if value == v {
                    true
                } else if value.starts_with(v) {
                    value.get(v.len()..v.len() + 1) == Some("-")
                } else {
                    false
                }
            }
        }
    }
}

/// A pseudo-class.
#[derive(Clone, Copy, PartialEq, Debug)]
#[allow(missing_docs)]
pub enum PseudoClass<'a> {
    FirstChild,
    // skia-canvas: structural pseudo-classes beyond upstream's lone :first-child.
    LastChild,
    OnlyChild,
    FirstOfType,
    LastOfType,
    OnlyOfType,
    NthChild(Nth),
    NthLastChild(Nth),
    NthOfType(Nth),
    NthLastOfType(Nth),
    // skia-canvas: :not() stores its argument's source text (re-parsed on match) so that
    // PseudoClass stays Copy; comma-separated lists are rejected at parse time, leaving a
    // single (possibly complex) inner selector.
    Not(&'a str),
    Link,
    Visited,
    Hover,
    Active,
    Focus,
    Lang(&'a str),
    // skia-canvas: any unknown/unsupported pseudo (e.g. :target, or a functional pseudo we
    // don't model). Parses successfully but never matches, so the enclosing rule survives
    // (graceful skip) instead of being dropped. Carries the name for Display round-tripping.
    Unsupported(&'a str),
}

/// skia-canvas: the `An+B` micro-syntax of a functional structural pseudo-class
/// (e.g. `:nth-child(2n+1)` parses to `Nth { a: 2, b: 1 }`; `odd` = `2n+1`, `even` = `2n`).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Nth {
    /// The `A` (step) coefficient of `An+B`.
    pub a: i32,
    /// The `B` (offset) term of `An+B`.
    pub b: i32,
}

impl Nth {
    /// skia-canvas: does a 1-based sibling index satisfy `An+B` for some integer `n >= 0`?
    pub fn matches(&self, index: i32) -> bool {
        if self.a == 0 {
            index == self.b
        } else {
            let diff = index - self.b;
            diff % self.a == 0 && diff / self.a >= 0
        }
    }
}

impl fmt::Display for Nth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // not canonical CSS, but re-parses to the same Nth (e.g. "2n+1", "-1n+3").
        write!(f, "{}n{}{}", self.a, if self.b < 0 { "-" } else { "+" }, self.b.abs())
    }
}

impl fmt::Display for PseudoClass<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PseudoClass::FirstChild => write!(f, "first-child"),
            PseudoClass::LastChild => write!(f, "last-child"),
            PseudoClass::OnlyChild => write!(f, "only-child"),
            PseudoClass::FirstOfType => write!(f, "first-of-type"),
            PseudoClass::LastOfType => write!(f, "last-of-type"),
            PseudoClass::OnlyOfType => write!(f, "only-of-type"),
            PseudoClass::NthChild(n) => write!(f, "nth-child({})", n),
            PseudoClass::NthLastChild(n) => write!(f, "nth-last-child({})", n),
            PseudoClass::NthOfType(n) => write!(f, "nth-of-type({})", n),
            PseudoClass::NthLastOfType(n) => write!(f, "nth-last-of-type({})", n),
            PseudoClass::Not(inner) => write!(f, "not({})", inner),
            PseudoClass::Link => write!(f, "link"),
            PseudoClass::Visited => write!(f, "visited"),
            PseudoClass::Hover => write!(f, "hover"),
            PseudoClass::Active => write!(f, "active"),
            PseudoClass::Focus => write!(f, "focus"),
            PseudoClass::Lang(lang) => write!(f, "lang({})", lang),
            PseudoClass::Unsupported(name) => write!(f, "{}", name),
        }
    }
}

/// A trait to query an element node metadata.
pub trait Element: Sized {
    /// Returns a parent element.
    fn parent_element(&self) -> Option<Self>;

    /// Returns a previous sibling element.
    fn prev_sibling_element(&self) -> Option<Self>;

    /// skia-canvas: Returns the next sibling element. Needed for the structural pseudo-classes
    /// that look forward — `:last-child`, `:only-child`, `:nth-last-child`, and the
    /// `:*-of-type` family (upstream only had `prev_sibling_element`, hence only `:first-child`).
    fn next_sibling_element(&self) -> Option<Self>;

    /// Checks that the element has a specified local name.
    fn has_local_name(&self, name: &str) -> bool;

    /// skia-canvas: Returns the element's own local name. Needed to compare sibling types for
    /// the `:*-of-type` pseudo-classes (`has_local_name` can't, since the generic matcher has
    /// no name of its own to pass).
    fn local_name(&self) -> &str;

    /// Checks that the element has a specified attribute.
    fn attribute_matches(&self, local_name: &str, operator: AttributeOperator<'_>) -> bool;

    /// Checks that the element matches a specified pseudo-class.
    ///
    /// skia-canvas: only *state-dependent* pseudo-classes reach this method now
    /// (`:hover`/`:focus`/`:active`/`:target`/`:link`/`:visited` and unsupported ones). The
    /// structural pseudos (`:*-child`, `:*-of-type`, `:nth-*`, `:not`) are matched generically
    /// by the crate via the sibling/name accessors above, so impls that only do static
    /// rendering can return `false` here.
    fn pseudo_class_matches(&self, class: PseudoClass<'_>) -> bool;
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum SimpleSelectorType<'a> {
    Type(&'a str),
    Universal,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum SubSelector<'a> {
    Attribute(&'a str, AttributeOperator<'a>),
    PseudoClass(PseudoClass<'a>),
}

#[derive(Clone, Debug)]
struct SimpleSelector<'a> {
    kind: SimpleSelectorType<'a>,
    subselectors: Vec<SubSelector<'a>>,
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Combinator {
    None,
    Descendant,
    Child,
    AdjacentSibling,
    // skia-canvas: general sibling combinator `~`
    GeneralSibling,
}

#[derive(Clone, Debug)]
struct Component<'a> {
    /// A combinator that precede the selector.
    combinator: Combinator,
    selector: SimpleSelector<'a>,
}

/// A selector.
#[derive(Clone, Debug)]
pub struct Selector<'a> {
    components: Vec<Component<'a>>,
}

impl<'a> Selector<'a> {
    /// Parses a selector from a string.
    ///
    /// Will log any errors as a warnings.
    ///
    /// Parsing will be stopped at EOF, `,` or `{`.
    pub fn parse(text: &'a str) -> Option<Self> {
        parse(text).0
    }

    /// Compute the selector's specificity.
    ///
    /// Cf. <https://www.w3.org/TR/selectors/#specificity>.
    pub fn specificity(&self) -> [u8; 3] {
        let mut spec = [0u8; 3];

        for selector in self.components.iter().map(|c| &c.selector) {
            if matches!(selector.kind, SimpleSelectorType::Type(_)) {
                spec[2] = spec[2].saturating_add(1);
            }

            for sub in &selector.subselectors {
                match sub {
                    SubSelector::Attribute("id", _) => spec[0] = spec[0].saturating_add(1),
                    // skia-canvas: :not() contributes the specificity of its argument, not
                    // of the pseudo itself (CSS Selectors L3/L4).
                    SubSelector::PseudoClass(PseudoClass::Not(inner)) => {
                        if let Some(sel) = Selector::parse(inner) {
                            let s = sel.specificity();
                            spec[0] = spec[0].saturating_add(s[0]);
                            spec[1] = spec[1].saturating_add(s[1]);
                            spec[2] = spec[2].saturating_add(s[2]);
                        }
                    }
                    _ => spec[1] = spec[1].saturating_add(1),
                }
            }
        }

        spec
    }

    /// Checks that the provided element matches the current selector.
    pub fn matches<E: Element>(&self, element: &E) -> bool {
        assert!(!self.components.is_empty(), "selector must not be empty");
        assert_eq!(
            self.components[0].combinator,
            Combinator::None,
            "the first component must not have a combinator"
        );

        self.matches_impl(self.components.len() - 1, element)
    }

    fn matches_impl<E: Element>(&self, idx: usize, element: &E) -> bool {
        let component = &self.components[idx];

        if !match_selector(&component.selector, element) {
            return false;
        }

        match component.combinator {
            Combinator::Descendant => {
                let mut parent = element.parent_element();
                while let Some(e) = parent {
                    if self.matches_impl(idx - 1, &e) {
                        return true;
                    }

                    parent = e.parent_element();
                }

                false
            }
            Combinator::Child => {
                if let Some(parent) = element.parent_element() {
                    if self.matches_impl(idx - 1, &parent) {
                        return true;
                    }
                }

                false
            }
            Combinator::AdjacentSibling => {
                if let Some(prev) = element.prev_sibling_element() {
                    if self.matches_impl(idx - 1, &prev) {
                        return true;
                    }
                }

                false
            }
            // skia-canvas: general sibling `~` — any preceding sibling may match
            Combinator::GeneralSibling => {
                let mut prev = element.prev_sibling_element();
                while let Some(e) = prev {
                    if self.matches_impl(idx - 1, &e) {
                        return true;
                    }

                    prev = e.prev_sibling_element();
                }

                false
            }
            Combinator::None => true,
        }
    }
}

fn match_selector<E: Element>(selector: &SimpleSelector<'_>, element: &E) -> bool {
    if let SimpleSelectorType::Type(ident) = selector.kind {
        if !element.has_local_name(ident) {
            return false;
        }
    }

    for sub in &selector.subselectors {
        match sub {
            SubSelector::Attribute(name, operator) => {
                if !element.attribute_matches(name, *operator) {
                    return false;
                }
            }
            SubSelector::PseudoClass(class) => {
                if !match_pseudo_class(*class, element) {
                    return false;
                }
            }
        }
    }

    true
}

// skia-canvas: structural & negation pseudo-classes are purely positional, so the crate matches
// them generically here (via the Element sibling/name accessors). Only state-dependent pseudos
// (:hover/:focus/:target/… and unsupported ones) are delegated to the downstream impl — which,
// for a static renderer, returns false.
fn match_pseudo_class<E: Element>(class: PseudoClass<'_>, element: &E) -> bool {
    match class {
        PseudoClass::FirstChild => element.prev_sibling_element().is_none(),
        PseudoClass::LastChild => element.next_sibling_element().is_none(),
        PseudoClass::OnlyChild => {
            element.prev_sibling_element().is_none() && element.next_sibling_element().is_none()
        }
        PseudoClass::FirstOfType => !has_type_sibling(element, true),
        PseudoClass::LastOfType => !has_type_sibling(element, false),
        PseudoClass::OnlyOfType => {
            !has_type_sibling(element, true) && !has_type_sibling(element, false)
        }
        PseudoClass::NthChild(nth) => nth.matches(nth_index(element, true, false)),
        PseudoClass::NthLastChild(nth) => nth.matches(nth_index(element, false, false)),
        PseudoClass::NthOfType(nth) => nth.matches(nth_index(element, true, true)),
        PseudoClass::NthLastOfType(nth) => nth.matches(nth_index(element, false, true)),
        PseudoClass::Not(inner) => Selector::parse(inner).map_or(false, |s| !s.matches(element)),
        // state-dependent — only the downstream impl can decide (false under static rendering)
        PseudoClass::Link
        | PseudoClass::Visited
        | PseudoClass::Hover
        | PseudoClass::Active
        | PseudoClass::Focus
        | PseudoClass::Lang(_)
        | PseudoClass::Unsupported(_) => element.pseudo_class_matches(class),
    }
}

// skia-canvas: 1-based sibling index of `element`, counted from the start (`forward`) or the end,
// over all element siblings (`same_type == false`) or only those sharing its local name.
fn nth_index<E: Element>(element: &E, forward: bool, same_type: bool) -> i32 {
    let name = element.local_name();
    let mut i = 1;
    let mut sib = if forward {
        element.prev_sibling_element()
    } else {
        element.next_sibling_element()
    };
    while let Some(s) = sib {
        if !same_type || s.local_name() == name {
            i += 1;
        }
        sib = if forward {
            s.prev_sibling_element()
        } else {
            s.next_sibling_element()
        };
    }
    i
}

// skia-canvas: whether `element` has a sibling of the same local name before it (`forward`) or after.
fn has_type_sibling<E: Element>(element: &E, forward: bool) -> bool {
    let name = element.local_name();
    let mut sib = if forward {
        element.prev_sibling_element()
    } else {
        element.next_sibling_element()
    };
    while let Some(s) = sib {
        if s.local_name() == name {
            return true;
        }
        sib = if forward {
            s.prev_sibling_element()
        } else {
            s.next_sibling_element()
        };
    }
    false
}

pub(crate) fn parse(text: &str) -> (Option<Selector<'_>>, usize) {
    let mut components: Vec<Component<'_>> = Vec::new();
    let mut combinator = Combinator::None;

    let mut tokenizer = SelectorTokenizer::from(text);
    for token in &mut tokenizer {
        let mut add_sub = |sub| {
            if combinator == Combinator::None && !components.is_empty() {
                if let Some(ref mut component) = components.last_mut() {
                    component.selector.subselectors.push(sub);
                }
            } else {
                components.push(Component {
                    selector: SimpleSelector {
                        kind: SimpleSelectorType::Universal,
                        subselectors: vec![sub],
                    },
                    combinator,
                });

                combinator = Combinator::None;
            }
        };

        let token = match token {
            Ok(t) => t,
            Err(e) => {
                warn!("Selector parsing failed cause {}.", e);
                return (None, tokenizer.stream.pos());
            }
        };

        match token {
            SelectorToken::UniversalSelector => {
                components.push(Component {
                    selector: SimpleSelector {
                        kind: SimpleSelectorType::Universal,
                        subselectors: Vec::new(),
                    },
                    combinator,
                });

                combinator = Combinator::None;
            }
            SelectorToken::TypeSelector(ident) => {
                components.push(Component {
                    selector: SimpleSelector {
                        kind: SimpleSelectorType::Type(ident),
                        subselectors: Vec::new(),
                    },
                    combinator,
                });

                combinator = Combinator::None;
            }
            SelectorToken::ClassSelector(ident) => {
                add_sub(SubSelector::Attribute(
                    "class",
                    AttributeOperator::Contains(ident),
                ));
            }
            SelectorToken::IdSelector(id) => {
                add_sub(SubSelector::Attribute("id", AttributeOperator::Matches(id)));
            }
            SelectorToken::AttributeSelector(name, op) => {
                add_sub(SubSelector::Attribute(name, op));
            }
            SelectorToken::PseudoClass(ident) => {
                let class = match ident {
                    "first-child" => PseudoClass::FirstChild,
                    // skia-canvas: additional structural pseudo-classes
                    "last-child" => PseudoClass::LastChild,
                    "only-child" => PseudoClass::OnlyChild,
                    "first-of-type" => PseudoClass::FirstOfType,
                    "last-of-type" => PseudoClass::LastOfType,
                    "only-of-type" => PseudoClass::OnlyOfType,
                    "link" => PseudoClass::Link,
                    "visited" => PseudoClass::Visited,
                    "hover" => PseudoClass::Hover,
                    "active" => PseudoClass::Active,
                    "focus" => PseudoClass::Focus,
                    // skia-canvas: graceful skip — an unknown pseudo (e.g. :target) becomes a
                    // never-matching subselector so the rule and its grouped siblings survive,
                    // rather than dropping the whole selector as upstream did.
                    _ => {
                        warn!("':{}' is not supported; treated as never-matching.", ident);
                        PseudoClass::Unsupported(ident)
                    }
                };

                // TODO: duplicates
                // TODO: order

                add_sub(SubSelector::PseudoClass(class));
            }
            // skia-canvas: functional pseudo-classes (:nth-*(An+B), :not(...)). A malformed or
            // unmodeled one becomes Unsupported (never matches) instead of dropping the rule.
            SelectorToken::FunctionalPseudoClass(name, args) => {
                let class = match name {
                    "nth-child" => parse_nth(args).map(PseudoClass::NthChild),
                    "nth-last-child" => parse_nth(args).map(PseudoClass::NthLastChild),
                    "nth-of-type" => parse_nth(args).map(PseudoClass::NthOfType),
                    "nth-last-of-type" => parse_nth(args).map(PseudoClass::NthLastOfType),
                    "not" => parse_not(args),
                    _ => None,
                };
                add_sub(SubSelector::PseudoClass(
                    class.unwrap_or(PseudoClass::Unsupported(name)),
                ));
            }
            SelectorToken::LangPseudoClass(lang) => {
                add_sub(SubSelector::PseudoClass(PseudoClass::Lang(lang)));
            }
            SelectorToken::DescendantCombinator => {
                combinator = Combinator::Descendant;
            }
            SelectorToken::ChildCombinator => {
                combinator = Combinator::Child;
            }
            SelectorToken::AdjacentCombinator => {
                combinator = Combinator::AdjacentSibling;
            }
            // skia-canvas: general sibling `~`
            SelectorToken::SiblingCombinator => {
                combinator = Combinator::GeneralSibling;
            }
        }
    }

    if components.is_empty() {
        (None, tokenizer.stream.pos())
    } else if components[0].combinator != Combinator::None {
        debug_assert_eq!(
            components[0].combinator,
            Combinator::None,
            "the first component must not have a combinator"
        );

        (None, tokenizer.stream.pos())
    } else {
        (Some(Selector { components }), tokenizer.stream.pos())
    }
}

// skia-canvas: parse the `An+B` argument of a :nth-* pseudo into an `Nth`. Accepts `odd`,
// `even`, a bare integer `B`, and the `n`, `-n`, `An`, `An+B`, `An-B` forms with arbitrary
// internal whitespace. Returns None for anything malformed (→ graceful skip by the caller).
fn parse_nth(text: &str) -> Option<Nth> {
    let clean: String = text
        .bytes()
        .filter(|b| !b.is_ascii_whitespace())
        .map(|b| b.to_ascii_lowercase() as char)
        .collect();

    match clean.as_str() {
        "odd" => return Some(Nth { a: 2, b: 1 }),
        "even" => return Some(Nth { a: 2, b: 0 }),
        _ => {}
    }

    if let Some(n_pos) = clean.find('n') {
        let a = match &clean[..n_pos] {
            "" | "+" => 1,
            "-" => -1,
            a_str => a_str.parse::<i32>().ok()?,
        };
        let b_str = &clean[n_pos + 1..];
        let b = if b_str.is_empty() {
            0
        } else {
            b_str.parse::<i32>().ok()?
        };
        Some(Nth { a, b })
    } else {
        Some(Nth {
            a: 0,
            b: clean.parse::<i32>().ok()?,
        })
    }
}

// skia-canvas: validate the argument of :not(). We accept a single (possibly complex)
// selector and store its source slice for re-parsing at match time (keeping PseudoClass
// Copy). Comma-separated selector lists and empty/garbage arguments return None, so the
// caller treats the pseudo as never-matching rather than partially applying it.
fn parse_not(args: &str) -> Option<PseudoClass<'_>> {
    let trimmed = args.trim();
    if trimmed.is_empty() || trimmed.contains(',') {
        return None;
    }
    Selector::parse(trimmed).map(|_| PseudoClass::Not(trimmed))
}

impl fmt::Display for Selector<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for component in &self.components {
            match component.combinator {
                Combinator::Descendant => write!(f, " ")?,
                Combinator::Child => write!(f, " > ")?,
                Combinator::AdjacentSibling => write!(f, " + ")?,
                Combinator::GeneralSibling => write!(f, " ~ ")?,
                Combinator::None => {}
            }

            match component.selector.kind {
                SimpleSelectorType::Universal => write!(f, "*")?,
                SimpleSelectorType::Type(ident) => write!(f, "{}", ident)?,
            };

            for sel in &component.selector.subselectors {
                match sel {
                    SubSelector::Attribute(name, operator) => {
                        match operator {
                            AttributeOperator::Exists => {
                                write!(f, "[{}]", name)?;
                            }
                            AttributeOperator::Matches(value) => {
                                write!(f, "[{}='{}']", name, value)?;
                            }
                            AttributeOperator::Contains(value) => {
                                write!(f, "[{}~='{}']", name, value)?;
                            }
                            AttributeOperator::StartsWith(value) => {
                                write!(f, "[{}|='{}']", name, value)?;
                            }
                        };
                    }
                    SubSelector::PseudoClass(class) => write!(f, ":{}", class)?,
                }
            }
        }

        Ok(())
    }
}

/// A selector token.
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum SelectorToken<'a> {
    /// `*`
    UniversalSelector,

    /// `div`
    TypeSelector(&'a str),

    /// `.class`
    ClassSelector(&'a str),

    /// `#id`
    IdSelector(&'a str),

    /// `[color=red]`
    AttributeSelector(&'a str, AttributeOperator<'a>),

    /// `:first-child`
    PseudoClass(&'a str),

    /// `:lang(en)`
    LangPseudoClass(&'a str),

    /// skia-canvas: `:nth-child(2n+1)` — a functional pseudo-class and its raw argument
    FunctionalPseudoClass(&'a str, &'a str),

    /// `a b`
    DescendantCombinator,

    /// `a > b`
    ChildCombinator,

    /// `a + b`
    AdjacentCombinator,

    /// skia-canvas: `a ~ b`
    SiblingCombinator,
}

/// A selector tokenizer.
///
/// # Example
///
/// ```
/// use simplecss::{SelectorTokenizer, SelectorToken};
///
/// let mut t = SelectorTokenizer::from("div > p:first-child");
/// assert_eq!(t.next().unwrap().unwrap(), SelectorToken::TypeSelector("div"));
/// assert_eq!(t.next().unwrap().unwrap(), SelectorToken::ChildCombinator);
/// assert_eq!(t.next().unwrap().unwrap(), SelectorToken::TypeSelector("p"));
/// assert_eq!(t.next().unwrap().unwrap(), SelectorToken::PseudoClass("first-child"));
/// assert!(t.next().is_none());
/// ```
pub struct SelectorTokenizer<'a> {
    stream: Stream<'a>,
    after_combinator: bool,
    finished: bool,
}

impl<'a> From<&'a str> for SelectorTokenizer<'a> {
    fn from(text: &'a str) -> Self {
        SelectorTokenizer {
            stream: Stream::from(text),
            after_combinator: true,
            finished: false,
        }
    }
}

impl<'a> Iterator for SelectorTokenizer<'a> {
    type Item = Result<SelectorToken<'a>, Error>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished || self.stream.at_end() {
            if self.after_combinator {
                self.after_combinator = false;
                return Some(Err(Error::SelectorMissing));
            }

            return None;
        }

        macro_rules! try2 {
            ($e:expr) => {
                match $e {
                    Ok(v) => v,
                    Err(e) => {
                        self.finished = true;
                        return Some(Err(e));
                    }
                }
            };
        }

        match self.stream.curr_byte_unchecked() {
            b'*' => {
                if !self.after_combinator {
                    self.finished = true;
                    return Some(Err(Error::UnexpectedSelector));
                }

                self.after_combinator = false;
                self.stream.advance(1);
                Some(Ok(SelectorToken::UniversalSelector))
            }
            b'#' => {
                self.after_combinator = false;
                self.stream.advance(1);
                let ident = try2!(self.stream.consume_ident());
                Some(Ok(SelectorToken::IdSelector(ident)))
            }
            b'.' => {
                self.after_combinator = false;
                self.stream.advance(1);
                let ident = try2!(self.stream.consume_ident());
                Some(Ok(SelectorToken::ClassSelector(ident)))
            }
            b'[' => {
                self.after_combinator = false;
                self.stream.advance(1);
                let ident = try2!(self.stream.consume_ident());

                let op = match try2!(self.stream.curr_byte()) {
                    b']' => AttributeOperator::Exists,
                    b'=' => {
                        self.stream.advance(1);
                        let value = try2!(self.stream.consume_string());
                        AttributeOperator::Matches(value)
                    }
                    b'~' => {
                        self.stream.advance(1);
                        try2!(self.stream.consume_byte(b'='));
                        let value = try2!(self.stream.consume_string());
                        AttributeOperator::Contains(value)
                    }
                    b'|' => {
                        self.stream.advance(1);
                        try2!(self.stream.consume_byte(b'='));
                        let value = try2!(self.stream.consume_string());
                        AttributeOperator::StartsWith(value)
                    }
                    _ => {
                        self.finished = true;
                        return Some(Err(Error::InvalidAttributeSelector));
                    }
                };

                try2!(self.stream.consume_byte(b']'));

                Some(Ok(SelectorToken::AttributeSelector(ident, op)))
            }
            b':' => {
                self.after_combinator = false;
                self.stream.advance(1);
                let ident = try2!(self.stream.consume_ident());

                // skia-canvas: a `(...)` argument marks a functional pseudo-class. Consume it
                // here (upstream only special-cased :lang) so the argument can't leak into the
                // stream and corrupt later tokens. Scanned flat to the first `)`, which suffices
                // for An+B and the single, non-nested selectors we model inside :not().
                if self.stream.curr_byte() == Ok(b'(') {
                    self.stream.advance(1);
                    let args = self.stream.consume_bytes(|c| c != b')');
                    try2!(self.stream.consume_byte(b')'));

                    if ident == "lang" {
                        let lang = args.trim();
                        if lang.is_empty() {
                            self.finished = true;
                            return Some(Err(Error::InvalidLanguagePseudoClass));
                        }
                        return Some(Ok(SelectorToken::LangPseudoClass(lang)));
                    }

                    Some(Ok(SelectorToken::FunctionalPseudoClass(ident, args)))
                } else {
                    Some(Ok(SelectorToken::PseudoClass(ident)))
                }
            }
            b'>' => {
                if self.after_combinator {
                    self.after_combinator = false;
                    self.finished = true;
                    return Some(Err(Error::UnexpectedCombinator));
                }

                self.stream.advance(1);
                self.after_combinator = true;
                Some(Ok(SelectorToken::ChildCombinator))
            }
            b'+' => {
                if self.after_combinator {
                    self.after_combinator = false;
                    self.finished = true;
                    return Some(Err(Error::UnexpectedCombinator));
                }

                self.stream.advance(1);
                self.after_combinator = true;
                Some(Ok(SelectorToken::AdjacentCombinator))
            }
            // skia-canvas: general sibling `~`. Unambiguous at the top level: the `[a~=v]`
            // form is consumed inside the `[` branch, so a `~` reaching here is a combinator.
            b'~' => {
                if self.after_combinator {
                    self.after_combinator = false;
                    self.finished = true;
                    return Some(Err(Error::UnexpectedCombinator));
                }

                self.stream.advance(1);
                self.after_combinator = true;
                Some(Ok(SelectorToken::SiblingCombinator))
            }
            b' ' | b'\t' | b'\n' | b'\r' | b'\x0C' => {
                self.stream.skip_spaces();

                if self.after_combinator {
                    return self.next();
                }

                while self.stream.curr_byte() == Ok(b'/') {
                    try2!(self.stream.skip_comment());
                    self.stream.skip_spaces();
                }

                match self.stream.curr_byte() {
                    // skia-canvas: `~` added so `a ~ b` doesn't emit a spurious descendant
                    Ok(b'>') | Ok(b'+') | Ok(b'~') | Ok(b',') | Ok(b'{') | Err(_) => self.next(),
                    _ => {
                        if self.after_combinator {
                            self.after_combinator = false;
                            self.finished = true;
                            return Some(Err(Error::UnexpectedSelector));
                        }

                        self.after_combinator = true;
                        Some(Ok(SelectorToken::DescendantCombinator))
                    }
                }
            }
            b'/' => {
                if self.stream.next_byte() == Ok(b'*') {
                    try2!(self.stream.skip_comment());
                } else {
                    self.finished = true;
                }

                self.next()
            }
            b',' | b'{' => {
                self.finished = true;
                self.next()
            }
            _ => {
                let ident = try2!(self.stream.consume_ident());

                if !self.after_combinator {
                    self.finished = true;
                    return Some(Err(Error::UnexpectedSelector));
                }

                self.after_combinator = false;
                Some(Ok(SelectorToken::TypeSelector(ident)))
            }
        }
    }
}
