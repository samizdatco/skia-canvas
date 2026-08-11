#![allow(non_snake_case)]
use std::iter::zip;
use std::collections::BTreeSet;
use serde_json::{json, Value};
use skia_safe::{FontMetrics, Paint, Point, Rect, Path as SkPath, PathBuilder, Font, GlyphId, TextBlob, TextBlobBuilder, Canvas as SkCanvas, Picture, PictureRecorder, dash_path_effect, path_utils::fill_path_with_paint};
use skia_safe::paint::{Style as PaintStyle, Cap as PaintCap};
use skia_safe::textlayout::{
  FontCollection, Paragraph, ParagraphBuilder, ParagraphStyle, RectHeightStyle, RectWidthStyle,
  TextAlign, TextDecorationStyle, TextDirection, TextStyle,
};
use crate::font_library::{FontLibrary, RenderAttrs};
use crate::bridge::*;
use crate::context::State;

//
// Text layout, metrics, and rendering
//

pub struct Typesetter{
  text: String,
  width: f32,
  typefaces: FontCollection,
  char_style: TextStyle,
  graf_style: ParagraphStyle,
  text_decoration: DecorationStyle,
  text_wrap: bool,
}

impl Typesetter{
  pub fn new(state:&State, text: &str, width:Option<f32>) -> Self {
    let (char_style, graf_style, text_decoration, text_wrap) = state.typography();

    let typefaces = FontLibrary::with_shared(|lib|
      lib
        .set_render_attrs(RenderAttrs{
          hinting: char_style.font_hinting(),
          edging: char_style.font_edging(),
          subpixel: char_style.subpixel(),
          synthesize: graf_style.fake_missing_font_styles(),
        })
        .font_collection()
    );
    let width = width.unwrap_or(100_000.0); // if not wrapping, pick an effectively infinite width
    let text = match text_wrap{
      true => text.to_string(),
      false => text.replace("\n", " ")
    };

    Typesetter{text, width, typefaces, char_style, graf_style, text_decoration, text_wrap}
  }

  // shape & line-break the text into a Paragraph (shared by `layout`, `metrics`, and `path`) and
  // provide a y-axis offset from the requested origin to the alphabetic baseline
  fn shape_text(&self) -> (Paragraph, Point) {
    let mut char_style = self.char_style.clone();

    // set the `wght` coordinate so variable fonts will instance at the correct weight
    // (non-variable fonts will just ignore the setting)
    {
      use skia_safe::FontArguments;
      use skia_safe::font_arguments::{VariationPosition, variation_position::Coordinate};
      let weight = *self.char_style.font_style().weight() as f32;
      let coords = [ Coordinate { axis: Coordinate::wght, value: weight } ];
      let args = FontArguments::new()
        .set_variation_design_position(VariationPosition { coordinates: &coords });
      char_style.set_font_arguments(&args);
    }

    let mut paragraph_builder = ParagraphBuilder::new(&self.graf_style, &self.typefaces);
    paragraph_builder.push_style(&char_style);
    paragraph_builder.add_text(&self.text);

    let mut paragraph = paragraph_builder.build();
    paragraph.layout(self.width);

    let offset = Point::new(
      self.alignment_offset(),
      -paragraph.alphabetic_baseline(),
    );

    (paragraph, offset)
  }

