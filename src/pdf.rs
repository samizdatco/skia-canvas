//
// PDF parsing: convert a Hayro content stream into Skia ops recorded into a Picture
//
use std::sync::Arc;
use std::rc::Rc;
use std::cell::RefCell;
use std::collections::HashMap;
use neon::{prelude::*, types::buffer::TypedArray};
use skia_safe::{
  AlphaType, BlendMode as SkBlendMode, Canvas as SkCanvas, Color4f, ColorSpace, ColorType, Data,
  FilterMode, Font as SkFont, FontHinting, FontMgr, ImageInfo, Matrix, Paint as SkPaint, PaintCap,
  PaintJoin, PaintStyle, Path as SkPath, PathBuilder, PathFillType, Picture, PictureRecorder,
  Point as SkPoint, Rect as SkRect, SamplingOptions, Shader, Size, TextBlobBuilder, TileMode,
  Typeface, canvas::SaveLayerRec, dash_path_effect, image::images, luma_color_filter,
  gradient::{self, Colors as GradientColors, Interpolation},
};
use kurbo::{Affine, BezPath, Cap, Join, PathEl, Shape};
use smallvec::smallvec;
use hayro_interpret::{
  BlendMode, CacheKey, ClipPath, Device, FillRule, GlyphDrawMode, Image, ImageData,
  InterpreterCache, InterpreterSettings, LumaData, MaskType, Paint, PathDrawMode, SoftMask,
  StrokeProps, interpret_page,
  Context as PdfContext,
  color::AlphaColor,
  font::{Glyph, OutlineGlyph},
  hayro_syntax::{Pdf, page::Page as PdfPage},
  pattern::{Pattern, ShadingPattern, TilingPattern},
  shading::{ShadingFunction, ShadingType},
  util::TransformExt,
};
use crate::context::BoxedContext2D;

// the header may at any newline in the opening bytes of the file (so check the first 1kb)
pub fn is_pdf(data:&[u8]) -> bool{
  let header = &data[..data.len().min(1024)];
  header.windows(5).enumerate().any(|(i, window)|
    window == b"%PDF-" && (i == 0 || matches!(header[i - 1], b'\n' | b'\r'))
  )
}

// let hayro's Pdf share the skia Data instead of copying the file contents
struct PdfBytes(Data);
impl AsRef<[u8]> for PdfBytes{
  fn as_ref(&self) -> &[u8]{ self.0.as_bytes() }
}

// catch hayro's malformed-input panics and return None instead
fn undecodable_on_panic<T>(f:impl FnOnce() -> Option<T>) -> Option<T>{
  std::panic::catch_unwind(std::panic::AssertUnwindSafe(f)).ok().flatten()
}

// convert the specified (1-based index) page into a Picture (return None if the data or index are invalid)
pub fn read_page(data:&Data, page_num:usize) -> Option<(Picture, Size)>{
  undecodable_on_panic(||{
    let pdf = Pdf::new(Arc::new(PdfBytes(data.clone()))).ok()?;
    let page = pdf.pages().get(page_num.checked_sub(1)?)?;
    render_page(page, &InterpreterCache::new(), &Rc::default())
  })
}

// interpret all the document's pages, sharing the interpreter and embedded font caches
pub fn read_document(data:&Data) -> Option<Vec<(Picture, Size)>>{
  undecodable_on_panic(||{
    let pdf = Pdf::new(Arc::new(PdfBytes(data.clone()))).ok()?;
    let (cache, fonts) = (InterpreterCache::new(), Rc::default());
    pdf.pages().iter()
      .map(|page| render_page(page, &cache, &fonts))
      .collect::<Option<Vec<_>>>()
  })
}

fn render_page<'a>(page:&PdfPage<'a>, cache:&InterpreterCache<'a>, fonts:&Rc<FontCache>) -> Option<(Picture, Size)>{
  let (width, height) = page.render_dimensions();
  let size = Size::new(width, height);

  let mut recorder = PictureRecorder::new();
  let canvas = recorder.begin_recording(SkRect::from_size(size), true);
  let mut device = PictureDevice::new(canvas, fonts.clone());
  let mut context = PdfContext::new(
    page.initial_transform(true).to_kurbo(), // pdf user space (y-up) → top-left-origin device space
    kurbo::Rect::new(0.0, 0.0, width as f64, height as f64),
    cache,
    page.xref(),
    InterpreterSettings::default(),
  );

  interpret_page(page, &mut context, &mut device);
  device.flush_glyphs();
  drop(device);
  recorder.finish_recording_as_picture(None).map(|pict| (pict, size))
}

//
// The hayro Device: converts the interpreter's draw calls into a Picture recording
//

struct PictureDevice<'a, 'c>{
  canvas: &'c SkCanvas,
  mask: Option<SoftMask<'a>>,             // soft mask applied around each individual draw
  blend: BlendMode,                       // blend mode carried on each draw's paint
  group_masks: Vec<Option<SoftMask<'a>>>, // mask stack to restore when transparency groups pop
  fonts: Rc<FontCache>,                   // parsed fonts, shared with the rest of the document
  run: Option<GlyphRun<'a>>,              // glyphs accumulated for the pending text run
}

impl<'a, 'c> PictureDevice<'a, 'c>{
  fn new(canvas:&'c SkCanvas, fonts:Rc<FontCache>) -> Self{
    PictureDevice{
      canvas, mask:None, blend:BlendMode::Normal, group_masks:vec![], fonts, run:None
    }
  }
}

impl<'a> Device<'a> for PictureDevice<'a, '_>{
  fn set_soft_mask(&mut self, mask:Option<SoftMask<'a>>){
    if self.mask != mask{
      self.flush_glyphs();
      self.mask = mask;
    }
  }

  fn set_blend_mode(&mut self, blend:BlendMode){
    if self.blend != blend{
      self.flush_glyphs();
      self.blend = blend;
    }
  }

