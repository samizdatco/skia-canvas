#!/usr/bin/env node

const fs = require('fs'),
      {inspect} = require('util'),
      {extname} = require('path'),
      glob = require('glob').sync,
      get = require('simple-get'),
      crate = require('./v6/index.node'),
      REPR = inspect.custom

const _s_e_l_f_ = Symbol.for('self'),
      alloc = (obj, ...args) => init(obj, 'new', ...args),
      init = (obj, fn, ...args) => obj[_s_e_l_f_] = crate[`${obj.constructor.name}_${fn}`](null, ...args),
      core = (obj) => obj[_s_e_l_f_],
      inject = (obj, box) => Object.assign(obj, {[_s_e_l_f_]:box}),
      ƒ = (obj, fn, ...args) => crate[`${obj.constructor.name}_${fn}`](obj[_s_e_l_f_], ...args);

// shorthand for attaching read-only attributes
const readOnly = (obj, attr, value) => {
  Object.defineProperty(obj, attr, {value, writable:false, enumerable:true})
}

// convert arguments list to a string of type abbreviations
function signature(args){
  return args.map(v => (Array.isArray(v) ? 'a' : {string:'s', number:'n', object:'o'}[typeof v] || 'x')).join('')
}

//
// Helpers to reconcile Skia and DOMMatrix’s disagreement about row/col orientation
//

function toSkMatrix(jsMatrix){
  if (Array.isArray(jsMatrix)){
    var [a, b, c, d, e, f] = jsMatrix
  }else{
    var {a, b, c, d, e, f} = jsMatrix
  }
  return [a, c, e, b, d, f]
}

function fromSkMatrix(skMatrix){
  // TBD: how/if to map the perspective terms
  let [a, c, e, b, d, f, p0, p1, p2] = skMatrix
  return new DOMMatrix([a, b, c, d, e, f])
}

class Canvas{
  static parent = new WeakMap()
  static context = new WeakMap()

  constructor(width, height){
    alloc(this, width, height)

    // let ctx = new CanvasRenderingContext2D(width, height)
    // Canvas.parent.set(ctx, this)
    // Canvas.context.set(this, [ctx])
  }

  getContext(kind){
    return (kind=="2d") ? Canvas.context.get(this)[0] : null
  }

  get width(){ return ƒ(this, 'get_width') }
  get height(){ return ƒ(this, 'get_height') }
  set width(w){ return ƒ(this, 'set_width', w) }
  set height(h){ return ƒ(this, 'set_height', h) }

  newPage(width, height){
    let ctx = new CanvasRenderingContext2D(width, height)
    Canvas.parent.set(ctx, this)
    Canvas.context.get(this).unshift(ctx)
    Object.assign(this, {width, height})
    return ctx
  }

  get pages(){
    return Canvas.context.get(this).slice().reverse()
  }

  get png(){ return this.toBuffer("png") }
  get jpg(){ return this.toBuffer("jpg") }
  get pdf(){ return this.toBuffer("pdf") }
  get svg(){ return this.toBuffer("svg") }

  saveAs(filename, {format, quality=100}={}){
    var seq
    filename = filename.replace(/{(\d*)}/g, (_, pad) => {
      pad = parseInt(pad, 10)
      seq = isFinite(pad) ? pad : isFinite(seq) ? seq : -1
      return "{}"
    })

    let ext = format || extname(filename),
        fmt = toFormat(ext);
    if (!fmt){
      throw new Error(`Unsupported file format "${ext}" (expected "png", "jpg", "pdf", or "svg")`)
    }
    ƒ(this, "save_as", filename, seq, fmt, quality)
  }

  toBuffer(extension, {format="png", quality=100, page}={}){
    ({format, quality, page} = Object.assign(
      {format, quality, page},
      typeof extension == 'string' ? {format:extension}
    : typeof extension == 'object' ? extension
    : {}
    ));

    let fmt = toFormat(format),
        pp = this.pages.length,
        idx = page >= 0 ? pp - page
            : page < 0 ? pp + page
            : undefined

    if (!fmt){
      throw new Error(`Unsupported file format "${format}" (expected "png", "jpg", "pdf", or "svg")`)
    }else if (isFinite(idx) && idx < 0 || idx >= pp){
      throw new RangeError(
        pp == 1 ? `Canvas only has a ‘page 1’ (${page} is out of bounds)`
                : `Canvas has pages 1–${pp} (${page} is out of bounds)`
      )
    }

    return ƒ(this, "to_buffer", fmt, quality, idx)
  }

  toDataURL(extension, {format="png", quality=100, page}={}){
    ({format, quality, page} = Object.assign(
      {format, quality, page},
      typeof extension == 'string' ? {format:extension}
    : typeof extension == 'object' ? extension
    : {}
    ));

    let fmt = toFormat(format),
        mime = toMime(fmt),
        buffer = this.toBuffer({format, quality, page});
    return `data:${mime};base64,${buffer.toString('base64')}`
  }

  [REPR](depth, options) {
    let {width, height} = this
    return `Canvas ${inspect({width, height}, options)}`
  }
}

