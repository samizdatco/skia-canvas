#!/usr/bin/env node

const fs = require('fs'),
      {inspect} = require('util'),
      {extname} = require('path'),
      glob = require('glob').sync,
      get = require('simple-get'),
      crate = require('./v6/index.node'),
      REPR = inspect.custom

//
// Neon <-> Node interface
//

const ø = Symbol.for('self'), // the attr containing the boxed struct
      core = (obj) => (obj||{})[ø] // dereference the boxed struct

class RustClass{
  alloc(...args){
    this.init('new', ...args)
  }

  init(fn, ...args){
    this[ø] = crate[`${this.constructor.name}_${fn}`](null, ...args)
  }

  hatch(boxed, ...args){
    return Object.assign(new this.constructor(...args), {[ø]:boxed})
  }

  cache(verb, key, val){
    if (verb=='set') this[Symbol.for(key)] = val
    else if (verb=='get') return this[Symbol.for(key)]
  }

  ƒ(fn, ...args){
    return crate[`${this.constructor.name}_${fn}`](this[ø], ...args);
  }
}

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



class Canvas extends RustClass{
  static parent = new WeakMap()
  static context = new WeakMap()

  constructor(width, height){
    super()
    this.alloc(width, height)

    // let ctx = new CanvasRenderingContext2D(width, height)
    // Canvas.parent.set(ctx, this)
    // Canvas.context.set(this, [ctx])
  }

  getContext(kind){
    return (kind=="2d") ? Canvas.context.get(this)[0] : null
  }

  get width(){ return this.ƒ('get_width') }
  get height(){ return this.ƒ('get_height') }
  set width(w){ return this.ƒ('set_width', w) }
  set height(h){ return this.ƒ('set_height', h) }

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
    this.ƒ("saveAs", filename, seq, fmt, quality)
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

    return this.ƒ("toBuffer", fmt, quality, idx)
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

class CanvasGradient extends RustClass{
  constructor(style, ...coords){
    super()
    style = (style || "").toLowerCase()
    if (style=='linear' || style=='radial') this.init(style, ...coords)
    else throw new Error(`Function is not a constructor (use CanvasRenderingContext2D's "createLinearGradient" and "createRadialGradient" methods instead)`)
  }

  addColorStop(offset, color){
    if (offset>=0 && offset<=1) this.ƒ('addColorStop', offset, color)
    else throw new Error("Color stop offsets must be between 0.0 and 1.0")
  }
}

class CanvasPattern extends RustClass{
  constructor(src, repeat){
    super()
    if (src instanceof Image){
      this.init('from_image', core(src), repeat)
    }else if (src instanceof Canvas){
      this.init('from_canvas', core(src), repeat)
    }else{
      throw new Error("CanvasPatterns require a source Image or a Canvas")
    }
  }

  setTransform(matrix){
    if (arguments.length>1) matrix = [...arguments]
    this.ƒ('setTransform', toSkMatrix(matrix))
  }
}

class CanvasRenderingContext2D extends RustClass{
  constructor(width, height){
    super()
    this.alloc(width, height)
  }

  get canvas(){ return Canvas.parent.get(this) }


  // -- grid state ------------------------------------------------------------
  save(){ this.ƒ('save') }
  restore(){ this.ƒ('restore') }

  get currentTransform(){ return fromSkMatrix( this.ƒ('get_currentTransform') ) }
  set currentTransform(matrix){  this.ƒ('set_currentTransform', toSkMatrix(matrix) ) }

  getTransform(){ return this.currentTransform }
  setTransform(matrix){
    this.currentTransform = arguments.length > 1 ? [...arguments] : matrix
  }
  transform(...terms){ this.ƒ('transform', ...terms)}
  translate(x, y){ this.ƒ('translate', x, y)}
  scale(x, y){ this.ƒ('scale', x, y)}
  rotate(angle){ this.ƒ('rotate', angle)}
  resetTransform(){ this.ƒ('resetTransform')}

  // -- bézier paths ----------------------------------------------------------
  beginPath(){ this.ƒ('beginPath') }
  rect(x, y, width, height){ this.ƒ('rect', ...argumments) }
  arc(x, y, radius, startAngle, endAngle, isCCW){ this.ƒ('arc', ...argumments) }
  ellipse(x, y, xRadius, yRadius, rotation, startAngle, endAngle, isCCW){ this.ƒ('ellipse', ...argumments) }
  moveTo(x, y){ this.ƒ('moveTo', x, y) }
  lineTo(x, y){ this.ƒ('lineTo', x, y) }
  arcTo(x1, y1, x2, y2, radius){ this.ƒ('arcTo', ...argumments) }
  bezierCurveTo(cp1x, cp1y, cp2x, cp2y, x, y){ this.ƒ('bezierCurveTo', ...arguments) }
  quadraticCurveTo(cpx, cpy, x, y){ this.ƒ('quadraticCurveTo', ...arguments) }
  closePath(){ this.ƒ('closePath') }
  isPointInPath(x, y){ return this.ƒ('isPointInPath', x, y) }
  isPointInStroke(x, y){ return this.ƒ('isPointInStroke', x, y) }

  clip(path, rule){
    if (path instanceof Path2D){
      this.ƒ('clip', core(path), rule)
    }else{
      this.ƒ('clip', path)
    }
  }

  // -- fill & stroke ---------------------------------------------------------
  fill(){ this.ƒ('fill', ...arguments) }
  stroke(){ this.ƒ('stroke', ...arguments) }
  fillRect(x, y, width, height){ this.ƒ('fillRect', ...arguments) }
  strokeRect(x, y, width, height){ this.ƒ('strokeRect', ...arguments) }
  clearRect(x, y, width, height){ this.ƒ('clearRect', ...arguments) }

