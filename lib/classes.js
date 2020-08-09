"use strict"

var fs = require('fs'),
    {inspect} = require('util'),
    get = require('simple-get'),
    {parseFont, parseSize} = require('./text'),
    font = require('css-font'),
    native = require('../native'),
    {$, RustClass, toSkMatrix, fromSkMatrix} = require('./utils');

const REPR = inspect.custom

class CanvasRenderingContext2D extends RustClass(native.CanvasRenderingContext2D, "getImageData"){

  get currentTransform(){ return fromSkMatrix( $(this, 'get_currentTransform') ) }
  set currentTransform(matrix){  $(this, 'set_currentTransform', toSkMatrix(matrix) ) }

  getTransform(){ return this.currentTransform }

  setTransform(matrix){
    if (arguments.length>1) matrix = [...arguments]
    this.currentTransform = matrix
  }

  createLinearGradient(...args){
    return new CanvasGradient("Linear", ...args)
  }

  createRadialGradient(...args){
    return new CanvasGradient("Radial", ...args)
  }

  createPattern(...args){
    return new CanvasPattern(...args)
  }

  createImageData(width, height){
    return new ImageData(width, height)
  }

  getImageData(...args){
    return new ImageData( $(this, 'getImageData', ...args) )
  }

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
    return `CanvasRenderingContext2D ${inspect(info, {colors:true})}`
  }
}

class Path2D extends RustClass(native.Path2D){
  addPath(path, matrix){
    if (matrix) $(this, 'addPath', path, toSkMatrix(matrix) )
    else $(this, 'addPath', path)
  }
}

class CanvasGradient extends RustClass(native.CanvasGradient){}

class CanvasPattern extends RustClass(native.CanvasPattern, 'setTransform'){
  setTransform(matrix){
    if (arguments.length>1) matrix = [...arguments]
    $(this, 'setTransform', toSkMatrix(matrix) )
  }
}

class Image extends RustClass(native.Image){

  get src(){ return $(this, "get_src") }
  set src(src){
    var data

    if (Buffer.isBuffer(src)) data = src
    else if (typeof src != 'string') return
    else if (/^\s*data:/.test(src)) {
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
          if ($(this, "set_data", data)) onload(this)
          else onerror(new Error("Could not decode image data"))
        }
      })
    } else {
      // local file path
      data = fs.readFileSync(src);
    }

    $(this, "set_src", src)
    if (data){
      let onerror = this.onerror || (() => {}), onload = this.onload || (() => {});
      if ($(this, "set_data", data)) onload(this)
      else onerror(new Error("Could not decode image data"))
    }
  }

  [REPR](depth, options) {
    let {width, height, complete, src} = this,
        maxStringLength = src.match(/^data:/) ? 128 : Infinity;
    return `Image ${inspect({width, height, complete, src}, {colors:true, maxStringLength})}`
  }
}

class ImageData extends RustClass(native.ImageData){
  constructor(width, height){
    if (arguments[0] instanceof native.ImageData){
      var {width, height, data} = arguments[0]
    }
    super(width, height)
    Object.defineProperty(this, "data", {
      value:new Uint8ClampedArray(data && data.buffer || (this.width * this.height * 4))
    })
  }

  [REPR](depth, options) {
    let {width, height, data} = this
    return `ImageData ${inspect({width, height, data}, {colors:true})}`
  }
}

module.exports = {CanvasRenderingContext2D, Path2D, Image, ImageData, CanvasGradient, CanvasPattern}