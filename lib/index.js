var {CanvasRenderingContext2D, CanvasGradient, Path2D} = require('../native'),
    {DOMMatrix, DOMRect, DOMPoint} = require('./geometry-polyfill');
const { wrap, toColor, fromColor } = require('./utils');


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
  getTransform:function(){ return this.currentTransform },
  setTransform:function(matrix){ this.currentTransform = matrix },
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

  createLinearGradient:function(...args){
    return new CanvasGradient("Linear", ...args)
  },
  createRadialGradient:function(){
    return new CanvasGradient("Radial", ...args)
  },

})

module.exports = {CanvasRenderingContext2D, Path2D, DOMMatrix, DOMRect, DOMPoint}