  // construct a `Layout` composed of text decorations (if any) and the shaped paragraph's sequence
  // of `GlyphRun`s, each containing the geometry, text, and font information needed to typeset a
  // contiguous run of characters (or generate a path outline of them).
  pub fn layout(&self, point:impl Into<Point>) -> Layout {
    let (mut paragraph, offset) = self.shape_text();
    let base = offset + point.into();
    let text = self.text.as_str();
    let shift = self.char_style.baseline_shift(); // `textBaseline`'s offset from the alphabetic baseline

    // collect GlyphRuns (character geometry) and run_clusters (start-indicies as utf-8 character offsets)
    let mut runs:Vec<GlyphRun> = vec![];
    let mut run_clusters:Vec<Vec<u32>> = vec![];
    paragraph.extended_visit(|_line, visit|{
      if let Some(info) = visit {
        let count = info.glyphs().len();
        if count == 0 { return }
        let origin = Point::new(
          info.origin().x + base.x,
          base.y + info.origin().y + shift, // use the subpixel position (don't snap-to-integer like Paragraph.paint())
        );

        run_clusters.push(info.utf8_starts()[..count].to_vec()); // index relative to full `text` string
        runs.push(GlyphRun{
          font: info.font().clone(),
          origin,
          advance: info.advance().width,
          glyphs: info.glyphs().to_vec(),
          positions: info.positions().to_vec(),
          blob: None,
        });
      }
    });

    // find the utf-8 offset range that corresponds to each run. `run_clusters` counts in glyph order
    // (either LTR or RTL) while `bounds` is sorted (so we can easily find the 'next' cluster)
    let mut bounds:BTreeSet<u32> = run_clusters.iter().flatten().copied().collect();
    bounds.insert(text.len() as u32);
    let ranges:Vec<_> = run_clusters.iter_mut().map(|starts|{
      let lo = starts.iter().copied().min().unwrap_or(0);
      let hi = starts.iter().copied().max()
        .and_then(|furthest| bounds.range(furthest+1..).next().copied())
        .unwrap_or(lo);
      for c in starts.iter_mut() { *c -= lo; } // rebase indices into slice-local coords
      lo as usize .. hi as usize
    }).collect();

    // create a TextBlob for each run (baking in its utf-8 text so it can be selectable in PDFs)
    for ((run, clusters), span) in runs.iter_mut().zip(&run_clusters).zip(&ranges) {
      let mut builder = TextBlobBuilder::new();
      let n = run.glyphs.len();
      let slice = text.get(span.clone())
        .expect("run text slice is outside source string (only reachable via ellipsis truncation)");

      let (glyphs, pos, txt, cl) = builder.alloc_run_text_pos(&run.font, n, slice.len(), None);
      glyphs.copy_from_slice(&run.glyphs);
      pos.copy_from_slice(&run.positions);
      txt.copy_from_slice(slice.as_bytes());
      cl.copy_from_slice(clusters);
      run.blob = builder.make();
    }

    let decorations = Decorations::from_style(&self.text_decoration, &self.char_style);
    Layout{ runs, decorations }
  }

