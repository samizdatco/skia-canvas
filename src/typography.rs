use std::iter::zip;
use std::sync::Once;
use std::collections::BTreeSet;
use serde_json::{json, Value};
use skia_safe::{FontMetrics, Paint, Point, Rect, Path as SkPath, PathBuilder, Font, GlyphId, TextBlob, TextBlobBuilder, Canvas as SkCanvas, Picture, PictureRecorder, dash_path_effect, path_utils::fill_path_with_paint};
use skia_safe::paint::{Style as PaintStyle, Cap as PaintCap};
use skia_safe::font::Edging;
use skia_safe::textlayout::{
  FontCollection, Paragraph, ParagraphBuilder, ParagraphStyle, RectHeightStyle, RectWidthStyle,
  TextAlign, TextDecorationStyle, TextDirection, TextStyle,
};
use read_fonts::{TableProvider, types::{Tag, Fixed, F2Dot14}, tables::{os2::SelectionFlags, mvar::tags::{HASC, HDSC}}};
use crate::font_library::{FontLibrary, RenderAttrs, cached_glyph_bounds};
use crate::bridge::*;
use crate::context::State;

//
// Text layout, metrics, and rendering
//

pub struct Typesetter{
  text: String,
  width: Option<f32>,
  text_align: TextAlign,
  typefaces: FontCollection,
  char_style: TextStyle,
  graf_style: ParagraphStyle,
  text_decoration: DecorationStyle,
  text_wrap: bool,
}

impl Typesetter{
  pub fn new(state:&State, text: &str, width:Option<f32>) -> Self {
    let (char_style, mut graf_style, text_decoration, text_wrap) = state.typography();

    // if width is undefined, typeset w/ left alignment then shift later based on actual alignment
    let text_align = graf_style.text_align();
    if width.is_none(){ graf_style.set_text_align(TextAlign::Left); }

    // normalize all whitespace as plain spaces (except line breaks in textWrap mode)
    let text = text.chars().map(|c| match c{
      '\t' | '\r' | '\x0c' | '\x0b' => ' ',
      '\n' | '\u{2028}' | '\u{2029}' if !text_wrap => ' ', // newline, line separator & paragraph separator
      c => c
    }).collect::<String>();

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

    static FONT_CHECK:Once = Once::new();
    FONT_CHECK.call_once(|| if FontLibrary::is_empty(){
      eprintln!("Warning: Cannot render text because no fonts are installed on this system.")
    });

    Typesetter{text, width, text_align, typefaces, char_style, graf_style, text_decoration, text_wrap}
  }

  // shape & line-break the text into a Paragraph (shared by `layout`, `metrics`, and `path`)
  fn shape_text(&self) -> Paragraph {
    let mut paragraph_builder = ParagraphBuilder::new(&self.graf_style, &self.typefaces);
    paragraph_builder.push_style(&self.char_style);
    paragraph_builder.add_text(&self.text);

    let mut paragraph = paragraph_builder.build();
    paragraph.layout(self.width.unwrap_or(100_000.0)); // make sure non-wrapped text fits in one line
    paragraph
  }

