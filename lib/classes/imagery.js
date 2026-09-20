//
// Image & ImageData
//

"use strict"

const {RustClass, core, readOnly, inspect, neon, argc, REPR, STRICT} = require('./neon'),
      {fetchURL, decodeDataURL, expandURL} = require('../urls'),
      {EventEmitter} = require('events'),
      {readFile} = require('fs/promises')

//
// Image
//

const DecodingError = () => new Error("Could not decode image data")
const DisposalError = () => new Error("Image was disposed before loading completed")
const STALE = Symbol("stale request") // decode result for a superseded request: drop with no event/rejection
const CORRUPT = Symbol("corrupt data") // decode result for undecodable data: reject with DecodingError

const loadImage = (src, args) => new Promise((res, rej) => {
  let {page, ...options} = args || {}, // separate the pdf page selection from the http request options
      pageNum = page==null ? undefined : Number.isFinite(Number(page)) ? Number(page) : -1
  fetchData(src, options, res, rej,
    (data, src, raw) => {
      let img = new Image()
      img.prop('src', src)
      return img.prop('data', data, raw, pageNum) ? img : CORRUPT
    },
  )
})

const loadCanvas = (src, args) => new Promise((res, rej) => {
  // Pull the creation-only settings out of the bag — the canvas & context options that can't be
  // applied after the fact — and pass whatever's left along as http request options.
  let {textContrast, textGamma, gpu, colorSpace, willReadFrequently, ...options} = args || {}

  fetchData(src, options, res, rej,
    (data, src, raw) => {
      const {Canvas} = require('./canvas') // deferred: canvas.js requires this module in turn

      // convert PDF data to an array of pages (or return `undefined` if it's any other format)
      let doc = neon.PDF.open(data)
      if (doc){
        let canvas
        for (let {page, width, height} of doc){
          let ctx = canvas ? canvas.newPage(width, height)
                           : (canvas = new Canvas(width, height, {textContrast, textGamma, gpu}))
                               .getContext('2d', {colorSpace, willReadFrequently})
          neon.PDF.impose(page, core(ctx))
        }
        return canvas ? canvas : CORRUPT // fails if no valid pages were recovered
      }

      // everything else becomes a single-page canvas sized to the image, with the image
      // content already drawn into it
      let img = new Image()
      img.prop('src', src)
      if (!img.prop('data', data, raw, undefined, true)) return CORRUPT

      let {width, height} = img,
          canvas = new Canvas(width, height, {textContrast, textGamma, gpu})
      canvas.getContext('2d', {colorSpace, willReadFrequently}).drawImage(img, 0, 0, width, height)
      return canvas
    },
  )
})

class Image extends RustClass {
  #fetch

  constructor(data, src='') {
    super(Image).alloc()

    data = expandURL(data)
    this.prop("src", ''+src || '::Buffer::')

    if (Buffer.isBuffer(data)) {
      if (!this.prop("data", data)) throw DecodingError()
    }else if (typeof data=='string'){
      decodeDataURL(data,
        buffer => {
          if (!this.prop("data", buffer)) throw DecodingError()
          if (!src) this.prop("src", data)
        },
        err => { throw err },
      )
    }else if (data){
      throw TypeError(`Exptected a Buffer or a String containing a data URL (got: ${data})`)
    }
  }

  get complete(){ return this.prop('complete') }
  get height(){ return this.prop('height') }
  get width(){ return this.prop('width') }