  pub fn metrics(&self) -> Value {
    let (mut paragraph, origin) = self.shape_text();

    // calculate baseline offsets (relative to line_metrics.baseline which reflects ctx.textBaseline setting)
    let shift = self.char_style.baseline_shift();
    let hang = Baseline::Hanging.get_offset(&self.char_style) - shift;
    let norm = Baseline::Alphabetic.get_offset(&self.char_style) - shift;
    let ideo = Baseline::Ideographic.get_offset(&self.char_style) - shift;

    // calculate bounds for each single-font block of glyphs on each line (and gather font info)
    struct RunMetrics{ line: usize, family: String, font: FontMetrics, bounds: Rect }
    let mut text_runs:Vec<RunMetrics> = vec![];
    paragraph.extended_visit(|line, visit|{
      if let Some(info) = visit{
        text_runs.push(RunMetrics{
          line,
          family: info.font().typeface().family_name(),
          font: info.font().metrics().1,
          bounds: zip(info.positions(), info.bounds())
            .filter(|(_, rect)| !rect.is_empty())
            .map(|(pt, rect)| rect.with_offset(*pt + info.origin() + origin - Point::new(0.0, norm)))
            .reduce(Rect::join2)
            .unwrap_or(Rect::new_empty())
        });
      }
    });

    // measure each line: its glyph-ink bounds (the tight `actualBoundingBox*` edges) and its
    // advance-based layout rect (which feeds the top-level `width`), plus the per-line JSON
    struct LineMeasure{ ink: Rect, advance: Rect, json: Value }
    let lines = (0..paragraph.line_number()).filter_map(|ln|{
      // find the range of byte & char indices that are on this line (includes trailing whitespace if not wrapping)
      let text_range = paragraph.get_actual_text_range(ln, !self.text_wrap);
      let char_range = utf16_range(&self.text, &text_range);

      // calculate this line's vertical offsets relative to the typesetting origin
      let line_metrics = paragraph.get_line_metrics_at(ln)?;
      let half_leading = self.graf_style.strut_style().leading().max(0.0) * self.char_style.font_size() / 2.0;
      let baseline = line_metrics.baseline as f32 + origin.y - half_leading; // the textBaseline-selected origin
      let alpha = baseline - norm; // the line's alphabetic baseline (the font-metric origin)
      let line_ascent = baseline - line_metrics.ascent as f32;
      let line_descent = baseline + line_metrics.descent as f32;

      // combine the glyph bounds of all single-font runs on this line (potentially omitting trailing spaces)
      let font_runs = text_runs.iter().filter(|r| r.line==ln).collect::<Vec<&RunMetrics>>();
      let ink = font_runs.iter()
        .map(|run| run.bounds)
        .reduce(Rect::join2)
        .unwrap_or(Rect::new_empty());

      // the advance-based layout rect gives the top-level `width`; keep its full advance
      // (including any trailing letter-space) so `width` matches Chrome/Safari
      let advance = paragraph
        .get_rects_for_range(char_range.clone(), RectHeightStyle::Tight, RectWidthStyle::Tight).iter()
        .map(|tb| {
          let Rect{top, bottom, ..} = ink;
          let Rect{left, right, ..} = tb.rect.with_offset(origin);
          Rect::new(left, top, right, bottom)
        })
        .reduce(Rect::join2)
        .unwrap_or(ink);

      Some(LineMeasure{ ink, advance, json: json!({
        "x": ink.left,
        "y": ink.top,
        "width": ink.width(),
        "height": ink.height(),
        "baseline": baseline, // corresponds to the ctx.textBaseline selection
        "hangingBaseline": baseline - hang,
        "alphabeticBaseline": alpha,
        "ideographicBaseline": baseline - ideo,
        "ascent": line_ascent,
        "descent": line_descent,
        "startIndex": char_range.start,
        "endIndex": char_range.end,
        "runs": font_runs.iter().map(|RunMetrics{family, font, bounds, ..}| {
          json!({
            "x": bounds.left,
            "y": bounds.top,
            "width": bounds.width(),
            "height": bounds.height(),
            "family": family,
            "ascent": alpha + font.ascent,
            "descent": alpha + font.descent,
            "capHeight": alpha - font.cap_height,
            "xHeight": alpha - font.x_height,
            "underline": font.underline_position().map(|ulH| alpha + ulH ),
            "strikethrough": font.strikeout_position().map(|stH| alpha + stH ),
          })
        }).collect::<Vec<Value>>()
      }) })
    }).collect::<Vec<LineMeasure>>();

    // use `advance_bounds` to set `width` & `ink_bounds` for the tight `actualBoundingBox*` edges
    let advance_bounds = lines.iter().map(|l| l.advance).reduce(Rect::join2).unwrap_or(Rect::new_empty());
    let ink_bounds = lines.iter().map(|l| l.ink).reduce(Rect::join2).unwrap_or(Rect::new_empty());
    let lines = lines.into_iter().map(|l| l.json).collect::<Vec<Value>>();

    // use line metrics to find maximal ascent/descent of all fonts on first line
    let (ascent, descent) = paragraph.get_line_metrics_at(0).map(|line|
      (norm + line.ascent as f32, line.descent as f32 - norm)
    ).unwrap_or_else(||{
      // or fall back to the first-matched font's metrics if measuring empty string
      let FontMetrics{ascent, descent, ..} = self.char_style.font_metrics();
      (norm - ascent, descent - norm)
    });

    json!({
      "width": advance_bounds.width(),
      "actualBoundingBoxLeft": -ink_bounds.left,
      "actualBoundingBoxRight": ink_bounds.right,
      "actualBoundingBoxAscent": -ink_bounds.top,
      "actualBoundingBoxDescent": ink_bounds.bottom,
      "fontBoundingBoxAscent": ascent,
      "fontBoundingBoxDescent": descent,
      "emHeightAscent": ascent,
      "emHeightDescent": descent,
      "hangingBaseline": hang,
      "alphabeticBaseline": norm,
      "ideographicBaseline": ideo,
      "lines": lines,
    })
  }

  // outline the text as a single path
  pub fn path(&self, point:impl Into<Point>) -> SkPath {
    self.layout(point).to_path()
  }

  fn alignment_offset(&self) -> f32{
    // convert start/end to left/right depending on writing system
    let gravity = match (self.graf_style.text_direction(), self.graf_style.text_align()){
      (TextDirection::LTR, TextAlign::Start) | (TextDirection::RTL, TextAlign::End) => TextAlign::Left,
      (TextDirection::LTR, TextAlign::End) | (TextDirection::RTL, TextAlign::Start) => TextAlign::Right,
      (_, alignment) => alignment,
    };

    // `alignment_factor` shifts the entire line to left/right/center align it
    // `spacing_step` compensates for the letterspacing Paragraph adds before the line's first character
    let (alignment_factor, spacing_step) = match gravity{
      TextAlign::Left | TextAlign::Justify => (0.0, -0.5),
      TextAlign::Center => (-0.5, 0.5),
      TextAlign::Right => (-1.0, 1.0),
      _ => (0.0, 0.0) // start & end have already been remapped
    };

    alignment_factor * self.width + spacing_step * self.char_style.letter_spacing()
  }
}