  // construct a `Layout` composed of text decorations (if any) and the shaped paragraph's sequence
  // of `GlyphRun`s, each containing the geometry, text, and font information needed to typeset a
  // contiguous run of characters (or generate a path outline of them).
  pub fn layout(&self, point:impl Into<Point>) -> Layout {
    let mut paragraph = self.shape_text();
    let line_info = line_info(&paragraph);
    let base = Point::new(self.alignment_offset(&paragraph, &line_info), 0.0) + point.into();
    let text = self.text.as_str();

    // collect GlyphRuns (character geometry) and run_clusters (start-indicies as utf-8 character offsets)
    let shift = self.char_style.baseline_shift(); // textBaseline's offset from the alphabetic baseline
    let line_origins = Self::line_origins(&line_info);
    let line_lefts:Vec<f32> = line_info.iter().map(|line| line.left as f32).collect();
    let mut left_edge = None;
    let mut runs:Vec<GlyphRun> = vec![];
    let mut run_clusters:Vec<Vec<u32>> = vec![];
    paragraph.visit(|line, visit|{
      let Some(info) = visit else { left_edge = None; return }; // end of line
      let x0 = left_edge.unwrap_or(line_lefts[line]); // inherit the starting edge
      let x1 = info.advance_x(); // the right edge of the run's layout box
      left_edge = Some(x1); // the next run will start where this one ends

      let count = info.count();
      if count == 0 { return }
      let origin = Point::new(
        info.origin().x + base.x,
        base.y + line_origins[line] + shift,
      );

      run_clusters.push(info.utf8_starts()[..count].to_vec()); // index relative to full `text` string
      runs.push(GlyphRun{
        font: info.font().clone(),
        origin,
        edges: (x0 + base.x, x1 + base.x),
        glyphs: info.glyphs().to_vec(),
        positions: info.positions().to_vec(),
        blob: None,
      });
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
    let mut paragraph = self.shape_text();
    let line_info = line_info(&paragraph);
    let origin = Point::new(self.alignment_offset(&paragraph, &line_info), 0.0);

    // calculate baseline offsets (relative to line_metrics.baseline which reflects ctx.textBaseline setting)
    let shift = self.char_style.baseline_shift();
    let relative = BaselineMetrics::for_style(&self.char_style).relative_to(shift);
    let hang = relative.get_offset(Baseline::Hanging);
    let norm = relative.get_offset(Baseline::Alphabetic);
    let ideo = relative.get_offset(Baseline::Ideographic);

    // calculate bounds for each single-font block of glyphs on each line (and gather font info)
    struct RunMetrics{ line: usize, family: String, font: FontMetrics, bounds: Rect }
    let mut text_runs:Vec<RunMetrics> = vec![];
    let line_origins = Self::line_origins(&line_info);
    paragraph.visit(|line, visit|{
      if let Some(info) = visit{
        // use SkParagraph's x, but not its y (since it's rounded to the nearest whole pixel)
        let run_origin = Point::new(info.origin().x, line_origins[line] + shift) + origin;
        text_runs.push(RunMetrics{
          line,
          family: info.font().typeface().family_name(),
          font: info.font().metrics().1,
          bounds: Self::ink_bounds(info.font(), info.glyphs(), info.positions(), run_origin),
        });
      }
    });

    // fill in the blank (or whitespace only) lines that visit() skips over
    if !self.text.is_empty(){
      for ln in 0..line_info.len(){
        if text_runs.iter().any(|run| run.line == ln){ continue }
        let start = paragraph.get_actual_text_range(ln, true).start; // byte offsets, not utf-16 indices
        let font = paragraph.get_font_at(start.min(self.text.len() - 1));
        let corner = origin + Point::new(line_info[ln].left as f32, line_origins[ln] + shift);
        text_runs.push(RunMetrics{
          line: ln,
          family: font.typeface().family_name(),
          font: font.metrics().1,
          bounds: Rect::from_xywh(corner.x, corner.y, 0.0, 0.0),
        });
      }
    }

    // measure each line: its glyph-ink bounds (the tight `actualBoundingBox*` edges) and its
    // advance-based layout rect (which feeds the top-level `width`), plus the per-line JSON
    struct LineMeasure{ ink: Rect, advance: Rect, json: Value }
    let lines = (0..paragraph.line_number()).filter_map(|ln|{
      // find the range of byte & char indices that are on this line (includes trailing whitespace if not wrapping)
      let text_range = paragraph.get_actual_text_range(ln, !self.text_wrap);
      let char_range = utf16_range(&self.text, &text_range);

      // calculate this line's vertical offsets relative to the typesetting origin
      let line_metrics = line_info.get(ln)?;
      let alpha = origin.y + line_origins.get(ln)? + shift; // the font's design origin (the alphabetic baseline unless BASE sets romn)
      let baseline = alpha - shift; // the textBaseline-selected origin
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
        "alphabeticBaseline": baseline - norm,
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
            "underline": font.underline_position().map(|pos| alpha + pos ),
            "strikethrough": font.strikeout_position().map(|pos| alpha + pos ),
          })
        }).collect::<Vec<Value>>()
      }) })
    }).collect::<Vec<LineMeasure>>();

    // use `advance_bounds` to set `width` & `ink_bounds` for the tight `actualBoundingBox*` edges
    let advance_bounds = lines.iter().map(|l| l.advance).reduce(Rect::join2).unwrap_or(Rect::new_empty());
    let ink_bounds = lines.iter().map(|l| l.ink).reduce(Rect::join2).unwrap_or(Rect::new_empty());
    let lines = lines.into_iter().map(|l| l.json).collect::<Vec<Value>>();

    // the font box & em square from the matched font, measured from the active textBaseline
    let BaselineMetrics{font: (ascent, descent), em: (em_ascent, em_descent), ..} = relative;

    json!({
      "width": advance_bounds.width(),
      "actualBoundingBoxLeft": -ink_bounds.left,
      "actualBoundingBoxRight": ink_bounds.right,
      "actualBoundingBoxAscent": -ink_bounds.top,
      "actualBoundingBoxDescent": ink_bounds.bottom,
      "fontBoundingBoxAscent": ascent,
      "fontBoundingBoxDescent": descent,
      "emHeightAscent": em_ascent,
      "emHeightDescent": em_descent,
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

  // get each line's precise glyph baseline (since SkParagraph's baselines are rounded to whole-pixel values)
  fn line_origins(lines:&[LineInfo]) -> Vec<f32>{
    lines.iter().scan(0.0, |top, lm|{ let y = *top; *top += lm.height as f32; Some(y) }).collect()
  }

  // bounds of all the actual glyph outlines (at Skia's canonical 64px size) in a run
  fn ink_bounds(font:&Font, glyphs:&[GlyphId], positions:&[Point], origin:Point) -> Rect {
    let (face, fake_bold) = (font.typeface().unique_id(), font.is_embolden());
    let size = if fake_bold { font.size() } else { 64.0 }; // fake bold is size-dependent
    let scale = font.size() / size;

    // get each glyph's outline bounds (empty for blank glyphs like spaces, None for glyphs without an outline)
    let outlines = cached_glyph_bounds(face, size, fake_bold, glyphs, |glyph| {
      let font = font.with_size(size)?;
      font.get_path(glyph).map(|path| path.compute_tight_bounds())
        .or_else(|| font.measure_text([glyph].as_slice(), None).1.is_empty().then(Rect::new_empty))
    });

    // use font's bounds rects for glyphs lacking outlines (like emoji and whitespace)
    zip(glyphs, zip(outlines, positions)).filter_map(|(glyph, (outline, pt))| {
      let rect = match outline{
        Some(b) => Rect::new(b.left * scale, b.top * scale, b.right * scale, b.bottom * scale),
        None => font.measure_text([*glyph].as_slice(), None).1,
      };
      if !rect.is_empty(){ Some(rect.with_offset(*pt + origin)) }else{ None } // blank glyphs (like spaces) have no ink
    }).reduce(Rect::join2).unwrap_or(Rect::new_empty())
  }

  // get the paragraph's left edge relative to the baseline origin
  fn alignment_offset(&self, paragraph:&Paragraph, lines:&[LineInfo]) -> f32{
    // convert start/end to left/right depending on writing system
    let gravity = match (self.graf_style.text_direction(), self.text_align){
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

    let letter_spacing = self.char_style.letter_spacing();
    match self.width{
      Some(box_width) => alignment_factor * box_width + spacing_step * letter_spacing,
      None => {
        // width-less lines are shaped with left-alignment so shift it based on its (unrounded) measured width
        let advance = if lines.first().is_some_and(|line| line.width > 0.0) { paragraph.longest_line() } else { 0.0 };
        alignment_factor * (advance - letter_spacing) - 0.5 * letter_spacing
      }
    }
  }
}

// a copy-able subset of LineMetrics (so the paragraph doesn't need to be borrowed)
struct LineInfo{ ascent:f64, descent:f64, height:f64, left:f64, width:f64 }

fn line_info(paragraph:&Paragraph) -> Vec<LineInfo>{
  paragraph.get_line_metrics().iter().map(|lm| LineInfo{
    ascent: lm.ascent, descent: lm.descent, height: lm.height, left: lm.left, width: lm.width,
  }).collect()
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
    flat.set_alpha_f(1.0).set_image_filter(None).set_blend_mode(skia_safe::BlendMode::SrcOver);
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
  edges: (f32, f32),      // left & right of the run's layout box
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
  antialias: bool,         // use the same smoothing rule as the text being underlined
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
      antialias: char_style.font_edging() != Edging::Alias,
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
    paint.set_anti_alias(self.antialias);
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
    let (x0, x1) = run.edges;
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
        self.append_rule(out, run, &Rule{x0, x1, pos,                     thickness, style, skip, graze_floor});
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

#[derive(Clone, Copy)]
pub struct BaselineMetrics{
  pub font: (f64, f64),  // font box ascent & descent (from `hhea` or `OS/2` if `USE_TYPO_METRICS`)
  pub em: (f64, f64),    // em square ascent & descent (from `OS/2`)
  pub romn: Option<f64>, // alphabetic offset (from `BASE`)
  pub hang: Option<f64>, // hanging offset (from `BASE`)
  pub ideo: Option<f64>, // ideographic offset (from `BASE`)
}

impl BaselineMetrics{
  pub fn for_style(style:&TextStyle) -> Self{
    let size = style.font_size() as f64;
    let typeface = style.typeface();
    let upm = typeface.as_ref().and_then(|face| face.units_per_em()).unwrap_or(0) as f64;
    let tables = &FontTables::metrics(typeface.as_ref());
    let px = |units:f64| units * size / upm;

    // variable fonts may have `hasc` and `hdsc` deltas that apply to their ascender & descender lengths
    let (hasc, hdsc) = (|| {
      let mvar = tables.mvar().ok()?;
      let instance = style.font_arguments()?.clone_typeface(typeface.clone()?)?;
      let position = instance.variation_design_position()?;
      let axes = tables.fvar().ok()?.axes().ok()?;
      let avar = tables.avar().ok(); // optional: without it, the normalized coordinates are used as-is
      let coords:Vec<F2Dot14> = axes.iter().enumerate().map(|(i, axis)| {
        let value = match position.iter().find(|coord| Tag::from_u32(*coord.axis) == axis.axis_tag()) {
          Some(coord) => Fixed::from_f64(coord.value as f64),
          None => axis.default_value(),
        };
        let coord = axis.normalize(value);
        match avar.as_ref().and_then(|avar| avar.axis_segment_maps().get(i)?.ok()) {
          Some(map) => map.apply(coord).to_f2dot14(),
          None => coord.to_f2dot14(),
        }
      }).collect();
      let delta = |tag| mvar.metric_delta(tag, &coords).map_or(0.0, |delta| delta.to_f64());
      Some((delta(HASC), delta(HDSC)))
    })().unwrap_or_default();

    // extract the em square and font box ascent/descent pairs
    let hhea = tables.hhea().ok().map(|hhea| (px(hhea.ascender().to_i16() as f64 + hasc), -px(hhea.descender().to_i16() as f64 + hdsc)));
    let os2 = tables.os2().ok().filter(|_| hhea.is_some()).map(|os2| {
      let typo = (px(os2.s_typo_ascender() as f64 + hasc), -px(os2.s_typo_descender() as f64 + hdsc));
      (typo, os2.fs_selection().contains(SelectionFlags::USE_TYPO_METRICS))
    });
    let typo = os2.map(|(typo, _)| typo);
    let font = match (hhea.filter(|&hhea| hhea != (0.0, 0.0)), os2) {
      (Some(_), Some((typo, true))) => typo,
      (Some(hhea), _) => hhea,
      (None, _) => {
        // compensate for the extra half-leading and baseline shift SkParagraph adds into these metrics
        let FontMetrics{ascent, descent, leading, ..} = style.font_metrics();
        let shift = style.baseline_shift();
        ((shift - ascent - leading / 2.0) as f64, (descent - shift - leading / 2.0) as f64)
      }
    };
    let (ascent, descent) = typo.filter(|(a, d)| *a > 0.0 && a + d > 0.0).unwrap_or(font);
    let em = if ascent + descent > 0.0 { (size * ascent / (ascent + descent), size * descent / (ascent + descent)) } else { (size, 0.0) };

    // extract any additional baseline offsets defined in the BASE table for the `DFLT` script
    let [romn, hang, ideo] = (|| {
      let axis = tables.base().ok()?.horiz_axis()?.ok()?;
      let (tags, scripts) = (axis.base_tag_list()?.ok()?, axis.base_script_list().ok()?);
      let values = scripts.base_script_records().iter()
        .find(|record| record.base_script_tag() == *b"DFLT")?
        .base_script(scripts.offset_data()).ok()?
        .base_values()?.ok()?;
      let coord = |tag:&[u8; 4]| {
        let index = tags.baseline_tags().iter().position(|t| t.get() == *tag)?;
        Some(px(values.base_coords().get(index).ok()?.coordinate() as f64))
      };
      Some([coord(b"romn"), coord(b"hang"), coord(b"ideo")])
    })().unwrap_or_default();

    BaselineMetrics{font, em, romn, hang, ideo}
  }

  // the same metrics measured from a baseline `shift` above the design origin (synthesized baselines are resolved first)
  pub fn relative_to(&self, shift:f32) -> Self{
    let shift = f64::from(shift);
    let offset = |baseline| Some(f64::from(self.get_offset(baseline)) - shift);
    BaselineMetrics{
      font: (self.font.0 - shift, self.font.1 + shift),
      em: (self.em.0 - shift, self.em.1 + shift),
      romn: offset(Baseline::Alphabetic),
      hang: offset(Baseline::Hanging),
      ideo: offset(Baseline::Ideographic),
    }
  }

  // the given baseline's offset above the alphabetic baseline
  pub fn get_offset(&self, baseline:Baseline) -> f32 {
    let BaselineMetrics{font: (ascent, descent), em: (em_ascent, em_descent), romn, hang, ideo} = *self;
    (match baseline {
      // from the em square
      Baseline::Top => em_ascent,
      Baseline::Middle => (em_ascent - em_descent) / 2.0,
      Baseline::Bottom => -em_descent,
      // from the font's declared baselines (w/ Chrome's formulas as fallbacks)
      Baseline::Alphabetic => romn.unwrap_or(0.0),
      Baseline::Hanging => hang.unwrap_or(ascent * 0.8),
      Baseline::Ideographic => ideo.unwrap_or(-descent),
    }) as f32
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
