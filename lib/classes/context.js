//
// The Canvas drawing API
//

"use strict"

const {RustClass, core, wrap, inspect, argc, REPR} = require('./neon'),
      {Canvas, CanvasGradient, CanvasPattern, CanvasTexture} = require('./canvas'),
      {fromSkMatrix} = require('./geometry'),
      {Image, ImageData} = require('./imagery'),
      {TextMetrics} = require('./typography'),
      {Path2D, DrawList} = require('./path'),
      css = require('./css')

const toString = val => typeof val=='string' ? val : new String(val).toString()

class CanvasRenderingContext2D extends RustClass{
  #canvas

  constructor(canvas){
    try{
      super(CanvasRenderingContext2D).alloc(core(canvas))
      this.#canvas = new WeakRef(canvas)
    }catch(e){
      throw new TypeError(`Function is not a constructor (use Canvas's "getContext" method instead)`)
    }
  }

  get canvas(){ return this.#canvas.deref() }

  getContextAttributes(){
    let colorSpace = this.canvas.ref('colorSpace') ?? 'srgb'
    let willReadFrequently = this.canvas.ref('willReadFrequently') ?? false
    return {alpha:true, colorSpace, desynchronized:false, willReadFrequently}
  }

  // -- global state & content reset ------------------------------------------
  reset(){ this.ƒ('reset') }

  // -- grid state ------------------------------------------------------------
  get currentTransform(){ return fromSkMatrix( this.prop('currentTransform') ) }
  set currentTransform(matrix){ this.setTransform(matrix) }
  getTransform(){ return this.currentTransform }

  createProjection(quad, basis){
    return fromSkMatrix(this.ƒ("createProjection", [quad].flat(), [basis].flat()))
  }

  // -- using paths -----------------------------------------------------------
  clip(path, rule){
    if (path instanceof Path2D) arguments[0] = core(path)
    return this.ƒ('clip', ...arguments)
  }

  isPointInPath(path, x, y, rule){
    if (path instanceof Path2D) arguments[0] = core(path)
    return this.ƒ('isPointInPath', ...arguments)
  }
  isPointInStroke(path, x, y){
    if (path instanceof Path2D) arguments[0] = core(path)
    return this.ƒ('isPointInStroke', ...arguments)
  }


  // -- shaders ---------------------------------------------------------------
  createPattern(image, repetition){ return new CanvasPattern(this.canvas, ...arguments) }
  createLinearGradient(...args){
    return new CanvasGradient("Linear", ...args)
  }
  createRadialGradient(...args){
    return new CanvasGradient("Radial", ...args)
  }
  createConicGradient(...args){
    return new CanvasGradient("Conic", ...args)
  }

  createTexture(spacing, options){
    return new CanvasTexture(...arguments)
  }

  // -- fill & stroke ---------------------------------------------------------
  set fillStyle(style){
    let isShader = style instanceof CanvasPattern || style instanceof CanvasGradient || style instanceof CanvasTexture,
        [ref, val] = isShader ? [style, core(style)] : [null, style]
    this.ref('fill', ref)
    this.prop('fillStyle', val)
  }

  get fillStyle(){
    let style = this.prop('fillStyle')
    return style===null ? this.ref('fill') : style
  }

  set strokeStyle(style){
    let isShader = style instanceof CanvasPattern || style instanceof CanvasGradient || style instanceof CanvasTexture,
        [ref, val] = isShader ? [style, core(style)] : [null, style]
    this.ref('stroke', ref)
    this.prop('strokeStyle', val)
  }

  get strokeStyle(){
    let style = this.prop('strokeStyle')
    return style===null ? this.ref('stroke') : style
  }

  // -- line style ------------------------------------------------------------
  getLineDash(){        return this.ƒ("getLineDash") }
  setLineDash(segments){       this.ƒ("setLineDash", ...arguments) }
  get lineCap(){        return this.prop("lineCap") }
  set lineCap(style){          this.prop("lineCap", style) }
  get lineDashFit(){    return this.prop("lineDashFit") }
  set lineDashFit(style){      this.prop("lineDashFit", style) }
  get lineDashMarker(){ return wrap(Path2D, this.prop("lineDashMarker")) }
  set lineDashMarker(path){    this.prop("lineDashMarker", path instanceof Path2D ? core(path) : path) }
  get lineDashOffset(){ return this.prop("lineDashOffset") }
  set lineDashOffset(offset){  this.prop("lineDashOffset", offset) }
  get lineJoin(){       return this.prop("lineJoin") }
  set lineJoin(style){         this.prop("lineJoin", style) }
  get lineWidth(){      return this.prop("lineWidth") }
  set lineWidth(width){        this.prop("lineWidth", width) }
  get miterLimit(){     return this.prop("miterLimit") }
  set miterLimit(limit){       this.prop("miterLimit", limit) }

  // -- imagery ---------------------------------------------------------------
  get imageSmoothingEnabled(){ return this.prop("imageSmoothingEnabled")}
  set imageSmoothingEnabled(flag){    this.prop("imageSmoothingEnabled", !!flag)}
  get imageSmoothingQuality(){ return this.prop("imageSmoothingQuality")}
  set imageSmoothingQuality(level){   this.prop("imageSmoothingQuality", level)}

  createImageData(width, height, settings){
    argc(arguments, 2, 3)
    const {colorSpace} = this.getContextAttributes() // inherit the canvas's color space by default
    return new ImageData(width, height, {colorSpace, ...settings})
  }

  getImageData(x, y, width, height, {colorType='rgba', colorSpace, density=1, matte, msaa}={}){
    argc(arguments, 4, 5)

    if (colorSpace===undefined) ({colorSpace} = this.getContextAttributes())

    if (typeof density!='number' || !Number.isInteger(density) || density<1){
      throw new TypeError("Expected a non-negative integer for `density`")
    }

    if (msaa===undefined) {
      msaa = undefined // default to shader-based AA)
    }else if (msaa===true) {
      msaa = 4
    }else if (!isFinite(+msaa) || +msaa<0){
      throw new TypeError("The number of MSAA samples must be an integer ≥0")
    }

    let opts = {colorType, colorSpace, density, matte, msaa},
        buffer = this.ƒ('getImageData', x, y, width, height, opts, core(this.canvas), this.canvas.ref('willReadFrequently') ?? false);
    return new ImageData(buffer, width*density, height*density, {colorType, colorSpace})
  }

  putImageData(imageData, ...coords){
    argc(arguments, 3, 7)
    if (!(imageData instanceof ImageData)) throw TypeError("Expected an ImageData as 1st arg")
    this.ƒ('putImageData', imageData, ...coords)
  }

  drawImage(image, ...coords){
    if (image instanceof Canvas){
      this.ƒ('drawImage', core(image.getContext('2d')), ...coords)
    }else if (image instanceof Image){
      if (image.complete) this.ƒ('drawImage', core(image), ...coords)
      else throw Error("Image has not completed loading: listen for `load` event or await `decode()` first")
    }else if (image instanceof ImageData){
      this.ƒ('drawImage', image, ...coords)
    }else if (image instanceof Promise) {
      throw Error("Promise has not yet resolved: `await` image loading before drawing")
    }else{
      let nonimage = inspect(image, {depth:1})
      throw Error(`Expected an Image or a Canvas argument (got: ${nonimage})`)
    }
  }

  drawCanvas(image, ...coords){
    if (image instanceof Canvas){
      this.ƒ('drawCanvas', core(image.getContext('2d')), ...coords)
    }else{
      this.drawImage(image, ...coords)
    }
  }

  // -- typography ------------------------------------------------------------
  get font(){          return this.prop('font') }
  set font(str){              this.prop('font', css.font(str)) }
  get textAlign(){     return this.prop("textAlign") }
  set textAlign(mode){        this.prop("textAlign", mode) }
  get textBaseline(){  return this.prop("textBaseline") }
  set textBaseline(mode ){    this.prop("textBaseline", mode) }
  get direction(){     return this.prop("direction") }
  set direction(mode){        this.prop("direction", mode) }
  get fontStretch(){   return this.prop('fontStretch') }
  set fontStretch(str){       this.prop('fontStretch', css.stretch(str)) }
  get letterSpacing(){ return this.prop('letterSpacing') }
  set letterSpacing(str){     this.prop('letterSpacing', css.spacing(str)) }
  get wordSpacing(){   return this.prop('wordSpacing') }
  set wordSpacing(str){       this.prop('wordSpacing', css.spacing(str)) }

  measureText(text, maxWidth){
    let metrics = JSON.parse(this.ƒ('measureText', toString(text), maxWidth))
    return new TextMetrics(metrics)
  }

  fillText(text, ...geom){
    this.ƒ('fillText', toString(text), ...geom)
  }

  strokeText(text, ...geom){
    this.ƒ('strokeText', toString(text), ...geom)
  }

  outlineText(text, ...geom){
    let path = this.ƒ('outlineText', toString(text), ...geom)
    return path ? wrap(Path2D, path) : null
  }

  // -- non-standard typography extensions --------------------------------------------
  get fontHinting(){    return this.prop("fontHinting") }
  set fontHinting(flag){       this.prop("fontHinting", !!flag) }
  get fontSmoothing(){  return this.prop("fontSmoothing") }
  set fontSmoothing(flag){     this.prop("fontSmoothing", !!flag) }
  get fontVariant(){    return this.prop('fontVariant') }
  set fontVariant(str){        this.prop('fontVariant', css.variant(str)) }
  get fontSynthesis(){  return this.prop("fontSynthesis") }
  set fontSynthesis(flag){     this.prop("fontSynthesis", !!flag) }
  get textWrap(){       return this.prop("textWrap") }
  set textWrap(flag){          this.prop("textWrap", !!flag) }
  get textDecoration(){ return this.prop("textDecoration") }
  set textDecoration(str){     this.prop("textDecoration", css.decoration(str)) }
  set textTracking(_){
    process.emitWarning("The .textTracking property has been removed; use the .letterSpacing property instead", "PropertyRemoved")
  }

  // -- effects ---------------------------------------------------------------
  get globalCompositeOperation(){ return this.prop("globalCompositeOperation") }
  set globalCompositeOperation(blend){   this.prop("globalCompositeOperation", blend) }
  get globalAlpha(){   return this.prop("globalAlpha") }
  set globalAlpha(alpha){     this.prop("globalAlpha", alpha) }
  get shadowBlur(){    return this.prop("shadowBlur") }
  set shadowBlur(level){      this.prop("shadowBlur", level) }
  get shadowColor(){   return this.prop("shadowColor") }
  set shadowColor(color){     this.prop("shadowColor", color) }
  get shadowOffsetX(){ return this.prop("shadowOffsetX") }
  set shadowOffsetX(x){       this.prop("shadowOffsetX", x) }
  get shadowOffsetY(){ return this.prop("shadowOffsetY") }
  set shadowOffsetY(y){       this.prop("shadowOffsetY", y) }
  get filter(){        return this.prop('filter') }
  set filter(str){            this.prop('filter', css.filter(str)) }

  [REPR](depth, options) {
    if (this.ref('disposed')) return `CanvasRenderingContext2D (disposed)`
    let props = [ "canvas", "currentTransform", "fillStyle", "strokeStyle", "font", "fontStretch", "fontVariant",
                  "direction", "textAlign", "textBaseline", "textWrap", "fontSynthesis", "fontHinting", "fontSmoothing", "letterSpacing", "wordSpacing", "globalAlpha",
                  "globalCompositeOperation", "imageSmoothingEnabled", "imageSmoothingQuality", "filter",
                  "shadowBlur", "shadowColor", "shadowOffsetX", "shadowOffsetY", "lineCap", "lineDashOffset",
                  "lineJoin", "lineWidth", "miterLimit" ]
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

// install the path verbs + Context2D extras (transforms, state stack, draw verbs) onto the prototype
DrawList.install(CanvasRenderingContext2D)

module.exports = {CanvasRenderingContext2D}
