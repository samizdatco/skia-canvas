//
// Image & ImageData
//

"use strict"

const {RustClass, core, readOnly, inspect, neon, argc, REPR} = require('./neon'),
      {EventEmitter} = require('events'),
      {readFile} = require('fs/promises'),
      {fetch} = require('cross-fetch'),
      url = require('url')

const loadImage = (src, options) => new Promise((res, rej) =>
  Image.fetchData(
    src,
    options,
    ({data, src, raw}) => {
      let img = new Image()
      img.prop('src', src)
      if (img.prop('data', data, raw)) res(img)
      else rej(new Error("Could not decode image data"))
    },
    rej,
  )
)

const loadImageData = (src, ...args) => new Promise((res, rej) => {
  let {colorType, colorSpace, ...options} = args[2] || {}
  Image.fetchData(src, options, ({data, raw}) => res(
    raw ? new ImageData(data, raw.width, raw.height) : new ImageData(data, ...args)
  ), rej)
})

class Image extends RustClass {
  #fetch
  #err

  constructor() {
    super(Image).alloc()
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

  get src(){ return this.prop('src') }
  set src(src){
    const request = this.#fetch = {}, // use an empty object as a unique token
          loaded = ({data, src, raw}) => {
            if (request === this.#fetch){ // confirm this is the most recent request with ===
              this.#fetch = undefined
              this.prop("src", src)
              this.#err = this.prop("data", data, raw) ? null : new Error("Could not decode image data")
              if (this.#err) this.emit('error', this.#err)
              else this.emit('load', this)
            }
          },
          failed = (err) => {
            this.#fetch = undefined
            this.#err = err
            this.prop("data", Buffer.alloc(0))
            this.emit('error', err)
          }

    this.prop("src", typeof src=='string' ? src : '')
    Image.fetchData(src, undefined, loaded, failed)
  }

  static fetchData(src, fetchOpts, ok, fail){
    if (src instanceof URL){
      if (src.protocol=='file:') src = url.fileURLToPath(src)
      else if (src.protocol.match(/^(https?|data):/)) src = src.href
      else throw new Error(`Unsupported protocol: ${src.protocol.replace(':', '')}`)
    }

    if (Buffer.isBuffer(src)) {
      // manually loaded Buffer
      ok({data:src, src:''})
    } else if (isSharpImage(src)) {
      // Sharp object
      src.ensureAlpha().raw().toBuffer((err, buf, info) => {
        if (err) fail(err)
        else ok({data:buf, src:'', raw:info})
      })
    } else if (typeof src != 'string') {
      fail(new Error("'src' property value is neither string nor Buffer type.'"))
    } else if (src.startsWith('data:')) {
      // data URI
      let [header, mime, enc] = src.slice(0, 40).match(/^\s*data:(?<mime>[^;]*);(?:charset=)?(?<enc>[^,]*),/) || []
      if (!mime || !enc){
        throw new Error(`Invalid data URI header`)
      } else {
        let content = src.slice(header.length)
        if (enc.toLowerCase() != 'base64'){
          content = decodeURIComponent(content)
        }
        ok({data:Buffer.from(content, enc), src:''})
      }
    } else if (/^\s*https?:\/\//.test(src)) {
      // remote URL
      fetch(src, fetchOpts)
        .then(resp => {
          if (resp.ok) return resp.arrayBuffer()
          else throw new Error(`Failed to load image from "${src}" (HTTP error ${resp.status})`)
        })
        .then(buf => ok({data:Buffer.from(buf), src}))
        .catch(err => fail(err))
    } else {
      // local file path
      readFile(src).then(data => ok({data, src})).catch(e => fail(e))
    }
  }

  decode(){
    return this.#fetch ? new Promise((res, rej) => this.once('load', res).once('error', rej) )
         : this.#err ? Promise.reject(this.#err)
         : this.complete ? Promise.resolve(this)
         : Promise.reject(new Error("Image source not set"))
  }

  [REPR](depth, options) {
    let {width, height, complete, src} = this
    options.maxStringLength = src.match(/^data:/) ? 128 : Infinity;
    return `Image ${inspect({width, height, complete, src}, options)}`
  }
}

// Mix the EventEmitter properties into Image
Object.assign(Image.prototype, EventEmitter.prototype)


class ImageData{
  constructor(...args){
    if (args[0] instanceof ImageData){
      argc(arguments, 1)
      var {data, width, height, colorSpace, colorType, bytesPerPixel} = args[0]
    }else if (args[0] instanceof Image){
      argc(arguments, 1)
      var [image, {colorSpace='srgb', colorType='rgba'}={}] = args,
          {width, height} = image,
          bytesPerPixel = pixelSize(colorType),
          buffer = neon.Image.pixels(core(image), {colorType}),
          data = new Uint8ClampedArray(buffer)
    }else if (args[0] instanceof Uint8ClampedArray || args[0] instanceof Buffer){
      argc(arguments, 2)
      var [data, width, height, {colorSpace='srgb', colorType='rgba'}={}] = args,
          bytesPerPixel = pixelSize(colorType) // validates the string as side effect

      width = Math.floor(Math.abs(width))
      height = Math.floor(Math.abs(height || data.length / width / bytesPerPixel))
      data = data instanceof Uint8ClampedArray ? data : new Uint8ClampedArray(data)
      if (data.length / bytesPerPixel != width * height){
        throw new TypeError("ImageData dimensions must match buffer length")
      }
    }else{
      argc(arguments, 2)
      var [width, height, {colorSpace='srgb', colorType='rgba'}={}] = args,
          bytesPerPixel = pixelSize(colorType)

      width = Math.floor(Math.abs(width))
      height = Math.floor(Math.abs(height))
    }

    if (!['srgb'].includes(colorSpace)){ // TODO: add display-p3 when supported…
      throw TypeError(`Unsupported colorSpace: ${colorSpace}`)
    }

    if (!Number.isInteger(width) || !Number.isInteger(height) || width <= 0 || height <= 0){
      throw RangeError("Dimensions must be non-zero")
    }

    readOnly(this, "colorSpace", colorSpace)
    readOnly(this, "colorType", colorType)
    readOnly(this, "width", width)
    readOnly(this, "height", height)
    readOnly(this, 'bytesPerPixel', bytesPerPixel)
    readOnly(this, "data", data || new Uint8ClampedArray(width * height * bytesPerPixel))
  }

  toSharp(){
    const sharp = getSharp()
    let {width, height, bytesPerPixel:channels} = this
    return sharp(this.data, {raw:{width, height, channels}}).withMetadata({density:72})
  }

  [REPR](depth, options) {
    let {width, height, colorType, bytesPerPixel, data} = this
    return `ImageData ${inspect({width, height, colorType, bytesPerPixel, data}, options)}`
  }
}

function pixelSize(colorType){
  const bpp = ["Alpha8", "Gray8", "R8UNorm"].includes(colorType) ? 1
    : ["A16Float", "A16UNorm", "ARGB4444", "R8G8UNorm", "RGB565"].includes(colorType) ? 2
    : [ "rgb", "rgba", "bgra", "BGR101010x", "BGRA1010102", "BGRA8888", "R16G16Float", "R16G16UNorm",
        "RGB101010x", "RGB888x", "RGBA1010102", "RGBA8888", "RGBA8888", "SRGBA8888" ].includes(colorType) ? 4
    : ["R16G16B16A16UNorm", "RGBAF16", "RGBAF16Norm"].includes(colorType) ? 8
    : colorType=="RGBAF32" ? 16
    : 0

  if (!bpp) throw new TypeError(`Unknown colorType: ${colorType}`)
  return bpp
}

function getSharp(){
  try{
    return require('sharp')
  }catch(e){
    throw Error("Cannot find module 'sharp' (try running `npm install sharp` first)")
  }
}

function isSharpImage(obj){
   try{
    return obj instanceof require('sharp')
  }catch{
    return false
  }
}

module.exports = {Image, ImageData, loadImage, loadImageData, pixelSize, getSharp}