  fn draw_path(&mut self, path:&BezPath, transform:Affine, paint:&Paint<'a>, draw_mode:&PathDrawMode){
    self.flush_glyphs();
    self.with_mask(|dev| dev.draw_bez_path(path, transform, paint, draw_mode));
  }

  fn draw_glyph(&mut self, glyph:&Glyph<'a>, transform:Affine, glyph_transform:Affine, paint:&Paint<'a>, draw_mode:&GlyphDrawMode){
    if matches!(draw_mode, GlyphDrawMode::Invisible){ return }
    let stroke = match draw_mode{
      GlyphDrawMode::Stroke(props) => Some(props.clone()),
      _ => None,
    };

    // check whether the next glyph is compatible with being batched into a text-run
    if let Glyph::Outline(outline) = glyph
      && let Some((typeface, size, glyph_id, position, orientation)) = self.run_glyph(outline, glyph_transform)
    {
      // append compatible glyphs onto the pending TextBlob
      let transform = transform * orientation;
      if !self.run.as_ref().is_some_and(|run| run.accepts(&typeface, size, transform, paint, &stroke)){
        self.flush_glyphs(); // flush & clear the run if the font, size, paint, draw mode, or text-transform changed
      }
      let run = self.run.get_or_insert_with(|| GlyphRun::new(typeface, size, transform, paint, stroke));
      run.glyphs.push(glyph_id);
      run.positions.push(position);
    }else{
      // if not-batchable, draw the glyph individually
      self.flush_glyphs();
      self.with_mask(|dev| match glyph{
        Glyph::Outline(outline) => {
          // trace the glyph as a path with glyph_transform baked in, so only the text-transform scales the stroke
          let path = glyph_transform * outline.outline();
          let path_mode = stroke.map(PathDrawMode::Stroke).unwrap_or(PathDrawMode::Fill(FillRule::NonZero));
          dev.draw_bez_path(&path, transform, paint, &path_mode);
        }
        Glyph::Type3(glyph) => {
          // replay the glyph's drawing procedure directly
          glyph.interpret(dev, transform, glyph_transform, paint);
          dev.flush_glyphs();
        }
      });
    }
  }

  fn draw_image(&mut self, image:Image<'a, '_>, transform:Affine){
    self.flush_glyphs();
    self.with_mask(|dev| match image{
      Image::Raster(raster) => raster.with_rgba(|data, alpha| dev.draw_raster_image(data, alpha, transform), None),
      Image::Stencil(stencil) => stencil.with_stencil(|data, paint| dev.draw_stencil_image(data, paint, transform), None),
    });
  }

  fn push_clip_path(&mut self, clip_path:&ClipPath){
    self.flush_glyphs();
    let mut sk_path = skia_path(&clip_path.path);
    sk_path.set_fill_type(fill_type(clip_path.fill));
    self.canvas.save();
    self.canvas.clip_path(&sk_path, None, true);
  }

  fn pop_clip_path(&mut self){
    self.flush_glyphs();
    self.canvas.restore();
  }

  fn push_transparency_group(&mut self, opacity:f32, mask:Option<SoftMask<'a>>, blend:BlendMode){
    self.flush_glyphs();
    let mut layer_paint = blend_paint(blend_mode(blend));
    layer_paint.set_alpha_f(opacity);
    self.canvas.save_layer(&SaveLayerRec::default().paint(&layer_paint));
    self.group_masks.push(mask);
  }

  fn pop_transparency_group(&mut self){
    self.flush_glyphs();
    if let Some(Some(mask)) = self.group_masks.pop(){
      self.apply_mask(&mask);
    }
    self.canvas.restore();
  }
}

