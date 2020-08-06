var fs = require('fs'),
    {inspect} = require('util'),
    get = require('simple-get'),
    {DOMMatrix, DOMRect, DOMPoint} = require('./geometry'),
    {CanvasRenderingContext2D, CanvasGradient, Path2D, Image} = require('../native'),
    {wrap, hide, toColor, fromColor, toSkMatrix, fromSkMatrix} = require('./utils');

// accessor for calling the rust implementation of a shadowed method
const _ = (obj, s, ...args) =>{
  let fn = Symbol.for(s)
  return obj[fn](...args)
}

//
// CanvasRenderingContext2D
//
wrap(CanvasRenderingContext2D, {
  describe:function(){
    let props = [ "canvas", "currentTransform", "fillStyle", "strokeStyle", "filter", "font", "direction", "textAlign", "textBaseline",
                  "globalAlpha", "globalCompositeOperation", "imageSmoothingEnabled", "imageSmoothingQuality", "shadowBlur", "shadowColor",
                  "shadowOffsetX", "shadowOffsetY", "lineCap", "lineDashOffset", "lineJoin", "lineWidth", "miterLimit" ]
    let info = {}
    for (var prop of props){
      try{ info[prop] = this[prop] }
      catch{ info[prop] = undefined }
    }
    return `CanvasRenderingContext2D ${inspect(info, {colors:true})}`
  },

  // Property overrides
  currentTransform:{
    get:function(){ return fromSkMatrix( _(this, 'get_currentTransform') ) },
    set:function(matrix){  _(this, 'set_currentTransform', toSkMatrix(matrix) ) }
  },
  strokeStyle:{
    get:function(){
      return fromColor( _(this, 'get_strokeStyle') )
    },
    set:function(clr){
      let rgba = toColor(clr)
      if (rgba) _(this, 'set_strokeStyle', ...rgba )
    }
  },
  fillStyle:{
    get:function(){
      return fromColor( _(this, 'get_fillStyle') )
    },
    set:function(clr){
      let rgba = toColor(clr)
      if (rgba) _(this, 'set_fillStyle', ...rgba )
  }
  },
  shadowColor:{
    get:function(){
      return fromColor( _(this, 'get_shadowColor') )
    },
    set:function(clr){
      let rgba = toColor(clr)
      if (rgba) _(this, 'set_shadowColor', ...rgba )
    }
  },

  // Method overrides
  getTransform:function(){ return this.currentTransform },

  setTransform:function(matrix){
    if (arguments.length>1) matrix = [...arguments]
    this.currentTransform = matrix
  },

  createLinearGradient:function(...args){
    return new CanvasGradient("Linear", ...args)
  },
  createRadialGradient:function(){
    return new CanvasGradient("Radial", ...args)
  },

})

//
// Path2D
//
wrap(Path2D, {
  addPath:function(path, matrix){
    if (matrix) _(this, 'addPath', path, toSkMatrix(matrix) )
    else _(this, 'addPath', path)
  }
})

//
// CanvasGradient
//
wrap(CanvasGradient, {
  addColorStop:function(pct, clr){
    let rgba = toColor(clr)
    if (rgba) _(this, 'addColorStop', pct, ...rgba )
    else throw new SyntaxError("The color string did not match the expected pattern")
  }
})

//
// Image
//
hide(Image, ["set_data"])

wrap(Image, {
  describe:function(){
    let {width, height, complete, src} = this,
        maxStringLength = src.match(/^data:/) ? 128 : Infinity;
    return `Image ${inspect({width, height, complete, src}, {colors:true, maxStringLength})}`
  },

  src:{
    get:function(){
      return _(this, "get_src")
    },
    set:function(src){
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
            onerror(new Error(`HTTP error ${code}`))
          }else{
            if (_(this, "set_data", data)) onload({target:this})
            else onerror(new Error("Could not decode image data"))
          }
        })
      } else {
        // local file path
        data = fs.readFileSync(src);
      }

      _(this, "set_src", src)
      if (data){
        let onerror = this.onerror || (() => {}), onload = this.onload || (() => {});
        if (_(this, "set_data", data)) onload({target:this})
        else onerror(new Error("Could not decode image data"))
      }
    }
  }

})

module.exports = {CanvasRenderingContext2D, Path2D, Image, DOMMatrix, DOMRect, DOMPoint}