class CanvasGradient{
  constructor(style, ...coords){
    style = (style || "").toLowerCase()
    if (style=='linear' || style=='radial') init(this, style, ...coords)
    else throw new Error("Function is not a constructor (use CanvasRenderingContext2D's \"createLinearGradient\" and \"createRadialGradient\" methods instead)")
  }

  addColorStop(offset, color){
    if (offset>=0 && offset<=1) ƒ(this, 'add_color_stop', offset, color)
    else throw new Error("Color stop offsets must be between 0.0 and 1.0")
  }
}

class CanvasPattern{
  constructor(src, repeat){
    if (src instanceof Image){
      init(this, 'from_image', core(src), repeat)
    }else if (src instanceof Canvas){
      init(this, 'from_canvas', core(src), repeat)
    }else{
      throw new Error("CanvasPatterns require a source Image or a Canvas")
    }
  }

  setTransform(matrix){
    if (arguments.length>1) matrix = [...arguments]
    ƒ(this, 'set_transform', toSkMatrix(matrix) )
  }
}

class CanvasRenderingContext2D{
  constructor(width, height){
    alloc(width, height)
  }

  get canvas(){ return Canvas.parent.get(this) }
  save(){ ƒ(this, 'save') }
  restore(){ ƒ(this, 'restore') }

  get currentTransform(){ return fromSkMatrix( ƒ(this, 'get_current_transform') ) }
  set currentTransform(matrix){  ƒ(this, 'set_current_transform', toSkMatrix(matrix) ) }
  getTransform(){ return this.currentTransform }
  setTransform(matrix){
    this.currentTransform = arguments.length > 1 ? [...arguments] : matrix
  }
  transform(...terms){ ƒ(this, 'transform', ...terms)}
  translate(x, y){ ƒ(this, 'translate', x, y)}
  scale(x, y){ ƒ(this, 'scale', x, y)}
  rotate(angle){ ƒ(this, 'rotate', angle)}
  reset_transform(){ ƒ(this, 'reset_transform')}

  clip(path, rule){
    if (path instanceof Path2D){
      ƒ(this, 'clip', core(path), rule)
    }else{
      ƒ(this, 'clip', path)
    }
  }


  getImageData(x, y, width, height){
    let w = Math.floor(width),
        h = Math.floor(height),
        buffer = ƒ(this, 'getImageData', x, y, w, h);
    return new ImageData(w, h, buffer)
  }

  putImageData(imageData, ...coords){
    ƒ(this, 'putImageData', imageData, ...coords)
  }

  // ------


  get font(){ return ƒ(this, 'get_font') }
  set font(str){ ƒ(this, 'set_font', parseFont(str)) }
  get fontVariant(){ return ƒ(this, 'get_fontVariant') }
  set fontVariant(str){ ƒ(this, 'set_fontVariant', parseVariant(str)) }

  measureText(text, ...args){
    let [metrics, ...lines] = ƒ(this, 'measureText', toString(text), ...args)
    return new TextMetrics(metrics, lines)
  }

  fillText(text, ...args){
    ƒ(this, 'fillText', toString(text), ...args)
  }

  strokeText(text, ...args){
    ƒ(this, 'strokeText', toString(text), ...args)
  }

  get filter(){ return ƒ(this, 'get_filter') }
  set filter(str){ ƒ(this, 'set_filter', parseFilter(str)) }
  createImageData(width, height){ return new ImageData(width, height) }
  getImageData(...args){ return new ImageData( ƒ(this, 'getImageData', ...args) ) }

  createLinearGradient(...args){ return new CanvasGradient("Linear", ...args) }
  createRadialGradient(...args){ return new CanvasGradient("Radial", ...args) }
  createPattern(...args){ return new CanvasPattern(...args) }

  [REPR](depth, options) {
    let props = [ "canvas", "currentTransform", "fillStyle", "strokeStyle", "filter", "font", "direction", "textAlign", "textBaseline",
                  "globalAlpha", "globalCompositeOperation", "imageSmoothingEnabled", "imageSmoothingQuality", "shadowBlur", "shadowColor",
                  "shadowOffsetX", "shadowOffsetY", "lineCap", "lineDashOffset", "lineJoin", "lineWidth", "miterLimit" ]
    let info = {}
    if (depth > 0 ){
      for (var prop of props){
        try{ info[prop] = this[prop] }
        catch{ info[prop] = undefined }
      }
    }
    return `CanvasRenderingContext2D ${inspect(info, options)}`
  }
}

const _expand = paths => [paths].flat(2).map(filename => glob(filename)).flat()

class FontLibrary{
  get families(){ return ƒ(this, 'get_families') }

  has(familyName){ return ƒ(this, 'has', familyName) }

  family(name){ return ƒ(this, 'family', name) }

  use(...args){
    let sig = signature(args)
    if (sig=='o'){
      let results = {}
      for (let [alias, paths] of Object.entries(args.shift())){
        results[alias] = ƒ(this, "add_family", alias, _expand(paths))
      }
      return results
    }else if (sig.match(/^s?[as]$/)){
      let fonts = _expand(args.pop())
      let alias = args.shift()
      return ƒ(this, "add_family", alias, fonts)
    }else{
      throw new Error("Expected an array of file paths or an object mapping family names to font files")
    }
  }
}