impl<'a> PictureDevice<'a, '_>{
  // every draw starts with AA enabled and uses the current blend mode
  fn base_paint(&self) -> SkPaint{
    let mut paint = blend_paint(blend_mode(self.blend));
    paint.set_anti_alias(true);
    paint
  }

  // run the callback with a temporary canvas transform
  fn with_ctm(&mut self, transform:Affine, f:impl FnOnce(&mut Self)){
    self.canvas.save();
    self.canvas.concat(&skia_matrix(transform));
    f(self);
    self.canvas.restore();
  }

  // run the callback with mask & blend state cleared (for mask groups & type3 glyphs that already account for them)
  fn isolated(&mut self, f:impl FnOnce(&mut Self)){
    let mask = self.mask.take();
    let blend = std::mem::replace(&mut self.blend, BlendMode::Normal);
    f(self);
    self.mask = mask;
    self.blend = blend;
  }

  // wrap a single draw in a masked layer when a soft mask is active (w/ the current blend mode on the layer)
  fn with_mask(&mut self, f:impl FnOnce(&mut Self)){
    match self.mask.clone(){
      Some(mask) => {
        self.canvas.save_layer(&SaveLayerRec::default().paint(&blend_paint(blend_mode(self.blend))));
        self.isolated(f);
        self.apply_mask(&mask);
        self.canvas.restore();
      }
      None => f(self)
    }
  }

  // wrap the mask group into a DstIn layer and replay atop the current layer (converting
  // luminance to alpha for /Luminosity masks). transfer functions aren't supported
  fn apply_mask(&mut self, mask:&SoftMask<'a>){
    let luminosity = mask.mask_type() == MaskType::Luminosity;
    let mut mask_paint = blend_paint(SkBlendMode::DstIn);
    if luminosity{
      mask_paint.set_color_filter(luma_color_filter::new());
    }

    self.canvas.save_layer(&SaveLayerRec::default().paint(&mask_paint));
    if luminosity{
      // unpainted areas count as transparent and only need a fill if the background isn't black
      let bg = mask.background_color().to_rgba();
      if bg.to_rgba8() != AlphaColor::BLACK.to_rgba8(){
        self.canvas.draw_color(color4f(bg), SkBlendMode::Src);
      }
    }
    self.isolated(|dev| {
      mask.interpret(dev);
      dev.flush_glyphs();
    });
    self.canvas.restore();
  }

  // map a glyph's embedded font to a skia Typeface (and memoize the lookup)
  fn font_for(&mut self, glyph:&OutlineGlyph) -> Option<Typeface>{
    let font_key = glyph.font_cache_key();
    let typeface = self.fonts.faces.borrow_mut().entry(font_key).or_insert_with(||
      glyph.font_data()
        .and_then(|font| FONT_MGR.with(|mgr| mgr.new_from_data((*font.data).as_ref(), None)))
    ).clone()?;

    // verification needs to be per-glyph since any one glyph may be a fallback font
    let verified = *self.fonts.verified.borrow_mut().entry((font_key, glyph.glyph_id().to_u32()))
      .or_insert_with(|| matches_outline(&typeface, glyph));
    verified.then_some(typeface)
  }

  // glyphs can join a TextBlob if it has a matching font and its glyph_transform has a uniform scale
  // plus rotation/reflection, then translation (i.e., no shearing, stretching, vertical layout)
  fn run_glyph(&mut self, glyph:&OutlineGlyph, glyph_transform:Affine) -> Option<(Typeface, f32, u16, SkPoint, Affine)>{
    // confirm the transform is compatible
    let [a, b, c, d, e, f] = glyph_transform.as_coeffs();
    let scale = (a * a + b * b).sqrt();
    let similarity = scale > 0.0
      && ((c * c + d * d).sqrt() - scale).abs() <= scale * 1e-4 // columns of equal length,
      && (a * c + b * d).abs() <= scale * scale * 1e-4;         // and at right angles
    if !similarity{ return None }

    // extract the transform and account for a y-flip (skia is y-down, hayro is y-up)
    let orientation = Affine::new([a, b, c, d, 0.0, 0.0])
      * Affine::scale_non_uniform(1.0 / scale, -1.0 / scale);

    let glyph_id = u16::try_from(glyph.glyph_id().to_u32()).ok()?;
    let typeface = self.font_for(glyph)?;

    // remap each glyph's 2D position relative to the TextBlob's frame (which may be multi-line)
    let placed = orientation.inverse() * kurbo::Point::new(e, f);
    let position = SkPoint::new(placed.x as f32, placed.y as f32);
    Some((typeface, (scale * 1000.0) as f32, glyph_id, position, orientation))
  }

  // draw the pending run as a single TextBlob
  fn flush_glyphs(&mut self){
    let Some(run) = self.run.take() else { return };
    self.with_mask(|dev|{
      let font = unhinted_font(run.typeface, run.size);
      let mut builder = TextBlobBuilder::new();

      // preserve PDF glyph positions rather than re-advancing from font metrics
      let (glyphs, points) = builder.alloc_run_pos(&font, run.glyphs.len(), None);
      glyphs.copy_from_slice(&run.glyphs);
      points.copy_from_slice(&run.positions);
      let Some(blob) = builder.make() else { return };

      let device_bounds = || map_rect(run.transform, kurbo_rect(*blob.bounds())); // guaranteed not to clip
      let Some(mut sk_paint) = dev.paint_for(&run.paint, run.transform, run.stroke.is_some(), device_bounds) else { return };
      if let Some(props) = &run.stroke{
        stroke_paint(&mut sk_paint, props);
      }

      dev.with_ctm(run.transform, |dev|{
        dev.clip_to_shading(&run.paint, run.transform);
        dev.canvas.draw_text_blob(&blob, (0.0, 0.0), &sk_paint);
      });
    });
  }

  // solid colors can be used directly but pattern shaders need the CTM so they can position themselves in page-space
  // and the device bounds to sample over (in case they need to fall back to rasterization)
  fn paint_for(&self, paint:&Paint<'a>, ctm:Affine, is_stroke:bool, device_bounds:impl FnOnce() -> kurbo::Rect) -> Option<SkPaint>{
    let mut sk_paint = self.base_paint();
    match paint{
      Paint::Color(color) => {
        sk_paint.set_color4f(color4f(color.to_rgba()), None);
      }
      Paint::Pattern(pattern) => {
        let shader = match pattern.as_ref(){
          Pattern::Shading(shading) => shading_shader(shading, ctm, device_bounds),
          Pattern::Tiling(tile) => self.tiling_shader(tile, ctm, is_stroke),
        }?;
        sk_paint.set_shader(shader);
      }
    }
    Some(sk_paint)
  }

  fn draw_bez_path(&mut self, path:&BezPath, transform:Affine, paint:&Paint<'a>, draw_mode:&PathDrawMode){
    let device_bounds = ||{
      let mut bounds = path.bounding_box();
      if let PathDrawMode::Stroke(props) = draw_mode{
        bounds = bounds.inflate(props.line_width as f64, props.line_width as f64);
      }
      map_rect(transform, bounds)
    };
    let is_stroke = matches!(draw_mode, PathDrawMode::Stroke(_));
    let Some(mut sk_paint) = self.paint_for(paint, transform, is_stroke, device_bounds) else { return };
    let mut sk_path = skia_path(path);
    match draw_mode{
      PathDrawMode::Fill(rule) => { sk_path.set_fill_type(fill_type(*rule)); }
      PathDrawMode::Stroke(props) => { stroke_paint(&mut sk_paint, props); }
    }

    self.with_ctm(transform, |dev|{
      dev.clip_to_shading(paint, transform);
      dev.canvas.draw_path(&sk_path, &sk_paint);
    });
  }

  // when the paint carries a shading pattern with a /BBox clip, Intersect it with the canvas's clip
  fn clip_to_shading(&self, paint:&Paint<'a>, transform:Affine){
    if let Paint::Pattern(pattern) = paint
      && let Pattern::Shading(shading) = pattern.as_ref()
      && let Some(clip) = &shading.shading.clip_path{
        // the bbox clip is in device space so factor out the CTM first
        self.canvas.clip_path(&skia_path(&(transform.inverse() * clip.clone())), None, true);
    }
  }

  // record the pattern stamp into a vector-based Picture then tile it, filling the entire rect
  fn tiling_shader(&self, tile:&TilingPattern<'a>, ctm:Affine, is_stroke:bool) -> Option<Shader>{
    let (x_step, y_step) = (tile.x_step.abs() as f64, tile.y_step.abs() as f64);
    let tile_rect = kurbo::Rect::new(tile.bbox.x0, tile.bbox.y0, tile.bbox.x0 + x_step, tile.bbox.y0 + y_step);
    // calculate how many neighbors-over a single stamp's ink can reach (which in turn sets the number
    // of distinct stamps that need to be drawn). capped at 65 x 65 since growth is quadratic
    let spill = |extent:f64, step:f64| ((extent / step).ceil() as i32 - 1).clamp(0, 32);
    let (nx, ny) = (spill(tile.bbox.width(), x_step), spill(tile.bbox.height(), y_step));

    // record the replayable tile stamp (which hayro has already clipped to its bbox)
    let mut cell_recorder = PictureRecorder::new();
    let cell_canvas = cell_recorder.begin_recording(skia_rect(tile.bbox), true);
    let mut cell_device = PictureDevice::new(cell_canvas, self.fonts.clone());
    tile.interpret(&mut cell_device, Affine::IDENTITY, is_stroke);
    cell_device.flush_glyphs();
    drop(cell_device);
    let cell = cell_recorder.finish_recording_as_picture(None)?;

    let mut recorder = PictureRecorder::new();
    let cull = tile.bbox.union(tile_rect).inflate(nx as f64 * x_step, ny as f64 * y_step);
    let canvas = recorder.begin_recording(skia_rect(cull), true);
    for dy in -ny..=ny{
      for dx in -nx..=nx{
        let offset = Matrix::translate((dx as f32 * x_step as f32, dy as f32 * y_step as f32));
        canvas.draw_picture(&cell, Some(&offset), None);
      }
    }

    let picture = recorder.finish_recording_as_picture(None)?;
    let local_matrix = skia_matrix(ctm.inverse() * tile.matrix);
    Some(picture.to_shader(
      Some((TileMode::Repeat, TileMode::Repeat)), FilterMode::Linear,
      Some(&local_matrix), Some(&skia_rect(tile_rect)),
    ))
  }

  fn draw_raster_image(&mut self, image_data:ImageData, alpha:Option<LumaData>, transform:Affine){
    let (scale_x, scale_y) = image_data.scale_factors();
    let transform = transform * Affine::scale_non_uniform(scale_x as f64, scale_y as f64);
    let sample_opts = sampling(image_data.interpolate());
    let (width, height) = (image_data.width(), image_data.height());

    // interleave the color channels and alpha channel (when its dimensions match) into rgba
    let matched_alpha = alpha.as_ref().filter(|a| a.width == width && a.height == height);
    let rgba:Vec<u8> = match &image_data{
      ImageData::Rgb(rgb) => match matched_alpha{
        Some(a) => rgb.data.chunks_exact(3).zip(&a.data).flat_map(|(px, a)| [px[0], px[1], px[2], *a]).collect(),
        None => rgb.data.chunks_exact(3).flat_map(|px| [px[0], px[1], px[2], 255]).collect(),
      },
      ImageData::Luma(luma) => match matched_alpha{
        Some(a) => luma.data.iter().zip(&a.data).flat_map(|(g, a)| [*g, *g, *g, *a]).collect(),
        None => luma.data.iter().flat_map(|g| [*g, *g, *g, 255]).collect(),
      },
    };
    let merged_alpha = matched_alpha.is_some();
    let Some(sk_image) = rgba_image(rgba, width, height) else { return };

    let sk_paint = self.base_paint();
    self.with_ctm(transform, |dev| match alpha.filter(|_| !merged_alpha){
      Some(a) => {
        // if the alpha channel has different dimensions, composite it with the colors in a DstIn layer
        dev.canvas.save_layer(&SaveLayerRec::default().paint(&sk_paint));
        dev.canvas.draw_image_with_sampling_options(&sk_image, (0, 0), sample_opts, None);
        if let Some(mask) = alpha_image(&a){
          dev.canvas.draw_image_rect_with_sampling_options(
            &mask, None, SkRect::from_iwh(width as i32, height as i32), sampling(a.interpolate),
            &blend_paint(SkBlendMode::DstIn)
          );
        }
        dev.canvas.restore();
      }
      None => {
        dev.canvas.draw_image_with_sampling_options(&sk_image, (0, 0), sample_opts, Some(&sk_paint));
      }
    });
  }

  fn draw_stencil_image(&mut self, stencil:LumaData, paint:&Paint<'a>, transform:Affine){
    let (scale_x, scale_y) = stencil.scale_factors;
    let transform = transform * Affine::scale_non_uniform(scale_x as f64, scale_y as f64);

    match paint{
      Paint::Color(color) => {
        // the stencil mask's on-bits take the paint color, the rest stay transparent
        let color = color4f(color.to_rgba()).to_color();
        let on:[u8; 4] = [color.r(), color.g(), color.b(), color.a()];
        let rgba:Vec<u8> = stencil.data.iter().flat_map(|d| if *d == 255{ on }else{ [0, 0, 0, 0] }).collect();
        let Some(sk_image) = rgba_image(rgba, stencil.width, stencil.height) else { return };

        let sk_paint = self.base_paint();
        self.with_ctm(transform, |dev| {
          dev.canvas.draw_image_with_sampling_options(&sk_image, (0, 0), sampling(stencil.interpolate), Some(&sk_paint));
        });
      }
      Paint::Pattern(_) => {
        // fill the stencil's rect with the pattern, then knock out the mask's off-bits with DstIn
        let rect = kurbo::Rect::new(0.0, 0.0, stencil.width as f64, stencil.height as f64);
        let Some(mut pattern_paint) = self.paint_for(paint, transform, false, || map_rect(transform, rect)) else { return };
        let Some(mask) = alpha_image(&stencil) else { return };

        // apply the blend more when compositing the masked result, not inside the layer
        let layer_paint = blend_paint(blend_mode(self.blend));
        pattern_paint.set_blend_mode(SkBlendMode::SrcOver);

        self.with_ctm(transform, |dev|{
          dev.clip_to_shading(paint, transform); // outside the layer, so it bounds the mask too
          dev.canvas.save_layer(&SaveLayerRec::default().paint(&layer_paint));
          dev.canvas.draw_rect(skia_rect(rect), &pattern_paint);
          dev.canvas.draw_image_with_sampling_options(
            &mask, (0, 0), sampling(stencil.interpolate), Some(&blend_paint(SkBlendMode::DstIn))
          );
          dev.canvas.restore();
        });
      }
    }
  }
}

