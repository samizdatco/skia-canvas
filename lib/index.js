var {DOMMatrix, DOMRect, DOMPoint} = require('./geometry-polyfill'),
    {CanvasRenderingContext2D, CanvasGradient, Path2D} = require('../native'),
    {wrap, toColor, fromColor, toSkMatrix, fromSkMatrix} = require('./utils');

// accessor for calling the rust implementation of a shadowed method
const _ = (obj, s, ...args) => obj[Symbol.for(s)](...args)

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
// CanvasRenderingContext2D
//
wrap(CanvasRenderingContext2D, {
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
  setTransform:function(matrix){ this.currentTransform = matrix },
  createLinearGradient:function(...args){
    return new CanvasGradient("Linear", ...args)
  },
  createRadialGradient:function(){
    return new CanvasGradient("Radial", ...args)
  },

})

module.exports = {CanvasRenderingContext2D, Path2D, DOMMatrix, DOMRect, DOMPoint}
