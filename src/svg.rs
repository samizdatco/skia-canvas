use std::borrow::Cow;
use skia_safe::{Data, Picture, PictureRecorder, Rect, Size, svg::{self, Length, LengthUnit}};
use crate::font_library::FontLibrary;

// parse svg data and return it as a Picture along with its dimensions and an autosized flag
// (fit_view_box=true will use the viewBox dimensions verbatim rather than autosizing)
pub fn read_image(data:&Data, fit_view_box:bool) -> Option<(Picture, Size, bool)>{
  // resolve any <style> CSS first, since Skia's SVG DOM ignores it
  let mut dom = svg::Dom::try_from(StyledSvg::from_data(data)).ok()?;
  let root = dom.root();

  let mut size = root.intrinsic_size();
  let mut autosized = false;

  // handle files without explicit width/height dimensions
  if size.is_empty(){
    autosized = true;

    let Length{ value:width, unit:w_unit } = root.width();
    let Length{ value:height, unit:h_unit } = root.height();

    size = match root.view_box().filter(|_| fit_view_box){
      Some(view_box) => {
        // `loadCanvas` prefers the viewBox dimensions since there isn't an opportunity to resize on draw
        autosized = false;
        (view_box.width(), view_box.height()).into()
      }
      None => {
        // follow the CSS default sizing algorithm with a default object size of 300×150
        let axis = |value:&f32, unit:&LengthUnit| matches!(unit, LengthUnit::Number).then_some(*value);
        let (w, h) = (axis(width, w_unit), axis(height, h_unit));
        let ratio = root.view_box().map(|vb| vb.width() / vb.height());
        match ((w, h), ratio){
          // one concrete axis plus a ratio fully determines an intrinsic size
          ((Some(w), None), Some(r)) => { autosized = false; (w, w / r).into() }
          ((None, Some(h)), Some(r)) => { autosized = false; (h * r, h).into() }
          // otherwise at least one axis falls back to the default object size, so `autosized`
          // stays set and the draw/pattern calls scale the whole box to fit the canvas
          ((Some(w), None), None)    => (w, 150.0).into(),
          ((None, Some(h)), None)    => (300.0, h).into(),
          ((None, None),    Some(r)) => if r > 2.0 { (300.0, 300.0 / r) } else { (150.0 * r, 150.0) }.into(),
          _                          => (300.0, 150.0).into(),
        }
      }
    };
  };

  // record the SVG contents to a Picture
  let bounds = Rect::from_size(size);
  let mut recorder = PictureRecorder::new();
  dom.set_container_size(bounds.size());
  dom.render(recorder.begin_recording(bounds, true));
  let picture = recorder.finish_recording_as_picture(None)?;

  Some((picture, size, autosized))
}

//
// SVG <style> tag handling
//
// Skia's SVG DOM ignores <style> elements and CSS selectors (it honors only presentation
// attributes and inline style="…"), so we resolve the cascade ourselves and splice the winners
// back in as inline styles before handing the bytes to Skia. We resolve it fully rather than
// relying on Skia since, among other things, it discards any rule that has an `!important` keyword.
//
// Limitations: var() only uses 2-level scoping when looking up custom properties: it only looks in
// `:root` and in the element's own `style` attr (not through parent <g> groups, etc.). Also, chained
// definitions (`--a: var(--b)`) aren't supported. also, shorthand vs. longhand (font vs. font-size)
// don't cascade against each other.

struct StyledSvg<'a>(Cow<'a, [u8]>);