//
// Text runs
//

// share one FontMgr per thread to amortize its expensive (~30ms) setup time
thread_local!(static FONT_MGR: FontMgr = FontMgr::new());

// per-document font cache
#[derive(Default)]
struct FontCache{
  faces: RefCell<HashMap<u128, Option<Typeface>>>, // embedded fonts by font_cache_key (None = unparseable)
  verified: RefCell<HashMap<(u128, u32), bool>>,   // per-glyph outline agreement, by font_cache_key & glyph id
}

// set of glyphs sharing a font, size, paint, and text-space transform (to be converted to a TextBlob)
struct GlyphRun<'a>{
  typeface: Typeface,
  size: f32,
  transform: Affine,
  paint: Paint<'a>,
  paint_key: Option<u128>, // None = uses a pattern paint that can't be compared (outside hayro) across neighboring glyphs
  stroke: Option<StrokeProps>, // None = fill
  glyphs: Vec<u16>,
  positions: Vec<SkPoint>,
}

impl<'a> GlyphRun<'a>{
  fn new(typeface:Typeface, size:f32, transform:Affine, paint:&Paint<'a>, stroke:Option<StrokeProps>) -> Self{
    GlyphRun{
      typeface, size, transform, paint_key:Self::paint_key(paint), paint:paint.clone(), stroke,
      glyphs:vec![], positions:vec![],
    }
  }