pub struct Layout{
  runs: Vec<GlyphRun>,
  decorations: Option<Decorations>, // None when no line (underline/overline/line-through) is set
}

impl Layout{
  pub fn draw(&self, canvas:&SkCanvas, paint:&Paint){
    let should_overprint = self.decorations.as_ref()
      .is_some_and(|deco| matches!(deco.line.kind, DecorationKind::LineThrough));

    if should_overprint { // line-through
      self.draw_glyphs(canvas, paint);
      self.draw_decorations(canvas, paint);
    }else{ // underline & overline
      self.draw_decorations(canvas, paint);
      self.draw_glyphs(canvas, paint);
    }
  }

  fn draw_decorations(&self, canvas:&SkCanvas, paint:&Paint){
    if let Some(deco) = &self.decorations {
      deco.draw(canvas, &self.runs, paint);
    }
  }

  pub fn draw_glyphs(&self, canvas:&SkCanvas, paint:&Paint){
    self.runs.iter().for_each(|run| run.draw(canvas, paint));
  }

  pub fn to_path(&self) -> SkPath {
    let mut path = PathBuilder::new();
    self.runs.iter().for_each(|run| run.outline(&mut path));
    path.detach()
  }

  // flag whether there are multiple glyph runs or a decoration (implying to_picture should be used for render)
  pub fn is_multi_op(&self) -> bool {
    self.runs.len() > 1 || self.decorations.is_some()
  }

  // flatten the draw into a Picture, condensing multiple ops into a single draw_picture
  pub fn to_picture(&self, paint:&Paint) -> Option<Picture> {
    // Record with a flattened paint (alpha 1, no image filter) so the outer paint applies
    // globalAlpha / blend / drop-shadow exactly once.
    let mut flat = paint.clone();
    flat.set_alpha_f(1.0).set_image_filter(None);
    let unbounded = Rect::new(f32::MIN, f32::MIN, f32::MAX, f32::MAX);
    let mut recorder = PictureRecorder::new();
    let canvas = recorder.begin_recording(unbounded, true);
    self.draw(canvas, &flat);
    recorder.finish_recording_as_picture(None)
  }
}

// a run of contiguous characters sharing the same font from a single line of the paragraph
pub struct GlyphRun{
  font: Font,             // used for converting glyphs to paths and its font_metrics field
  origin: Point,          // the baseline-left paragraph origin
  advance: f32,           // full horizontal extent (for decoration line length)
  glyphs: Vec<GlyphId>,   // per-glyph outline IDs
  positions: Vec<Point>,  // per-glyph x-positions
  blob: Option<TextBlob>, // drawable blob (including text for selectable PDF)
}

impl GlyphRun{
  // draw the run's blob at its baseline origin
  fn draw(&self, canvas:&SkCanvas, paint:&Paint){
    if let Some(blob) = self.blob.as_ref() { canvas.draw_text_blob(blob, self.origin, paint); }
  }

  // append the run's glyph outlines to a mutable path (positioned at the run origin)
  fn outline(&self, out:&mut PathBuilder){
    for (glyph, pos) in self.glyphs.iter().zip(&self.positions) {
      if let Some(glyph_path) = self.font.get_path(*glyph) {
        out.add_path_with_offset(&glyph_path, self.origin + *pos, None);
      }
    }
  }

  // find locations where a glyph's descender breaks into the vertical zone occupied by the text-decoration.
  // to be a true gap, the descender has to go all the way through the decoration (past its `graze_floor`)
  fn descender_gaps(&self, rule:&Rule) -> Vec<(f32,f32)>{
    let Some(blob) = self.blob.as_ref() else { return vec![] };
    let top = (rule.pos - rule.thickness/2.0).max(rule.graze_floor);
    let bottom = rule.pos + rule.thickness/2.0;
    if bottom <= top { return vec![] }
    let mut probe = Paint::default();
    probe.set_style(PaintStyle::Stroke).set_stroke_width(rule.thickness).set_anti_alias(true);
    blob.get_intercepts([top, bottom], Some(&probe))
      .chunks_exact(2)
      .map(|c| (self.origin.x + c[0], self.origin.x + c[1]))
      .collect()
  }
}