class Image {
  constructor(){ alloc(this) }

  get complete(){ return ƒ(this, 'get_complete') }
  get height(){ return ƒ(this, 'get_height') }
  get width(){ return ƒ(this, 'get_width') }

  get src(){ return ƒ(this, 'get_src') }
  set src(src){
    // return ƒ(this, 'set_src', src)
    var data

    if (Buffer.isBuffer(src)){
      [data, src] = [src, '']
    } else if (typeof src != 'string'){
      return
    } else if (/^\s*data:/.test(src)) {
      // data URI
      let split = src.indexOf(','),
          enc = src.lastIndexOf('base64', split) !== -1 ? 'base64' : 'utf8',
          content = src.slice(split + 1);
      data = Buffer.from(content, enc);
    } else if (/^\s*https?:\/\//.test(src)) {
      // remote URL
      get.concat(src, (err, res, data) => {
        let code = res.statusCode,
            onerror = this.onerror || (() => {}),
            onload = this.onload || (() => {});
        if (err) onerror(err)
        else if (code < 200 || code >= 300) {
          onerror(new Error(`Failed to load image from "${src}" (error ${code})`))
        }else{
          if (ƒ(this, "set_data", data)) onload(this)
          else onerror(new Error("Could not decode image data"))
        }
      })
    } else {
      // local file path
      data = fs.readFileSync(src);
    }

    ƒ(this, "set_src", src)
    if (data){
      let onerror = this.onerror || (() => {}), onload = this.onload || (() => {});
      if (ƒ(this, "set_data", data)) onload(this)
      else onerror(new Error("Could not decode image data"))
    }

  }

  [REPR](depth, options) {
    let {width, height, complete, src} = this
    options.maxStringLength = src.match(/^data:/) ? 128 : Infinity;
    return `Image ${inspect({width, height, complete, src}, options)}`
  }
}

class ImageData{
  constructor(width, height, data){
    if (arguments[0] instanceof ImageData){
      var {width, height, data} = arguments[0]
    }

    if (!Number.isInteger(width) || !Number.isInteger(height) || width < 0 || height < 0){
      throw new Error("ImageData dimensions must be positive integers")
    }

    readOnly(this, "width", width)
    readOnly(this, "height", height)
    readOnly(this, "data", new Uint8ClampedArray(data && data.buffer || width * height * 4))
  }

  [REPR](depth, options) {
    let {width, height, data} = this
    return `ImageData ${inspect({width, height, data}, options)}`
  }
}

class Path2D{
  constructor(source){
    if (source instanceof Path2D) init(this, 'from_path2d', core(source))
    else if (typeof source == 'string') init(this, 'from_svg', source)
    else alloc(this)
  }

  addPath(path, matrix){
    if (!path instanceof Path2D) throw new Error("Expected a Path2D object")
    if (matrix) ƒ(this, 'add_path_matrix', core(path), toSkMatrix(matrix) )
    else ƒ(this, 'add_path', core(path))
  }

  get bounds(){ return ƒ(this, 'bounds')}

  // boolean operations
  difference(path){
    return inject(new Path2D(), ƒ(this, "op", core(path), "difference"))
  }

  intersect(path){
    return inject(new Path2D(), ƒ(this, "op", core(path), "intersect"))
  }

  union(path){
    return inject(new Path2D(), ƒ(this, "op", core(path), "union"))
  }

  xor(path){
    return inject(new Path2D(), ƒ(this, "op", core(path), "xor"))
  }

  complement(path){
    return inject(new Path2D(), ƒ(this, "op", core(path), "complement"))
  }
}

class TextMetrics{
  constructor([
    width, left, right, ascent, descent,
    fontAscent, fontDescent, emAscent, emDescent,
    hanging, alphabetic, ideographic
  ], lines){
    readOnly(this, "width", width)
    readOnly(this, "actualBoundingBoxLeft", left)
    readOnly(this, "actualBoundingBoxRight", right)
    readOnly(this, "actualBoundingBoxAscent", ascent)
    readOnly(this, "actualBoundingBoxDescent", descent)
    readOnly(this, "fontBoundingBoxAscent", fontAscent)
    readOnly(this, "fontBoundingBoxDescent", fontDescent)
    readOnly(this, "emHeightAscent", emAscent)
    readOnly(this, "emHeightDescent", emDescent)
    readOnly(this, "hangingBaseline", hanging)
    readOnly(this, "alphabeticBaseline", alphabetic)
    readOnly(this, "ideographicBaseline", ideographic)
    readOnly(this, "lines", lines.map( ([x, y, width, height, baseline, startIndex, endIndex]) => (
      {x, y, width, height, baseline, startIndex, endIndex}
    )))
  }
}

module.exports = {Canvas, CanvasGradient, CanvasPattern,
                  CanvasRenderingContext2D, FontLibrary:new FontLibrary(),
                  Image, ImageData, Path2D, TextMetrics}