impl<'a> StyledSvg<'a> {
  fn from_data(data: &'a Data) -> Self {
    let svg = data.as_bytes();
    if !svg.windows(6).any(|w| w == b"<style") {
      return Self(Cow::Borrowed(svg)); // bail out early if there are no <style> tags
    }
    let Ok(text) = std::str::from_utf8(svg) else { return Self(Cow::Borrowed(svg)) };
    let Ok(doc) = roxmltree::Document::parse(text) else { return Self(Cow::Borrowed(svg)) };

    // gather CSS from every <style> element (regardless of the namespace being used)
    let mut css = String::new();
    for node in doc.descendants().filter(|n| n.tag_name().name() == "style") {
      for child in node.children() {
        if let Some(t) = child.text() { css.push_str(t); css.push('\n'); }
      }
    }
    let sheet = simplecss::StyleSheet::parse(&css);
    if sheet.rules.is_empty() {
      return Self(Cow::Borrowed(svg));
    }

    // for each element, resolve the full cascade to one keyword-stripped declaration per property,
    // merging in the element's own inline style (if any)
    let mut edits: Vec<Edit> = doc.descendants()
      .filter(|n| n.is_element() && n.tag_name().name() != "style")
      .filter_map(|node| {
        let el = StyleNode(node);
        let decls = sheet.resolve_inline(&el, el.style_value().unwrap_or(""));
        (!decls.is_empty()).then(|| el.style_edit(decls))
      })
      .collect();
    if edits.is_empty() {
      return Self(Cow::Borrowed(svg));
    }

    // apply the splices back-to-front so earlier offsets stay valid
    edits.sort_by_key(|e| e.0);
    let mut out = text.to_string();
    for (start, end, repl) in edits.into_iter().rev() {
      out.replace_range(start..end, &repl);
    }
    Self(Cow::Owned(out.into_bytes()))
  }
}

// build a Skia SVG DOM from the styled bytes or return Err(LoadError) if Skia can't parse it
impl<'a> TryFrom<StyledSvg<'a>> for svg::Dom {
  type Error = svg::LoadError;
  fn try_from(styled: StyledSvg<'a>) -> Result<Self, Self::Error> {
    svg::Dom::from_bytes(&styled.0, FontLibrary::with_shared(|lib| lib.font_mgr()))
  }
}


#[derive(Clone, Copy)]
struct StyleNode<'a, 'input>(roxmltree::Node<'a, 'input>); // a simplecss-compatible roxml node
type Edit = (usize, usize, String); // a single splice operation (start, end, replacement)

impl<'a, 'input> StyleNode<'a, 'input> {
  // the element's `style="…"` attribute, if it has one
  fn style_attr(&self) -> Option<roxmltree::Attribute<'a, 'input>> {
    self.0.attributes().find(|a| a.name() == "style")
  }

  // the element's existing inline style value, if it has one
  fn style_value(&self) -> Option<&str> {
    self.style_attr().map(|a| a.value())
  }

  // splice `decls` into an inline `style="…"` attribute, either the existing `style_attr` or a new one
  // placed after the node's last existing attribute. Coming last means it can override any presentation
  // attributes (fill="", stroke="", etc.) from earlier in the sequence.
  fn style_edit(&self, decls: String) -> Edit {
    match self.style_attr() {
      Some(style_attr) => {
        let r = style_attr.range_value();
        (r.start, r.end, decls)
      }
      None => {
        let at = self.0.attributes().next_back()
          .map(|a| a.range().end)
          .unwrap_or_else(|| self.tag_name_end());
        (at, at, format!(" style=\"{}\"", decls))
      }
    }
  }

  // byte offset just past `<tagname` in this element's start tag
  fn tag_name_end(&self) -> usize {
    let bytes = self.0.document().input_text().as_bytes();
    let mut i = self.0.range().start + 1; // skip '<'
    while i < bytes.len() && !matches!(bytes[i], b' '|b'\t'|b'\n'|b'\r'|b'/'|b'>') { i += 1; }
    i
  }
}

impl simplecss::Element for StyleNode<'_, '_> {
  fn parent_element(&self) -> Option<Self> {
    self.0.parent_element().map(StyleNode)
  }
  fn prev_sibling_element(&self) -> Option<Self> {
    self.0.prev_sibling_element().map(StyleNode)
  }
  fn next_sibling_element(&self) -> Option<Self> {
    self.0.next_sibling_element().map(StyleNode)
  }
  fn has_local_name(&self, name: &str) -> bool {
    self.0.tag_name().name() == name
  }
  fn local_name(&self) -> &str {
    self.0.tag_name().name()
  }
  fn attribute_matches(&self, local_name: &str, operator: simplecss::AttributeOperator) -> bool {
    self.0.attribute(local_name).is_some_and(|v| operator.matches(v))
  }
  fn pseudo_class_matches(&self, _class: simplecss::PseudoClass) -> bool {
    false // ignore :hover/:focus/:target/etc.
  }
}
