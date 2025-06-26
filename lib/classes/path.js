//
// Bézier paths
//

"use strict"

const {RustClass, core, wrap, inspect, argc, REPR} = require('./neon'),
      {toSkMatrix} = require('./geometry'),
      css = require('./css')

class Path2D extends RustClass{
  static op(operation, path, other){
    let args = other ? [core(other), operation] : []
    return wrap(Path2D, path.ƒ("op", ...args))
  }

  static interpolate(path, other, weight){
    let args = other ? [core(other), weight] : []
    return wrap(Path2D, path.ƒ("interpolate", ...args))
  }

  static effect(effect, path, ...args){
    return wrap(Path2D, path.ƒ(effect, ...args))
  }

  constructor(source){
    super(Path2D)
    if (source instanceof Path2D) this.init('from_path', core(source))
    else if (typeof source == 'string') this.init('from_svg', source)
    else this.alloc()
  }

  // dimensions & contents
  get bounds(){ return this.ƒ('bounds') }
  get edges(){ return this.ƒ("edges") }
  get d(){ return this.prop("d") }
  set d(svg){ return this.prop("d", svg) }
  contains(x, y){ return this.ƒ("contains", ...arguments)}

  points(step=1){
    return this.jitter(step, 0).edges
               .map(([verb, ...pts]) => pts.slice(-2))
               .filter(pt => pt.length)
  }

  // concatenation
  addPath(path, matrix){
    let args = path instanceof Path2D ? [core(path)] : []
    if (matrix) args.push(toSkMatrix(matrix))
    this.ƒ('addPath', ...args)
  }

  // line segments
  moveTo(x, y){ this.ƒ("moveTo", ...arguments) }
  lineTo(x, y){ this.ƒ("lineTo", ...arguments) }
  closePath(){ this.ƒ("closePath") }
  arcTo(x1, y1, x2, y2, radius){ this.ƒ("arcTo", ...arguments) }
  bezierCurveTo(cp1x, cp1y, cp2x, cp2y, x, y){ this.ƒ("bezierCurveTo", ...arguments) }
  quadraticCurveTo(cpx, cpy, x, y){ this.ƒ("quadraticCurveTo", ...arguments) }
  conicCurveTo(cpx, cpy, x, y, weight){ this.ƒ("conicCurveTo", ...arguments) }

  // shape primitives
  ellipse(x, y, radiusX, radiusY, rotation, startAngle, endAngle, isCCW){ this.ƒ("ellipse", ...arguments) }
  rect(x, y, width, height){this.ƒ("rect", ...arguments) }
  arc(x, y, radius, startAngle, endAngle){ this.ƒ("arc", ...arguments) }
  roundRect(x, y, w, h, r){
    argc(arguments, 4, 5)
    let radii = css.radii(r)
    if (radii){
      if (w < 0) radii = [radii[1], radii[0], radii[3], radii[2]]
      if (h < 0) radii = [radii[3], radii[2], radii[1], radii[0]]
      this.ƒ("roundRect", x, y, w, h, ...radii.map(({x, y}) => [x, y]).flat())
    }
  }

  // tween similar paths
  interpolate(path, weight){ return Path2D.interpolate(this, ...arguments) }

  // boolean operations
  complement(path){ return Path2D.op("complement", this, ...arguments) }
  difference(path){ return Path2D.op("difference", this, ...arguments) }
  intersect(path){  return Path2D.op("intersect", this, ...arguments) }
  union(path){      return Path2D.op("union", this, ...arguments) }
  xor(path){        return Path2D.op("xor", this, ...arguments) }

  // path effects
  jitter(len, amt, seed){ return Path2D.effect("jitter", this, ...arguments) }
  simplify(rule){         return Path2D.effect("simplify", this, ...arguments) }
  unwind(){               return Path2D.effect("unwind", this) }
  round(radius){          return Path2D.effect("round", this, ...arguments) }
  offset(dx, dy){         return Path2D.effect("offset", this, ...arguments) }

  transform(matrix){
    return Path2D.effect("transform", this, toSkMatrix.apply(null, arguments))
  }

  trim(...rng){
    if (typeof rng[1] != 'number'){
      if (rng[0] > 0) rng.unshift(0)
      else if (rng[0] < 0) rng.splice(1, 0, 1)
    }
    if (rng[0] < 0) rng[0] = Math.max(-1, rng[0]) + 1
    if (rng[1] < 0) rng[1] = Math.max(-1, rng[1]) + 1
    return Path2D.effect("trim", this, ...rng)
  }

  [REPR](depth, options) {
    let {d, bounds, edges} = this
    return `Path2D ${inspect({d, bounds, edges}, options)}`
  }
}

module.exports = {Path2D}