  // a glyph can extend the run only if everything the TextBlob bakes in is unchanged
  fn accepts(&self, typeface:&Typeface, size:f32, transform:Affine, paint:&Paint, stroke:&Option<StrokeProps>) -> bool{
    let paint_key = Self::paint_key(paint);
    paint_key.is_some()
      && self.paint_key == paint_key
      && self.typeface.unique_id() == typeface.unique_id()
      && (self.size - size).abs() <= size * 1e-4
      && self.transform == transform
      && match (&self.stroke, stroke){
        // stroke settings are part of the run's identity too: any change breaks the batch
        (None, None) => true,
        (Some(a), Some(b)) =>
          (a.line_width, a.miter_limit, a.line_cap, a.line_join, &a.dash_array, a.dash_offset) ==
          (b.line_width, b.miter_limit, b.line_cap, b.line_join, &b.dash_array, b.dash_offset),
        _ => false,
      }
  }

  // a paint's identity: Some for solid colors and None for patterns (whose keys are lossy and
  // collide). None never lets a glyph join a run, so pattern-filled glyphs each draw alone
  fn paint_key(paint:&Paint) -> Option<u128>{
    matches!(paint, Paint::Color(_)).then(|| paint.cache_key())
  }
}

// a font configured for precise vector outlines (no hinting or pixel-grid snapping)
fn unhinted_font(typeface:Typeface, size:f32) -> SkFont{
  let mut font = SkFont::from_typeface(typeface, size);
  font.set_hinting(FontHinting::None);
  font.set_subpixel(true);
  font.set_baseline_snap(false);
  font.set_linear_metrics(true);
  font
}

// confirm the typeface actually reproduces the embedded font (since the FontMgr may parse the
// blob without honoring it) by comparing skia's glyph bounds vs hayro's
fn matches_outline(typeface:&Typeface, glyph:&OutlineGlyph) -> bool{
  let Ok(glyph_id) = u16::try_from(glyph.glyph_id().to_u32()) else { return false };
  let font = unhinted_font(typeface.clone(), 1000.0);
  let reference = glyph.outline().bounding_box();
  let skia_path = font.get_path(glyph_id).filter(|path| !path.is_empty());
  match (reference.is_zero_area(), skia_path){
    (true, None) => true, // both agree the glyph is blank
    (false, Some(path)) => {
      let bounds = path.bounds();
      [
        bounds.left as f64 - reference.x0, bounds.right as f64 - reference.x1,
        bounds.top as f64 + reference.y1, bounds.bottom as f64 + reference.y0,
      ].iter().all(|delta| delta.abs() <= 5.0)
    }
    _ => false,
  }
}

//
// Shading patterns
//


fn shading_shader(pattern:&ShadingPattern, ctm:Affine, device_bounds:impl FnOnce() -> kurbo::Rect) -> Option<Shader>{
  gradient_shader(pattern, ctm) // axial and radial shadings are converted to gradients
    .or_else(|| sweep_shader(pattern, ctm)) // function-based shadings that are angular become sweeps
    .or_else(|| sampled_shader(pattern, ctm, device_bounds())) // everything else is rasterized
}

// if the pattern is a linear or radial gradient, convert it directly
fn gradient_shader(pattern:&ShadingPattern, ctm:Affine) -> Option<Shader>{
  let ShadingType::RadialAxial{coords, domain, function, extend, axial} = pattern.shading.shading_type.as_ref()
    else { return None };
  if pattern.shading.background.is_some(){ return None }

  let (colors, positions) = gradient_stops(pattern, function, *domain, *extend)?;
  let local_matrix = skia_matrix(ctm.inverse() * pattern.matrix);
  let spec = gradient::Gradient::new(
    GradientColors::new(&colors, Some(&positions), TileMode::Clamp, None),
    Interpolation::default(),
  );
  match axial{
    true => gradient::shaders::linear_gradient(
      ((coords[0], coords[1]), (coords[2], coords[3])), &spec, Some(&local_matrix)
    ),
    false => gradient::shaders::two_point_conical_gradient(
      ((coords[0], coords[1]), coords[2]), ((coords[3], coords[4]), coords[5]), &spec, Some(&local_matrix)
    ),
  }
}