// The underline/overline/line-through renderer for a particular `Layout`
struct Decorations{
  line: DecorationLine,    // the active line (kind + style); Decorations exists only when there is one
  size: Option<Spacing>,   // explicit thickness override, else the font metric
  color: Option<CssColor>, // explicit color, else `currentColor` (the fill color)
  font_size: f32,          // for the default `fontSize/14` thickness and dash/wave scaling
}

// each distinct line gets a Rule record (i.e., normally there's one but double-underline has two)
struct Rule{ x0:f32, x1:f32, pos:f32, thickness:f32, style:TextDecorationStyle, skip:bool, graze_floor:f32 }

impl Decorations{
  // return a configured `Decorations` for the reqeusted style (or None if decorations are off)
  fn from_style(style:&DecorationStyle, char_style:&TextStyle) -> Option<Self>{
    style.line.clone().map(|line| Decorations{
      line,
      size: style.size.clone(),
      color: style.color,
      font_size: char_style.font_size(),
    })
  }

  // walk the runs and collect their decorations into a single fill path
  fn draw(&self, canvas:&SkCanvas, runs:&[GlyphRun], paint:&Paint){
    let base = self.fill_paint(paint);
    let mut deco_path = PathBuilder::new();
    for run in runs {
      self.trace_run(&mut deco_path, run);
    }
    let deco_path = deco_path.detach();
    if !deco_path.is_empty() {
      canvas.draw_path(&deco_path, &base);
    }
  }

  // inherit the current fill color unless a decoration color was explicitly specified
  fn fill_paint(&self, base:&Paint) -> Paint {
    let mut paint = base.clone();
    paint.set_path_effect(None);
    paint.set_anti_alias(true);
    paint.set_style(PaintStyle::Fill);
    if let Some(css) = self.color {
      paint.set_shader(None);
      paint.set_color(color4f_to_color(css.color));
    }
    paint
  }

  // add the underline/overline/line-through geometry for a single run to the mutable path
  fn trace_run(&self, out:&mut PathBuilder, run:&GlyphRun){
    let (_, metrics) = run.font.metrics();
    let x0 = run.origin.x;
    let x1 = run.origin.x + run.advance;
    let (thick_metric, pos) = self.line.kind.placement(&metrics);
    let graze_floor = pos; // check for descenders reaching the (topmost) underline's position
    let thickness = self.size.as_ref()
      .map(|s| s.in_px(self.font_size))
      .unwrap_or_else(|| thick_metric.unwrap_or(self.font_size / 14.0))
      .max(1.0);
    let skip = run.blob.is_some() && self.line.kind == DecorationKind::Underline; // only add descender gaps for underlines

    match self.line.style {
      TextDecorationStyle::Double => {
        // draw double-underscores with a space between them equal to the stroke thickness and calculate gaps independently
        let style = TextDecorationStyle::Solid;
        self.append_rule(out, run, &Rule{x0, x1, pos:pos,                 thickness, style, skip, graze_floor});
        self.append_rule(out, run, &Rule{x0, x1, pos:pos + thickness*2.0, thickness, style, skip, graze_floor});
      }
      style => self.append_rule(out, run, &Rule{ x0, x1, pos, thickness, style, skip, graze_floor }),
    }
  }

  // add a single decoration line to the mutable path (potentially with gaps to dodge descenders)
  fn append_rule(&self, out:&mut PathBuilder, run:&GlyphRun, rule:&Rule){
    let yc = run.origin.y + rule.pos;
    let segments = match rule.skip && run.blob.is_some() {
      true => {
        // when leaving gaps for descenders, add a 'halo' on either side horizontally
        let halo = rule.thickness;
        let mut segs = vec![];
        let mut start = rule.x0;
        for (a, b) in run.descender_gaps(rule) {
          let end = a - halo;
          if end - start >= halo { segs.push((start, end)) } // only keep if segment width >= halo
          start = start.max(b + halo);
        }
        if rule.x1 - start > halo { segs.push((start, rule.x1)) }
        segs
      }
      _ => vec![(rule.x0, rule.x1)],
    };
    for (sx, ex) in segments {
      self.append_rule_segment(out, sx, ex, yc, rule.thickness, rule.style);
    }
  }