  createPattern(image, repetition){ return new CanvasPattern(...arguments) }
  createLinearGradient(x0, y0, x1, y1){
    return new CanvasGradient("Linear", ...arguments)
  }
  createRadialGradient(x0, y0, r0, x1, y1, r1){
    return new CanvasGradient("Radial", ...arguments)
  }
  createConicGradient(startAngle, x, y){}


  set fillStyle(style){
    let isShader = style instanceof CanvasPattern || style instanceof CanvasGradient,
        [ref, val] = isShader ? [style, core(style)] : [null, style]
    this.cache('set', 'fill', ref)
    this.ƒ('set_fillStyle', val)
  }

  get fillStyle(){
    let style = this.ƒ('get_fillStyle')
    return style===null ? this.cache('get', 'fill') : style
  }

  set strokeStyle(style){
    let isShader = style instanceof CanvasPattern || style instanceof CanvasGradient,
        [ref, val] = isShader ? [style, core(style)] : [null, style]
    this.cache('set', 'stroke', ref)
    this.ƒ('set_strokeStyle', val)
  }

  get strokeStyle(){
    let style = this.ƒ('get_strokeStyle')
    return style===null ? this.cache('get', 'stroke') : style
  }

  // -- line style ------------------------------------------------------------
  // -- imagery ---------------------------------------------------------------
  createImageData(width, height){ return new ImageData(width, height) }
  getImageData(x, y, width, height){
    let w = Math.floor(width),
        h = Math.floor(height),
        buffer = this.ƒ('getImageData', x, y, w, h);
    return new ImageData(w, h, buffer)
  }

  putImageData(imageData, ...coords){
    this.ƒ('putImageData', imageData, ...coords)
  }

  // -- typography ------------------------------------------------------------
  get font(){ return this.ƒ('get_font') }
  set font(str){ this.ƒ('set_font', parseFont(str)) }
  get fontVariant(){ return this.ƒ('get_fontVariant') }
  set fontVariant(str){ this.ƒ('set_fontVariant', parseVariant(str)) }

  measureText(text, ...args){
    let [metrics, ...lines] = this.ƒ('measureText', toString(text), ...args)
    return new TextMetrics(metrics, lines)
  }

  fillText(text, ...args){
    this.ƒ('fillText', toString(text), ...args)
  }

  strokeText(text, ...args){
    this.ƒ('strokeText', toString(text), ...args)
  }

  // -- effects ---------------------------------------------------------------
  get filter(){ return this.ƒ('get_filter') }
  set filter(str){ this.ƒ('set_filter', parseFilter(str)) }


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

class FontLibrary extends RustClass {
  get families(){ return this.ƒ('get_families') }

  has(familyName){ return this.ƒ('has', familyName) }

  family(name){ return this.ƒ('family', name) }

  use(...args){
    let sig = signature(args)
    if (sig=='o'){
      let results = {}
      for (let [alias, paths] of Object.entries(args.shift())){
        results[alias] = this.ƒ("addFamily", alias, _expand(paths))
      }
      return results
    }else if (sig.match(/^s?[as]$/)){
      let fonts = _expand(args.pop())
      let alias = args.shift()
      return this.ƒ("addFamily", alias, fonts)
    }else{
      throw new Error("Expected an array of file paths or an object mapping family names to font files")
    }
  }
}

class Image extends RustClass {
  constructor(){
    super()
    this.alloc()
  }

  get complete(){ return this.ƒ('get_complete') }
  get height(){ return this.ƒ('get_height') }
  get width(){ return this.ƒ('get_width') }

  get src(){ return this.ƒ('get_src') }
  set src(src){
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
          if (this.ƒ("set_data", data)) onload(this)
          else onerror(new Error("Could not decode image data"))
        }
      })
    } else {
      // local file path
      data = fs.readFileSync(src);
    }

    this.ƒ("set_src", src)
    if (data){
      let onerror = this.onerror || (() => {}), onload = this.onload || (() => {});
      if (this.ƒ("set_data", data)) onload(this)
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

class Path2D extends RustClass{
  constructor(source){
    super()
    if (source instanceof Path2D) this.init('from_path', core(source))
    else if (typeof source == 'string') this.init('from_svg', source)
    else this.alloc()
  }

  // measure dimensions
  get bounds(){ return this.ƒ('bounds') }

  // concatenation
  addPath(path, matrix){
    if (!(path instanceof Path2D)) throw new Error("Expected a Path2D object")
    if (matrix) matrix = toSkMatrix(matrix)
    this.ƒ('addPath', core(path), matrix)
  }

  // line segments
  moveTo(x, y){   this.ƒ("moveTo", x, y) }
  lineTo(x, y){   this.ƒ("lineTo", x, y) }
  arcTo(...args){ this.ƒ("arcTo", ...args) }
  closePath(){    this.ƒ("closePath") }

  // curves
  bezierCurveTo(...args){    this.ƒ("bezierCurveTo", ...args) }
  quadraticCurveTo(...args){ this.ƒ("quadraticCurveTo", ...args) }

  // shape primitives
  ellipse(...args){ this.ƒ("ellipse", ...args) }
  rect(...args){    this.ƒ("rect", ...args) }
  arc(...args){     this.ƒ("arc", ...args) }

  // boolean operations
  complement(path){ return this.hatch(this.ƒ("op", core(path), "complement")) }
  difference(path){ return this.hatch(this.ƒ("op", core(path), "difference")) }
  intersect(path){  return this.hatch(this.ƒ("op", core(path), "intersect")) }
  union(path){      return this.hatch(this.ƒ("op", core(path), "union")) }
  xor(path){        return this.hatch(this.ƒ("op", core(path), "xor")) }

  // elide overlaps
  simplify(){       return this.hatch(this.ƒ('simplify')) }
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