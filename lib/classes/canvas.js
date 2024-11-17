//
// Canvas object & export options
//

"use strict"

const {RustClass, core, inspect, REPR} = require('./neon'),
      {Image, ImageData, pixelSize} = require('./imagery'),
      {toSkMatrix} = require('./geometry')

class Canvas extends RustClass{
  #contexts

  constructor(width, height){
    super(Canvas).alloc()
    this.#contexts = []
    Object.assign(this, {width, height})
  }

  getContext(kind){
    return (kind=="2d") ? this.#contexts[0] || this.newPage() : null
  }

  get gpu(){ return this.prop('engine')=='gpu' }
  set gpu(mode){ this.prop('engine', !!mode ? 'gpu' : 'cpu') }

  get engine(){ return JSON.parse(this.prop('engine_status')) }

  get width(){ return this.prop('width') }
  set width(w){
    this.prop('width', (typeof w=='number' && !Number.isNaN(w) && w>=0) ? w : 300)
    if (this.#contexts[0]) this.getContext("2d").ƒ('resetSize', core(this))
  }

  get height(){ return this.prop('height') }
  set height(h){
    this.prop('height', h = (typeof h=='number' && !Number.isNaN(h) && h>=0) ? h : 150)
    if (this.#contexts[0]) this.getContext("2d").ƒ('resetSize', core(this))
  }

  newPage(width, height){
    const {CanvasRenderingContext2D} = require('./context')
    let ctx = new CanvasRenderingContext2D(this)
    this.#contexts.unshift(ctx)
    if (arguments.length==2){
      Object.assign(this, {width, height})
    }
    return ctx
  }

  get pages(){
    return this.#contexts.slice().reverse()
  }

  get png(){ return this.toBuffer("png") }
  get jpg(){ return this.toBuffer("jpg") }
  get pdf(){ return this.toBuffer("pdf") }
  get svg(){ return this.toBuffer("svg") }
  get webp(){ return this.toBuffer("webp") }

  saveAs(filename, opts={}){
    opts = typeof opts=='number' ? {quality:opts} : opts
    let {format, quality, pages, padding, pattern, density, outline, matte, msaa, colorType} = exportOptions(this.pages, {filename, ...opts}),
        args = [pages.map(core), pattern, padding, {format, quality, density, outline, matte, msaa, colorType}]
    return this.ƒ("save", ...args)
  }

  saveAsSync(filename, opts={}){
    opts = typeof opts=='number' ? {quality:opts} : opts
    let {format, quality, pages, padding, pattern, density, outline, matte, msaa, colorType} = exportOptions(this.pages, {filename, ...opts})
    this.ƒ("saveSync", pages.map(core), pattern, padding, {format, quality, density, outline, matte, msaa, colorType})
  }

  toBuffer(extension="png", opts={}){
    opts = typeof opts=='number' ? {quality:opts} : opts
    let {format, quality, pages, density, outline, matte, msaa, colorType} = exportOptions(this.pages, {extension, ...opts}),
        args = [pages.map(core), {format, quality, density, outline, matte, msaa, colorType}];
    return this.ƒ("toBuffer", ...args)
  }

  toBufferSync(extension="png", opts={}){
    opts = typeof opts=='number' ? {quality:opts} : opts
    let {format, quality, pages, density, outline, matte, msaa, colorType} = exportOptions(this.pages, {extension, ...opts})
    return this.ƒ("toBufferSync", pages.map(core), {format, quality, density, outline, matte, msaa, colorType})
  }

  toDataURL(extension="png", opts={}){
    opts = typeof opts=='number' ? {quality:opts} : opts
    let {mime} = exportOptions(this.pages, {extension, ...opts}),
        buffer = this.toBuffer(extension, opts);
    return buffer.then(data => `data:${mime};base64,${data.toString('base64')}`)
  }

  toDataURLSync(extension="png", opts={}){
    opts = typeof opts=='number' ? {quality:opts} : opts
    let {mime} = exportOptions(this.pages, {extension, ...opts}),
        buffer = this.toBufferSync(extension, opts);
    return `data:${mime};base64,${buffer.toString('base64')}`
  }


  [REPR](depth, options) {
    let {width, height, gpu, engine, pages} = this
    return `Canvas ${inspect({width, height, gpu, engine, pages}, options)}`
  }
}

class CanvasGradient extends RustClass{
  constructor(style, ...coords){
    super(CanvasGradient)
    style = (style || "").toLowerCase()
    if (['linear', 'radial', 'conic'].includes(style)) this.init(style, ...coords)
    else throw new Error(`Function is not a constructor (use CanvasRenderingContext2D's "createConicGradient", "createLinearGradient", and "createRadialGradient" methods instead)`)
  }

  addColorStop(offset, color){
    if (offset>=0 && offset<=1) this.ƒ('addColorStop', offset, color)
    else throw new Error("Color stop offsets must be between 0.0 and 1.0")
  }

  [REPR](depth, options) {
    return `CanvasGradient (${this.ƒ("repr")})`
  }
}

class CanvasPattern extends RustClass{
  constructor(canvas, src, repeat){
    super(CanvasPattern)
    if (src instanceof Image){
      let {width, height} = canvas
      this.init('from_image', core(src), width, height, repeat)
    }else if (src instanceof ImageData){
      this.init('from_image_data', src, repeat)
    }else if (src instanceof Canvas){
      let ctx = src.getContext('2d')
      this.init('from_canvas', core(ctx), repeat)
    }else{
      throw new Error("CanvasPatterns require a source Image or a Canvas")
    }
  }

  setTransform(matrix) { this.ƒ('setTransform', toSkMatrix.apply(null, arguments)) }

  [REPR](depth, options) {
    return `CanvasPattern (${this.ƒ("repr")})`
  }
}

class CanvasTexture extends RustClass{
  constructor(spacing, {path, line, color, angle, offset=0}={}){
    super(CanvasTexture)
    let [x, y] = typeof offset=='number' ? [offset, offset] : offset.slice(0, 2)
    let [h, v] = typeof spacing=='number' ? [spacing, spacing] : spacing.slice(0, 2)
    path = core(path)
    line = line != null ? line : (path ? 0 : 1)
    angle = angle != null ? angle : (path ? 0 : -Math.PI / 4)
    this.alloc(path, color, line, angle, h, v, x, y)
  }

  [REPR](depth, options) {
    return `CanvasTexture (${this.ƒ("repr")})`
  }
}


//
// Mime type <-> File extension mappings
//

class Format{
  constructor(){
    let png = "image/png",
        jpg = "image/jpeg",
        jpeg = "image/jpeg",
        webp = "image/webp",
        pdf = "application/pdf",
        svg = "image/svg+xml",
        raw = "application/octet-stream"

    Object.assign(this, {
      toMime: this.toMime.bind(this),
      fromMime: this.fromMime.bind(this),
      expected: `"png", "jpg", "webp", "raw", "pdf", or "svg"`,
      formats: {png, jpg, jpeg, webp, raw, pdf, svg},
      mimes: {[png]: "png", [jpg]: "jpg", [webp]: "webp", [raw]: "raw", [pdf]: "pdf", [svg]: "svg"},
    })
  }

  toMime(ext){
    return this.formats[(ext||'').replace(/^\./, '').toLowerCase()]
  }

  fromMime(mime){
    return this.mimes[mime]
  }
}

//
// Validation of the options dict shared by the `saveAs`, `toBuffer`, and `toDataURL` methods
//

const {basename, extname} = require('path')

function exportOptions(pages, {filename='', extension='', format, page, quality, matte, density, outline, msaa, colorType}={}){
  var {fromMime, toMime, expected} = new Format(),
      ext = format || extension.replace(/@\d+x$/i,'') || extname(filename),
      format = fromMime(toMime(ext) || ext),
      mime = toMime(format),
      pp = pages.length

  if (!ext) throw new Error(`Cannot determine image format (use a filename extension or 'format' argument)`)
  if (!format) throw new Error(`Unsupported file format "${ext}" (expected ${expected})`)
  if (!pp) throw new RangeError(`Canvas has no associated contexts (try calling getContext or newPage first)`)

  let padding, isSequence, pattern = filename.replace(/{(\d*)}/g, (_, width) => {
    isSequence = true
    width = parseInt(width, 10)
    padding = isFinite(width) ? width : isFinite(padding) ? padding : -1
    return "{}"
  })

  // allow negative indexing if a specific page is specified
  let idx = page > 0 ? page - 1
          : page < 0 ? pp + page
          : undefined;

  if (isFinite(idx) && idx < 0 || idx >= pp) throw new RangeError(
    pp == 1 ? `Canvas only has a ‘page 1’ (${idx} is out of bounds)`
            : `Canvas has pages 1–${pp} (${idx} is out of bounds)`
  )

  pages = isFinite(idx) ? [pages[idx]]
        : isSequence || format=='pdf' ? pages
        : pages.slice(-1) // default to the 'current' context

  if (quality===undefined){
    quality = 0.92
  }else{
    if (typeof quality!='number' || !isFinite(quality) || quality<0 || quality>1){
      throw new TypeError("The quality option must be an number in the 0.0–1.0 range")
    }
  }

  if (density===undefined){
    let m = (extension || basename(filename, ext)).match(/@(\d+)x$/i)
    density = m ? parseInt(m[1], 10) : 1
  }else if (typeof density!='number' || !Number.isInteger(density) || density<1){
    throw new TypeError("The density option must be a non-negative integer")
  }

  if (outline===undefined){
    outline = true
  }else if (format == 'svg'){
    outline = !!outline
  }

  if (msaa===undefined) {
    msaa = undefined // leave as-is
  }else if (!msaa){
    msaa = 0 // null, false, etc. should all disable it
  }else if (typeof msaa!='number' || !isFinite(msaa) || msaa<0){
    throw new TypeError("The number of MSAA samples must be an integer ≥0")
  }

  if (colorType!==undefined){
    pixelSize(colorType) // throw an error if invalid
  }

  return {filename, pattern, format, mime, pages, padding, quality, matte, density, outline, msaa, colorType}
}

module.exports = {Canvas, CanvasGradient, CanvasPattern, CanvasTexture}
