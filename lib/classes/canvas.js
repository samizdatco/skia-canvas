//
// Canvas object & export options
//

"use strict"

const {fileURLToPath} = require('url'),
      {RustClass, core, inspect, argc, REPR} = require('./neon'),
      {Image, ImageData, pixelSize, getSharp} = require('./imagery'),
      {Path2D} = require('./path'),
      {toSkMatrix} = require('./geometry')

class Canvas extends RustClass{
  #contexts

  constructor(width, height, {textContrast=0, textGamma=1.4}={}){
    super(Canvas).alloc({textContrast, textGamma})
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
    this.prop('width', !Number.isNaN(+w) && +w>=0 ? w : 300)
    if (this.#contexts[0]) this.getContext("2d").ƒ('resetSize', core(this))
  }

  get height(){ return this.prop('height') }
  set height(h){
    this.prop('height', !Number.isNaN(+h) && +h>=0 ? h : 150)
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
    let {pages, padding, pattern, ...rest} = exportOptions(this, {filename, ...opts}),
        args = [pages.map(core), pattern, padding, rest]
    return this.ƒ("save", ...args)
  }

  saveAsSync(filename, opts={}){
    opts = typeof opts=='number' ? {quality:opts} : opts
    let {pages, padding, pattern, ...rest} = exportOptions(this, {filename, ...opts})
    this.ƒ("saveSync", pages.map(core), pattern, padding, rest)
  }

  toBuffer(extension="png", opts={}){
    opts = typeof opts=='number' ? {quality:opts} : opts
    let {pages, ...rest} = exportOptions(this, {extension, ...opts})
    return this.ƒ("toBuffer", pages.map(core), rest)
  }

  toBufferSync(extension="png", opts={}){
    opts = typeof opts=='number' ? {quality:opts} : opts
    let {pages, ...rest} = exportOptions(this, {extension, ...opts})
    return this.ƒ("toBufferSync", pages.map(core), rest)
  }

  toDataURL(extension="png", opts={}){
    opts = typeof opts=='number' ? {quality:opts} : opts
    let {mime} = exportOptions(this, {extension, ...opts}),
        buffer = this.toBuffer(extension, opts);
    return buffer.then(data => `data:${mime};base64,${data.toString('base64')}`)
  }

  toDataURLSync(extension="png", opts={}){
    opts = typeof opts=='number' ? {quality:opts} : opts
    let {mime} = exportOptions(this, {extension, ...opts}),
        buffer = this.toBufferSync(extension, opts);
    return `data:${mime};base64,${buffer.toString('base64')}`
  }

  toSharp({page, matte, msaa, density=1}={}){
    const {Readable} = require('node:stream'),
          sharp = getSharp(),
          buffer = this.toBuffer("raw", {page, matte, density, msaa})

    return Readable.from(
      (async function * (){ yield buffer })()
    ).pipe(sharp({
      raw: {width:this.width*density, height:this.height*density, channels:4}
    }).withMetadata({density:density * 72}))
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
    this.ƒ('addColorStop', ...arguments)
  }

  [REPR](depth, options) {
    return `CanvasGradient (${this.ƒ("repr")})`
  }
}

class CanvasPattern extends RustClass{
  constructor(canvas, src, repeat){
    repeat = [...arguments].slice(2)
    super(CanvasPattern)
    if (src instanceof Image){
      let {width, height} = canvas
      this.init('from_image', core(src), width, height, ...repeat)
    }else if (src instanceof ImageData){
      this.init('from_image_data', src, ...repeat)
    }else if (src instanceof Canvas){
      let ctx = src.getContext('2d')
      this.init('from_canvas', core(ctx), ...repeat)
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
  constructor(spacing, {path, color, angle, line, cap="butt", outline=false, offset=0}={}){
    super(CanvasTexture)
    argc(arguments, 1)
    let [x, y] = Array.isArray(offset) ? offset.concat(offset).slice(0, 2) : [offset, offset]
    let [h, v] = Array.isArray(spacing) ? spacing.concat(spacing).slice(0, 2) : [spacing, spacing]
    if (path!==undefined && !(path instanceof Path2D)){
      throw TypeError("Expected a Path2D object for `path`")
    }
    path = core(path)
    line = line != null ? line : (path ? 0 : 1)
    angle = angle != null ? angle : (path ? 0 : -Math.PI / 4)
    this.alloc(path, color, line, cap, angle, !!outline, h, v, x, y)
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

function exportOptions(canvas, {
  filename='', extension='', format, page, quality, matte, density, msaa, outline, downsample, colorType
}={}){
  if (filename instanceof URL){
    if (filename.protocol=='file:') filename = fileURLToPath(filename)
    else throw Error(`URLs must use 'file' protocol (got '${filename.protocol.replace(':', '')}')`)
  }

  var {fromMime, toMime, expected} = new Format(),
      ext = format || extension.replace(/@\d+x$/i,'') || extname(filename),
      format = fromMime(toMime(ext) || ext),
      mime = toMime(format),
      pages = canvas.pages,
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

  // inherit text settings from the canvas (since they can't be changed on a per-render basis due to glyph caching)
  const {textContrast, textGamma} = canvas.engine

  if (quality===undefined){
    quality = 0.92
  }else{
    if (typeof quality!='number' || !isFinite(quality) || quality<0 || quality>1){
      throw new TypeError("Expected a number between 0.0–1.0 for `quality`")
    }
  }

  if (density===undefined){
    let m = (extension || basename(filename, ext)).match(/@(\d+)x$/i)
    density = m ? parseInt(m[1], 10) : 1
  }else if (typeof density!='number' || !Number.isInteger(density) || density<1){
    throw new TypeError("Expected a non-negative integer for `density`")
  }

  if (msaa===undefined || msaa===true) {
    msaa = undefined // use the default 4x msaa
  }else if (!isFinite(+msaa) || +msaa<0){
    throw new TypeError("The number of MSAA samples must be an integer ≥0")
  }

  if (colorType!==undefined){
    pixelSize(colorType) // throw an error if invalid
  }

  // default to false, otherwise detect truthy
  downsample = !!downsample
  outline = !!outline

  return {
    filename, pattern, format, mime, pages, padding, quality, matte,
    density, msaa, outline, textContrast, textGamma, downsample, colorType
  }
}

module.exports = {Canvas, CanvasGradient, CanvasPattern, CanvasTexture, getSharp}