  #onload
  get onload(){ return this.#onload }
  set onload(cb){
    if (this.#onload) this.off('load', this.#onload)
    this.#onload = typeof cb=='function' ? cb : null
    if (this.#onload) this.on('load', this.#onload)
  }

  #onerror
  get onerror(){ return this.#onerror }
  set onerror(cb){
    if (this.#onerror) this.off('error', this.#onerror)
    this.#onerror = typeof cb=='function' ? cb : null
    if (this.#onerror) this.on('error', this.#onerror)
  }

  #fail(err){
    // only emit the 'error' event if there are listeners (since otherwise EventEmitter will throw)
    this.ref('error', err)
    if (this.listenerCount('error')) this.emit('error', err)
  }

  get src(){ return this.prop('src') }
  set src(src){
    const request = this.#fetch = {} // use an empty object as a unique token
    src = expandURL(src)
    this.prop("src", typeof src=='string' ? src : '')

    fetchData(src, undefined,
      () => {
        // loaded
        this.#fetch = undefined
        this.ref('error', null)
        this.emit('load', this)
      },
      err => {
        // failed
        if (request !== this.#fetch) return // confirm this is still the most recent request
        this.#fetch = undefined
        this.prop("data", Buffer.alloc(0))
        this.#fail(err)
      },
      (data, imgSrc, raw) => {
        // decode image data
        if (request !== this.#fetch) return STALE // src has been overwritten since the prior request
        this.prop("src", imgSrc)
        return this.prop("data", data, raw) ? this : CORRUPT
      },
    )
  }

  decode(){
    return this.#fetch ? new Promise((res, rej) => this.once('load', res).once('error', rej) )
         : this.ref('error') ? Promise.reject(this.ref('error'))
         : this.complete ? Promise.resolve(this)
         : Promise.reject(new Error("Image source not set"))
  }

  dispose(){
    if (this.ref('disposed')) return

    if (this.#fetch){
      this.#fetch = undefined
      this.#fail(DisposalError())
    }

    this.ƒ('dispose')
    this.ref('disposed', true)
  }
  [Symbol.dispose](){ this.dispose() }

  async release(){
    this.dispose()
    await new Promise(resolve => setImmediate(resolve))
  }
  [Symbol.asyncDispose](){ return this.release() }

  [REPR](depth, options) {
    if (this.ref('disposed')) return `Image (disposed)`
    let {width, height, complete, src} = this,
        err = this.ref('error')
    options.maxStringLength = src.startsWith("data:") ? 128 : Infinity;
    let error = err ? {error:err.message} : {}
    return `Image ${inspect({width, height, complete, src, ...error}, options)}`
  }
}

// Mix the EventEmitter properties into Image
Object.assign(Image.prototype, EventEmitter.prototype)

//
// ImageData
//

const loadImageData = (src, ...args) => new Promise((res, rej) => {
  let {colorType, colorSpace, ...options} = args[2] || {}
  fetchData(src, options, res, rej,
    (data, src, raw) => raw ? new ImageData(data, raw.width, raw.height) : new ImageData(data, ...args),
  )
})

class ImageData{
  constructor(...args){
    if (args[0] instanceof ImageData){
      argc(arguments, 1)
      var {data, width, height, colorSpace, colorType, bytesPerPixel} = args[0]
    }else if (args[0] instanceof Image){
      argc(arguments, 1)
      validImage(args[0])
      var [image, {colorSpace='srgb', colorType='rgba'}={}] = args,
          {width, height} = image,
          bytesPerPixel = validPixelSize(colorType),
          buffer = neon.Image.pixels(core(image), {colorType}),
          data = wrapPixels(buffer, colorType)
    }else if (args[0] instanceof Uint8ClampedArray || args[0] instanceof Uint8Array || isFloat16Array(args[0])){
      argc(arguments, 2)
      var [data, width, height, options={}] = args,
          {colorSpace='srgb'} = options,
          colorType = (options.colorType != null) ? options.colorType
                    : isFloat16Array(data) ? 'RGBAF16' : 'rgba',
          bytesPerPixel = validPixelSize(colorType) // validates the string as side effect

      // throw on 8/16-bit mismatches
      if (isFloat16Array(data) && !isFloat16Type(colorType)){
        throw new TypeError(`A Float16Array requires a half-float colorType (got '${colorType}')`)
      }

      let {byteLength} = data
      width = Math.floor(Math.abs(width))
      height = Math.floor(Math.abs(height || byteLength / width / bytesPerPixel))
      data = wrapPixels(data, colorType)
      if (byteLength / bytesPerPixel != width * height){
        throw new TypeError("ImageData dimensions must match buffer length")
      }
    }else if (ArrayBuffer.isView(args[0]) || args[0] instanceof ArrayBuffer){
      throw new TypeError(`Expected a Uint8ClampedArray, Uint8Array, Float16Array, or Buffer (got ${args[0].constructor.name})`)
    }else{
      argc(arguments, 2)
      var [width, height, {colorSpace='srgb', colorType='rgba'}={}] = args,
          bytesPerPixel = validPixelSize(colorType)

      width = Math.floor(Math.abs(width))
      height = Math.floor(Math.abs(height))
    }

    colorSpace = validColorSpace(colorSpace)

    if (!Number.isInteger(width) || !Number.isInteger(height) || width <= 0 || height <= 0){
      throw RangeError("Dimensions must be non-zero")
    }

    readOnly(this, "colorSpace", colorSpace)
    readOnly(this, "colorType", colorType)
    readOnly(this, "width", width)
    readOnly(this, "height", height)
    readOnly(this, 'bytesPerPixel', bytesPerPixel)
    readOnly(this, "data", data || allocPixels(width, height, bytesPerPixel, colorType))
  }

  toSharp(){
    const spec = SHARP_FORMATS[this.colorType]
    if (!spec) throw new TypeError(`toSharp() does not support the colorType '${this.colorType}'`)

    const sharp = getSharp()
    let {width, height} = this,
        {channels} = spec

    // Sharp can only read wide-gamut colors when wrapped in a file container, so convert display-p3 to TIFF
    let sharpImage = this.colorSpace != 'srgb' && !spec.colorless
      ? sharp(neon.ImageData.toTiff(this, spec))
      : sharp(neon.ImageData.toRaw(this, spec), {raw:{width, height, channels}})

    return sharpImage
      .keepIccProfile()
      .withDensity(72)
  }

  [REPR](depth, options) {
    let {width, height, colorType, bytesPerPixel, data} = this
    return `ImageData ${inspect({width, height, colorType, bytesPerPixel, data}, options)}`
  }
}

//
// Utilities
//

function validImage(image){
  if (image.ref('error')) throw new Error("Image failed to load: listen for the `error` event or await `decode()` for details")
  if (!image.complete) throw new Error("Image has not completed loading: listen for the `load` event or await `decode()` first")
  return image
}

function validColorSpace(colorSpace){
  if (!['srgb', 'display-p3'].includes(colorSpace)){
    if (STRICT) throw TypeError(`Unsupported colorSpace: ${colorSpace}`)
    colorSpace = 'srgb' // unless in strict mode, invalid spaces are silently ignored
  }
  return colorSpace
}

const FLOAT16_TYPES = ["RGBAF16", "RGBAF16Norm", "A16Float", "R16G16Float"]
const hasFloat16 = typeof Float16Array !== 'undefined' // Float16Array is Node 23+
const isFloat16Type = colorType => FLOAT16_TYPES.includes(colorType)
const isFloat16Array = value => hasFloat16 && value instanceof Float16Array
const arrayForColorType = colorType =>
  isFloat16Type(colorType) && hasFloat16 ? Float16Array : Uint8ClampedArray

// ensure raw pixel bytes are wrapped in a colorType-appropriate array
function wrapPixels(data, colorType){
  const ArrayType = arrayForColorType(colorType)
  if (data instanceof ArrayType) return data // return as-is if already correctly typed

  // borrow the buffer contents rather than copying (if alingment permits it)
  let {buffer, byteOffset, byteLength} = data
  return byteOffset % ArrayType.BYTES_PER_ELEMENT == 0
    ? new ArrayType(buffer, byteOffset, byteLength / ArrayType.BYTES_PER_ELEMENT)
    : new ArrayType(buffer.slice(byteOffset, byteOffset + byteLength))
}

// allocate a zero-filled pixel buffer of the colorType-appropriate array type
function allocPixels(width, height, bytesPerPixel, colorType){
  const ArrayType = arrayForColorType(colorType)
  return new ArrayType(width * height * bytesPerPixel / ArrayType.BYTES_PER_ELEMENT)
}

function validPixelSize(colorType){
  const bpp = ["Alpha8", "Gray8", "R8UNorm"].includes(colorType) ? 1
    : ["A16Float", "A16UNorm", "ARGB4444", "R8G8UNorm", "RGB565"].includes(colorType) ? 2
    : [ "rgb", "rgba", "bgra", "BGR101010x", "BGRA1010102", "BGRA8888", "R16G16Float", "R16G16UNorm",
        "RGB101010x", "RGB888x", "RGBA1010102", "RGBA8888", "SRGBA8888" ].includes(colorType) ? 4
    : ["R16G16B16A16UNorm", "RGBAF16", "RGBAF16Norm"].includes(colorType) ? 8
    : colorType=="RGBAF32" ? 16
    : 0

  if (!bpp) throw new TypeError(`Unknown colorType: ${colorType}`)
  return bpp
}

// Sharp accepts 1–4 bands (8-bit or 16-bit unsigned) in RGBA order only, so some colorTypes require
// layout changes or re-encoding:
//
// `to`:        the color type to be converted to (omitted if the layout is already compatible)
// `channels`:  the number of distinct channels to be preserved (<4 if monochrome or alpha-less)
// `bits`:      the number of bits per channel (defaults to 8 if omitted)
// `colorless`: whether the format contains <3 color channels, so an ICC profile is meaningless
//
const SHARP_FORMATS = {
  // Native libvips layouts (no `to` conversion necessary)
  Alpha8:            {channels:1, colorless:true},
  Gray8:             {channels:1, colorless:true},
  R8UNorm:           {channels:1, colorless:true},
  rgba:              {channels:4},
  RGBA8888:          {channels:4},

  // like RGBA8888 but double-applies the sRGB transfer function, so needs to be reencoded first
  SRGBA8888:         {to:'rgba', channels:4},

  // 8-bit, but the channel order or bit packing needs to be normalized
  bgra:              {to:'rgba', channels:4},
  BGRA8888:          {to:'rgba', channels:4},
  ARGB4444:          {to:'rgba', channels:4},
  RGB565:            {to:'rgba', channels:4},

  // RGB plus an unused padding byte: normalize then drop the pad
  rgb:               {to:'rgba', channels:3},
  RGB888x:           {to:'rgba', channels:3},

  // 16-bit unsigned, used as-is (no `to` conversion)
  A16UNorm:          {channels:1, bits:16, colorless:true},
  R16G16B16A16UNorm: {channels:4, bits:16},

  // 10-bit packed: convert to 16-bit to preserve detail
  RGB101010x:        {to:'R16G16B16A16UNorm', channels:4, bits:16},
  RGBA1010102:       {to:'R16G16B16A16UNorm', channels:4, bits:16},
  BGR101010x:        {to:'R16G16B16A16UNorm', channels:4, bits:16},
  BGRA1010102:       {to:'R16G16B16A16UNorm', channels:4, bits:16},

  // Two-channel RG data: expand to 3 channels with B=0
  R8G8UNorm:         {to:'rgba', channels:3, colorless:true},
  R16G16UNorm:       {to:'R16G16B16A16UNorm', channels:3, bits:16, colorless:true},
  R16G16Float:       {to:'R16G16B16A16UNorm', channels:3, bits:16, colorless:true},

  // Canvas surfaces are 8-bit SDR so float types are already clamped [0..1]:
  // remap to unsigned since libvips's float-handling would use *linear* scRGB
  A16Float:          {to:'A16UNorm', channels:1, bits:16, colorless:true},
  RGBAF16:           {to:'R16G16B16A16UNorm', channels:4, bits:16},
  RGBAF16Norm:       {to:'R16G16B16A16UNorm', channels:4, bits:16},
  RGBAF32:           {to:'R16G16B16A16UNorm', channels:4, bits:16},
}

function getSharp(){
  try{
    return require('sharp')
  }catch(e){
    throw Error("Cannot find module 'sharp' (try running `npm install sharp` first)")
  }
}

function isSharpImage(obj){
  return obj != null && typeof obj == 'object' &&
         obj.constructor && obj.constructor.name == 'Sharp' &&
         obj.options && typeof obj.options == 'object' &&
         typeof obj.ensureAlpha == 'function' && typeof obj.raw == 'function' &&
         typeof obj.toBuffer == 'function'
}

const fetchData = (src, reqOpts, loaded, failed, decode) => {
  const fetched = (data, imgSrc, raw) => {
    let result

    // `decode` runs within a try so errors are turned into promise rejections/`error` events
    try{ result = decode(data, imgSrc, raw) }
    catch(e){ return failed(e) }

    if (result === STALE) return // got response to a no-longer-current request
    if (result === CORRUPT || !result) return failed(DecodingError()) // received unusable data

    // `loaded` runs without a try, so a throw from a user 'load' handler propagates instead of being
    // reported as a decode failure
    loaded(result)
  }

  src = expandURL(src)
  if (Buffer.isBuffer(src)) {
    fetched(src, '::Buffer::')
  }else if (isSharpImage(src)){
    src.ensureAlpha().raw().toBuffer((err, buf, info) => {
      let {file} = src.options.input // preserve path if file-based
      if (err) failed(err)
      else fetched(buf, file ? file : '::Sharp::', info)
    })
  }else{
    src = typeof src=='string' ? src : ''+src
    if (src.startsWith('data:')){
      decodeDataURL(src,
        buffer => fetched(buffer, src),
        err =>  failed(err),
      )
    }else if (/^\s*https?:\/\//.test(src)){
      fetchURL(src, reqOpts,
        buffer => fetched(buffer, src),
        err => failed(err)
      )
    }else{
      readFile(src)
        .then(data => fetched(data, src))
        .catch(e => failed(e))
    }
  }
}

module.exports = {Image, ImageData, loadImage, loadImageData, loadCanvas, validPixelSize, validColorSpace, getSharp, validImage}