// when skia writes a sweep gradient to a PDF it uses a function-based shading that only depends on
// the angle around the origin (and no radial term). sample a loop of colors from the function and,
// if nothing varies with radius, convert it back into a skia conic gradient
fn sweep_shader(pattern:&ShadingPattern, ctm:Affine) -> Option<Shader>{
  if pattern.shading.background.is_some(){ return None }
  let ShadingType::FunctionBased{domain, matrix, function} = pattern.shading.shading_type.as_ref()
    else { return None };

  // sweeps are centered on the shading space origin so the domain needs to fully enclose it to continue
  let bounds = kurbo::Rect::new(domain[0] as f64, domain[2] as f64, domain[1] as f64, domain[3] as f64);
  let reach = [-bounds.x0, bounds.x1, -bounds.y0, bounds.y1]
    .into_iter().fold(f64::INFINITY, f64::min);
  if !(reach > 0.0){ return None }

  // sampling helpers
  let color_at = |turns:f64, radius:f64| -> Option<Color4f>{
    let (sin, cos) = (std::f64::consts::TAU * turns).sin_cos();
    let value = function.eval(&smallvec![(radius * cos) as f32, (radius * sin) as f32])?;
    Some(shading_color(pattern, &value))
  };
  let radial_match = |turns:f64, spin:f64, n:usize| -> Option<bool>{
    let (sin, cos) = (std::f64::consts::TAU * turns).sin_cos();
    let edge = |d:f64, near:f64, far:f64|
      if d > 0.0{ far / d }else if d < 0.0{ near / d }else{ f64::INFINITY };
    let extent = edge(cos, bounds.x0, bounds.x1).min(edge(sin, bounds.y0, bounds.y1));
    let probe = |k:usize| 0.15 + 0.83 * (k as f64 / n as f64 + spin).fract();

    let reference = color_at(turns, extent * probe(0))?;
    for k in 1..n{
      if !alike(&reference, &color_at(turns, extent * probe(k))?){ return Some(false) }
    }
    Some(true)
  };

  // sample at multiple points along each ray to ensure colors only differ based on angle
  const RAYS:usize = 24; // number of angles tested
  const RADII:usize = 4; // number of radii checked along each of those angles
  const GOLDEN:f64 = 0.618_033_988_749_895; // irrational steps, so neither angles nor radii repeat
  const SILVER:f64 = 0.414_213_562_373_095;
  for ray in 0..RAYS{
    if !radial_match((ray as f64 * GOLDEN).fract(), (ray as f64 * SILVER).fract(), RADII)?{ return None }
  }

  // uniformly sample the ring of colors then bisect between them to find the actual gradient stops
  const SAMPLES:usize = 256;  // uniform probes around the circle
  const CHECKS:usize = 8;     // angular samples between radial spot-checks
  const CEILING:usize = 4096; // max number of stops to generate
  let sample = |turns:f64| color_at(turns, reach * 0.6); // sample the ring at the 60% point of the disc
  let mut stops = Vec::with_capacity(SAMPLES);
  let mut lo = (0.0, sample(0.0)?);
  for i in 1..SAMPLES{
    let turns = i as f64 / (SAMPLES - 1) as f64;
    let hi = (turns, sample(turns)?);

    // keep double-checking that colors are constant along each ray
    if i % CHECKS == 0 && !radial_match(turns, (i as f64 * SILVER).fract(), 2)?{ return None }

    stops.extend(find_stops(lo, hi, &sample)?); // bisect to find the next actual stop
    if stops.len() > CEILING{ return None } // bail out if it would be cheaper to just rasterize
    lo = hi;
  }
  stops.push(lo);

  // construct and return a sweep gradient equivalent to the shading function
  let (positions, colors):(Vec<f32>, Vec<Color4f>) =
    trim_stops(stops).iter().map(|(turns, color)| (*turns as f32, *color)).unzip();
  let spec = gradient::Gradient::new(
    GradientColors::new(&colors, Some(&positions), TileMode::Clamp, None),
    Interpolation::default(),
  );
  let local_matrix = skia_matrix(ctm.inverse() * pattern.matrix * *matrix);
  gradient::shaders::sweep_gradient((0.0, 0.0), (0.0, 360.0), &spec, Some(&local_matrix))
}

// emulate hayro's color-handling/effect-ordering so our live gradients match its rasterizations
fn shading_color(pattern:&ShadingPattern, value:&[f32]) -> Color4f{
  let mut components = pattern.shading.color_space.to_rgba(value, 1.0, false).components();
  components[3] *= pattern.opacity;
  if let Some(tf) = &pattern.transfer_function{
    components = tf.apply(&AlphaColor::new(components)).components();
  }
  Color4f::new(components[0], components[1], components[2], components[3])
}

//
// Gradient helpers
//

// compare two colors at 8-bit precision
fn alike(a:&Color4f, b:&Color4f) -> bool{
  [a.r - b.r, a.g - b.g, a.b - b.b, a.a - b.a].iter().all(|delta| delta.abs() <= 1.0/255.0)
}

// drop stops that are indistinguishable from the neighbors on either side (let interpolation reproduce them)
fn trim_stops(stops:Vec<(f64, Color4f)>) -> Vec<(f64, Color4f)>{
  let mut trimmed:Vec<(f64, Color4f)> = Vec::with_capacity(stops.len());
  for (i, stop) in stops.iter().enumerate(){
    let follows_match = trimmed.last().is_some_and(|prev| alike(&prev.1, &stop.1));
    let precedes_match = stops.get(i + 1).is_some_and(|next| alike(&stop.1, &next.1));
    if !(follows_match && precedes_match){ trimmed.push(*stop) }
  }
  trimmed
}

