//
// The Canvas drawing API
//

"use strict"

const {RustClass, core, wrap, inspect, REPR} = require('./neon'),
      {Canvas, CanvasGradient, CanvasPattern, CanvasTexture} = require('./canvas'),
      {fromSkMatrix, toSkMatrix} = require('./geometry'),
      {Image, ImageData} = require('./imagery'),
      {TextMetrics} = require('./typography'),
      {Path2D} = require('./path'),
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

  // -- global state & content reset ------------------------------------------
  reset(){ this.ƒ('reset') }

  // -- grid state ------------------------------------------------------------
  save(){ this.ƒ('save') }
  restore(){ this.ƒ('restore') }

  get currentTransform(){ return fromSkMatrix( this.prop('currentTransform') ) }
  set currentTransform(matrix){ this.setTransform(matrix) }

  resetTransform(){ this.ƒ('resetTransform')}
  getTransform(){ return this.currentTransform }
  setTransform(matrix){ this.prop('currentTransform', toSkMatrix.apply(null, arguments)) }

  transform(matrix) { this.ƒ('transform', toSkMatrix.apply(null, arguments)) }
  translate(x, y){ this.ƒ('translate', ...arguments)}
  scale(x, y){ this.ƒ('scale', ...arguments)}
  rotate(angle){ this.ƒ('rotate', ...arguments)}

  createProjection(quad, basis){
    return fromSkMatrix(this.ƒ("createProjection", [quad].flat(), [basis].flat()))
  }

  // -- bézier paths ----------------------------------------------------------
  beginPath(){ this.ƒ('beginPath') }
  rect(x, y, width, height){ this.ƒ('rect', ...arguments) }
  arc(x, y, radius, startAngle, endAngle, isCCW){ this.ƒ('arc', ...arguments) }
  ellipse(x, y, xRadius, yRadius, rotation, startAngle, endAngle, isCCW){ this.ƒ('ellipse', ...arguments) }
  moveTo(x, y){ this.ƒ('moveTo', ...arguments) }
  lineTo(x, y){ this.ƒ('lineTo', ...arguments) }
  arcTo(x1, y1, x2, y2, radius){ this.ƒ('arcTo', ...arguments) }
  bezierCurveTo(cp1x, cp1y, cp2x, cp2y, x, y){ this.ƒ('bezierCurveTo', ...arguments) }
  quadraticCurveTo(cpx, cpy, x, y){ this.ƒ('quadraticCurveTo', ...arguments) }
  conicCurveTo(cpx, cpy, x, y, weight){ this.ƒ("conicCurveTo", ...arguments) }
  closePath(){ this.ƒ('closePath') }
  roundRect(x, y, w, h, r){
    let radii = css.radii(r)
    if (radii){
      if (w < 0) radii = [radii[1], radii[0], radii[3], radii[2]]
      if (h < 0) radii = [radii[3], radii[2], radii[1], radii[0]]
      this.ƒ("roundRect", x, y, w, h, ...radii.map(({x, y}) => [x, y]).flat())
    }
  }


  // -- using paths -----------------------------------------------------------
  fill(path, rule){
    if (path instanceof Path2D) this.ƒ('fill', core(path), rule)
    else this.ƒ('fill', path) // 'path' is the optional winding-rule
  }

  stroke(path, rule){
    if (path instanceof Path2D) this.ƒ('stroke', core(path), rule)
    else this.ƒ('stroke', path) // 'path' is the optional winding-rule
  }

  clip(path, rule){
    if (path instanceof Path2D) this.ƒ('clip', core(path), rule)
    else this.ƒ('clip', path) // 'path' is the optional winding-rule
  }

  isPointInPath(path, x, y){
    let args = path instanceof Path2D ? [core(path), x, y] : arguments
    return this.ƒ('isPointInPath', ...args)
  }
  isPointInStroke(path, x, y){
    let args = path instanceof Path2D ? [core(path), x, y] : arguments
    return this.ƒ('isPointInStroke', ...args)
  }


  // -- shaders ---------------------------------------------------------------
  createPattern(image, repetition){ return new CanvasPattern(this.canvas, image, repetition) }
  createLinearGradient(x0, y0, x1, y1){
    return new CanvasGradient("Linear", ...arguments)
  }
  createRadialGradient(x0, y0, r0, x1, y1, r1){
    return new CanvasGradient("Radial", ...arguments)
  }
  createConicGradient(startAngle, x, y){
    return new CanvasGradient("Conic", ...arguments)
  }

  createTexture(spacing, options){
    return new CanvasTexture(spacing, options)
  }

  // -- fill & stroke ---------------------------------------------------------
  fillRect(x, y, width, height){ this.ƒ('fillRect', ...arguments) }
  strokeRect(x, y, width, height){ this.ƒ('strokeRect', ...arguments) }
  clearRect(x, y, width, height){ this.ƒ('clearRect', ...arguments) }

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
  setLineDash(segments){       this.ƒ("setLineDash", segments) }
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
  putImageData(imageData, ...coords){ this.ƒ('putImageData', imageData, ...coords) }
  createImageData(width, height, settings){ return new ImageData(width, height, settings) }

  getImageData(x, y, width, height, {colorType='rgba', colorSpace='srgb'}={}){
    let w = Math.floor(width),
        h = Math.floor(height),
        buffer = this.ƒ('getImageData', core(this.canvas), x, y, w, h, {colorType, colorSpace});
    return new ImageData(buffer, w, h, {colorType, colorSpace})
  }

  drawImage(image, ...coords){
    if (image instanceof Canvas){
      this.ƒ('drawImage', core(image.getContext('2d')), ...coords)
    }else if (image instanceof Image){
      if (image.complete) this.ƒ('drawImage', core(image), ...coords)
      else throw new Error("Image has not completed loading: listen for `load` event or await `decode()` first")
    }else if (image instanceof ImageData){
      this.ƒ('drawImage', image, ...coords)
    }else if (image instanceof Promise) {
      throw new Error("Promise has not yet resolved: `await` image loading before drawing")
    }else{
      throw new Error(`Expected an Image or a Canvas argument (got: ${inspect(image, {depth:1})})`)
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
    text = this.textWrap ? text : text + '\u200b' // include trailing whitespace by default
    let [metrics, ...lines] = this.ƒ('measureText', toString(text), maxWidth)
    return new TextMetrics(metrics, lines)
  }

  fillText(text, x, y, maxWidth){
    this.ƒ('fillText', toString(text), x, y, maxWidth)
  }

  strokeText(text, x, y, maxWidth){
    this.ƒ('strokeText', toString(text), x, y, maxWidth)
  }

  outlineText(text, width){
    let path = this.ƒ('outlineText', toString(text), width)
    return path ? wrap(Path2D, path) : null
  }

  // -- non-standard typography extensions --------------------------------------------
  get fontVariant(){    return this.prop('fontVariant') }
  set fontVariant(str){        this.prop('fontVariant', css.variant(str)) }
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
    let props = [ "canvas", "currentTransform", "fillStyle", "strokeStyle", "font", "fontStretch", "fontVariant",
                  "direction", "textAlign", "textBaseline", "textWrap", "letterSpacing", "wordSpacing", "globalAlpha",
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

module.exports = {CanvasRenderingContext2D}