  // add one segment of a decoration rule (as a *fill* contour) to the mutable path
  fn append_rule_segment(&self, out:&mut PathBuilder, x0:f32, x1:f32, yc:f32, t:f32, style:TextDecorationStyle){
    use TextDecorationStyle::*;
    if x1 - x0 <= 0.0 { return }
    let mut stroke_path = PathBuilder::new();

    match style {
      Solid | Double => {
        out.add_rect(Rect::new(x0, yc - t/2.0, x1, yc + t/2.0), None, None);
      }
      Dotted | Dashed => {
        // use a dash_path_effect with spacing proportional to font size
        let s = (self.font_size / 14.0).max(1.0);
        let (intervals, cap) = if matches!(style, Dotted){ ([s, 1.5*s], PaintCap::Round) }else{ ([4.0*s, 2.0*s], PaintCap::Butt) };
        let mut p = Paint::default();
        p.set_style(PaintStyle::Stroke).set_stroke_width(t).set_stroke_cap(cap);
        p.set_path_effect(dash_path_effect::new(&intervals, 0.0));
        let mut line = PathBuilder::new();
        line.move_to((x0, yc)); line.line_to((x1, yc));
        if fill_path_with_paint(&line.detach(), &p, &mut stroke_path, None, None) {
          out.add_path(&stroke_path.detach(), skia_safe::path::AddPathMode::Append);
        }
      }
      Wavy => {
        // draw a rounded-off zig-zag
        let step = (t * 2.0).max(2.0); // wavelength = 400% of line-thickness
        let ctrl = t * 1.4;            // amplitude = ±70% of line-thickness
        let mut wave = PathBuilder::new();
        wave.move_to((x0, yc));
        let mut x = x0;
        let mut up = true;
        while x < x1 {
          let nx = (x + step).min(x1);
          let cx = (x + nx) / 2.0;
          let cy = if up { yc - ctrl } else { yc + ctrl };
          wave.quad_to((cx, cy), (nx, yc));
          x = nx;
          up = !up;
        }
        let mut p = Paint::default();
        p.set_style(PaintStyle::Stroke).set_stroke_width(t);
        if fill_path_with_paint(&wave.detach(), &p, &mut stroke_path, None, None) {
          out.add_path(&stroke_path.detach(), skia_safe::path::AddPathMode::Append);
        }
      }
    }
  }
}

#[derive(Copy, Clone, Debug)]
pub enum Baseline{ Top, Hanging, Middle, Alphabetic, Ideographic, Bottom }

impl Baseline{
  pub fn get_offset(&self, style:&TextStyle) -> f32 {
    let FontMetrics{mut ascent, mut descent, ..} = style.font_metrics();
    ascent -= style.baseline_shift();  // offsets are defined relative to the alphabetic baseline, so
    descent -= style.baseline_shift(); // compensate for any other textBaseline setting

    // see TextMetrics::GetFontBaseline from Chromium for reference:
    // https://github.com/chromium/chromium/blob/main/third_party/blink/renderer/core/html/canvas/text_metrics.cc#L34
    match self {
      Baseline::Top => -ascent,
      Baseline::Hanging => -ascent * 0.8,
      Baseline::Middle => -(ascent + descent) / 2.0,
      Baseline::Alphabetic => 0.0,
      Baseline::Bottom | Baseline::Ideographic => -descent,
    }
  }
}

// the kind of line a text-decoration draws (and where it sits relative to the baseline)
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum DecorationKind{ Underline, Overline, LineThrough }

impl DecorationKind{
  // this line's (thickness metric, baseline-relative position) from the run's font metrics
  fn placement(&self, m:&FontMetrics) -> (Option<f32>, f32){
    match self{
      DecorationKind::Underline   => (m.underline_thickness(), m.underline_position().unwrap_or(0.0)),
      DecorationKind::Overline    => (m.underline_thickness(), m.ascent),
      DecorationKind::LineThrough => (m.strikeout_thickness(), m.strikeout_position().unwrap_or(-m.x_height/2.0)),
    }
  }
}

// the selected combination of line position (kind) and stroke/shape (style)
#[derive(Clone, Debug)]
pub(crate) struct DecorationLine{ pub(crate) kind: DecorationKind, pub(crate) style: TextDecorationStyle }

#[derive(Clone, Debug)]
pub struct DecorationStyle{
  pub css: String,
  pub(crate) line: Option<DecorationLine>, // None if no decoration is active
  pub(crate) size: Option<Spacing>,
  pub(crate) color: Option<CssColor>,
}

impl Default for DecorationStyle{
  fn default() -> Self {
    Self{ css:"none".to_string(), line:None, size:None, color:None }
  }
}