// repeatedly split the span between pairs of samples until what's left is a linear ramp (in order to find
// the actual stops and handle hard transition boundaries where the bisection LIMIT is reached)
fn find_stops(lo:(f64, Color4f), hi:(f64, Color4f),
              sample:&impl Fn(f64) -> Option<Color4f>) -> Option<Vec<(f64, Color4f)>>{
  const LIMIT:f64 = 1e-5; // narrowest span worth splitting
  let mut stops = Vec::new();
  let mut spans = vec![(lo, hi)];
  while let Some((a, b)) = spans.pop(){
    if b.0 - a.0 < LIMIT{ stops.push(a); continue }
    let split = (a.0 + b.0) / 2.0;
    let mid = (split, sample(split)?);
    let mean = Color4f::new(
      (a.1.r + b.1.r) / 2.0, (a.1.g + b.1.g) / 2.0,
      (a.1.b + b.1.b) / 2.0, (a.1.a + b.1.a) / 2.0,
    );
    match alike(&mean, &mid.1){ // is midpoint linear?
      true => stops.push(a), // keep the span's left end
      false => { spans.push((mid, b)); spans.push((a, mid)); } // keep dividing, left half first
    }
  }
  Some(stops)
}

// find the list of stops in a linear/radial shading function
fn gradient_stops(pattern:&ShadingPattern, function:&ShadingFunction, domain:[f32; 2], extend:[bool; 2]) -> Option<(Vec<Color4f>, Vec<f32>)>{
  const PROBES:usize = 256;   // uniform probes across the domain
  const CEILING:usize = 4096; // max number of stops to generate
  const CLEAR:Color4f = Color4f::new(0.0, 0.0, 0.0, 0.0);
  let [t0, t1] = domain;
  let sample = |s:f64| -> Option<Color4f>{
    let value = function.eval(&smallvec![t0 + (t1 - t0) * s as f32])?;
    Some(shading_color(pattern, &value))
  };

  let mut stops = Vec::with_capacity(PROBES);
  let mut lo = (0.0, sample(0.0)?);
  for i in 1..PROBES{
    let s = i as f64 / (PROBES - 1) as f64;
    let hi = (s, sample(s)?);
    stops.extend(find_stops(lo, hi, &sample)?);
    if stops.len() > CEILING{ return None } // bail out if it would be cheaper to just rasterize
    lo = hi;
  }
  stops.push(lo);

  // bracket the ramp with transparent stops wherever the shading isn't set to extend
  let stops = trim_stops(stops);
  let mut colors = Vec::with_capacity(stops.len() + 2);
  let mut positions = Vec::with_capacity(stops.len() + 2);
  if !extend[0]{ colors.push(CLEAR); positions.push(0.0); }
  for (s, color) in stops{
    colors.push(color);
    positions.push(s as f32);
  }
  if !extend[1]{ colors.push(CLEAR); positions.push(1.0); }
  Some((colors, positions))
}

// rasterize the shading over the device-space bounds (when the shading isn't recognized as a gradient)
fn sampled_shader(pattern:&ShadingPattern, ctm:Affine, device_bounds:kurbo::Rect) -> Option<Shader>{
  const MAX_EDGE:f64 = 4096.0;
  const MAX_SAMPLES:f64 = 4.0e6;

  // non-finite bounds can't be covered by any raster, so cap the size and let the shader's
  // Decal tiling leave the rest transparent
  let extent = |size:f64| if size.is_finite(){ size.max(1.0) }else{ MAX_EDGE };
  let (extent_x, extent_y) = (extent(device_bounds.width()), extent(device_bounds.height()));

  let (mut px_x, mut px_y) = (extent_x.min(MAX_EDGE), extent_y.min(MAX_EDGE));
  if px_x * px_y > MAX_SAMPLES{
    let shrink = (MAX_SAMPLES / (px_x * px_y)).sqrt();
    px_x *= shrink;
    px_y *= shrink;
  }
  let (width, height) = ((px_x.ceil() as i32).max(1), (px_y.ceil() as i32).max(1));
  let (scale_x, scale_y) = (width as f64 / extent_x, height as f64 / extent_y);

  let encoded = pattern.encode();
  let mut rgba:Vec<u8> = Vec::with_capacity((width * height * 4) as usize);
  for y in 0..height{
    for x in 0..width{
      let pos = encoded.base_transform * kurbo::Point::new(
        device_bounds.x0 + (x as f64 + 0.5) / scale_x,
        device_bounds.y0 + (y as f64 + 0.5) / scale_y,
      );
      rgba.extend(encoded.sample(pos).map(|c| (c.clamp(0.0, 1.0) * 255.0 + 0.5) as u8));
    }
  }

  // wrap the resulting raster in an image shader
  let image = rgba_image(rgba, width as u32, height as u32)?;
  let local_matrix = skia_matrix(
    ctm.inverse()
      * Affine::translate((device_bounds.x0, device_bounds.y0))
      * Affine::scale_non_uniform(1.0 / scale_x, 1.0 / scale_y)
  );
  image.to_shader(
    Some((TileMode::Decal, TileMode::Decal)),
    SamplingOptions::from(FilterMode::Linear),
    Some(&local_matrix),
  )
}

//
// hayro/kurbo <-> skia type conversions
//

fn skia_path(bez:&BezPath) -> SkPath{
  let xy = |p:&kurbo::Point| (p.x as f32, p.y as f32);
  let mut path = PathBuilder::new();
  for element in bez.elements(){
    match element{
      PathEl::MoveTo(p) => { path.move_to(xy(p)); }
      PathEl::LineTo(p) => { path.line_to(xy(p)); }
      PathEl::QuadTo(c, p) => { path.quad_to(xy(c), xy(p)); }
      PathEl::CurveTo(c1, c2, p) => { path.cubic_to(xy(c1), xy(c2), xy(p)); }
      PathEl::ClosePath => { path.close(); }
    }
  }
  path.snapshot()
}

fn skia_rect(rect:kurbo::Rect) -> SkRect{
  SkRect::new(rect.x0 as f32, rect.y0 as f32, rect.x1 as f32, rect.y1 as f32)
}

fn kurbo_rect(rect:SkRect) -> kurbo::Rect{
  kurbo::Rect::new(rect.left as f64, rect.top as f64, rect.right as f64, rect.bottom as f64)
}

