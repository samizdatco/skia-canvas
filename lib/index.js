var {inspect} = require('util'),
    {DOMMatrix, DOMRect, DOMPoint} = require('./geometry'),
    {CanvasRenderingContext2D, CanvasGradient, Path2D, Image} = require('../native'),
    {wrap, toColor, fromColor, toSkMatrix, fromSkMatrix} = require('./utils');

// accessor for calling the rust implementation of a shadowed method
const _ = (obj, s, ...args) =>{
  let fn = Symbol.for(s)
  return obj[fn](...args)
}

//
// Image
//
wrap(Image, {
  src:{
    get:function(){

    },
    set:function(src){
      if (Buffer.isBuffer(src)){
        _(this, "set_src", src)
      }
    }
  }

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

module.exports = {CanvasRenderingContext2D, Path2D, DOMMatrix, DOMRect, DOMPoint}