// the device-space bounding box of a rect drawn under the given transform
fn map_rect(transform:Affine, rect:kurbo::Rect) -> kurbo::Rect{
  (transform * rect.to_path(0.1)).bounding_box()
}

fn skia_matrix(affine:Affine) -> Matrix{
  let [a, b, c, d, e, f] = affine.as_coeffs();
  Matrix::new_all(a as f32, c as f32, e as f32, b as f32, d as f32, f as f32, 0.0, 0.0, 1.0)
}

fn color4f(color:AlphaColor) -> Color4f{
  let [r, g, b, a] = color.components();
  Color4f::new(r, g, b, a)
}

fn blend_paint(mode:SkBlendMode) -> SkPaint{
  let mut paint = SkPaint::default();
  paint.set_blend_mode(mode);
  paint
}

fn fill_type(rule:FillRule) -> PathFillType{
  match rule{
    FillRule::NonZero => PathFillType::Winding,
    FillRule::EvenOdd => PathFillType::EvenOdd,
  }
}

fn sampling(interpolate:bool) -> SamplingOptions{
  match interpolate{
    true => SamplingOptions::from(FilterMode::Linear),
    false => SamplingOptions::from(FilterMode::Nearest),
  }
}

fn stroke_paint(paint:&mut SkPaint, props:&StrokeProps){
  paint.set_style(PaintStyle::Stroke);
  paint.set_stroke_width(props.line_width);
  paint.set_stroke_miter(props.miter_limit);
  paint.set_stroke_cap(match props.line_cap{
    Cap::Butt => PaintCap::Butt, Cap::Round => PaintCap::Round, Cap::Square => PaintCap::Square
  });
  paint.set_stroke_join(match props.line_join{
    Join::Miter => PaintJoin::Miter, Join::Round => PaintJoin::Round, Join::Bevel => PaintJoin::Bevel
  });

  if props.dash_array.iter().any(|gap| *gap > 0.0){
    // skia requires an even number of intervals; pdf allows odd-length arrays, which repeat
    let mut intervals = props.dash_array.to_vec();
    if intervals.len() % 2 != 0{
      intervals.extend_from_slice(&props.dash_array);
    }
    if let Some(effect) = dash_path_effect::new(&intervals, props.dash_offset){
      paint.set_path_effect(effect);
    }
  }
}

fn rgba_image(rgba:Vec<u8>, width:u32, height:u32) -> Option<skia_safe::Image>{
  let info = ImageInfo::new(
    (width as i32, height as i32), ColorType::RGBA8888, AlphaType::Unpremul, ColorSpace::new_srgb()
  );
  images::raster_from_data(&info, Data::new_copy(&rgba), info.min_row_bytes())
}

fn alpha_image(luma:&LumaData) -> Option<skia_safe::Image>{
  let info = ImageInfo::new(
    (luma.width as i32, luma.height as i32), ColorType::Alpha8, AlphaType::Premul, None
  );
  images::raster_from_data(&info, Data::new_copy(&luma.data), info.min_row_bytes())
}

fn blend_mode(mode:BlendMode) -> SkBlendMode{
  match mode{
    BlendMode::Normal => SkBlendMode::SrcOver,
    BlendMode::Multiply => SkBlendMode::Multiply,
    BlendMode::Screen => SkBlendMode::Screen,
    BlendMode::Overlay => SkBlendMode::Overlay,
    BlendMode::Darken => SkBlendMode::Darken,
    BlendMode::Lighten => SkBlendMode::Lighten,
    BlendMode::ColorDodge => SkBlendMode::ColorDodge,
    BlendMode::ColorBurn => SkBlendMode::ColorBurn,
    BlendMode::HardLight => SkBlendMode::HardLight,
    BlendMode::SoftLight => SkBlendMode::SoftLight,
    BlendMode::Difference => SkBlendMode::Difference,
    BlendMode::Exclusion => SkBlendMode::Exclusion,
    BlendMode::Hue => SkBlendMode::Hue,
    BlendMode::Saturation => SkBlendMode::Saturation,
    BlendMode::Color => SkBlendMode::Color,
    BlendMode::Luminosity => SkBlendMode::Luminosity,
  }
}


//
// -- Javascript Methods --------------------------------------------------------------------------
//

pub struct DocumentPage{ picture: Picture }
pub type BoxedDocumentPage = JsBox<DocumentPage>;
impl Finalize for DocumentPage {}

pub fn open(mut cx: FunctionContext) -> JsResult<JsValue> {
  let buffer = cx.argument::<JsBuffer>(0)?;

  // check for the %PDF magic number so the caller can distinguish between not-a-pdf and broken-pdf
  if is_pdf(buffer.as_slice(&cx)){
    let data = Data::new_copy(buffer.as_slice(&cx));
    match read_document(&data){
      Some(pages) => {
        // return the Picture + dimensions for each decoded page
        let array = JsArray::new(&mut cx, pages.len());
        for (i, (picture, size)) in pages.into_iter().enumerate(){
          let entry = cx.empty_object();
          let width = cx.number(size.width);
          let height = cx.number(size.height);
          let page = cx.boxed(DocumentPage{ picture });
          entry.set(&mut cx, "page", page)?;
          entry.set(&mut cx, "width", width)?;
          entry.set(&mut cx, "height", height)?;
          array.set(&mut cx, i as u32, entry)?;
        }
        Ok(array.upcast())
      }
      None => return Ok(cx.boolean(false).upcast()) // false = broken PDF
    }
  }else{
    return Ok(cx.undefined().upcast()) // undefined = not a pdf
  }
}

pub fn impose(mut cx: FunctionContext) -> JsResult<JsUndefined> {
  let page = cx.argument::<BoxedDocumentPage>(0)?;
  let context = cx.argument::<BoxedContext2D>(1)?;
  context.borrow().with_canvas(|canvas|{ canvas.draw_picture(&page.picture, None, None); });
  Ok(cx.undefined())
}
