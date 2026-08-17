//
// Bézier paths — the Path2D class plus the DrawList kernel it shares with Context2D.
//

"use strict"

const {neon, RustClass, core, wrap, inspect, argc, STRICT, rustError, REPR, ϟ, ø} = require('./neon'),
      {toSkMatrix} = require('./geometry'),
      css = require('./css')

//
// DrawList
//

class DrawList {
  static install(klass){
    // install the appropriate drawing verbs onto class's prototype
    let verbs = [
      // common to Path2D and Context
      'moveTo', 'lineTo', 'bezierCurveTo', 'quadraticCurveTo', 'conicCurveTo',
      'arc', 'arcTo', 'ellipse', 'rect', 'roundRect', 'closePath',
    ].concat(klass !== Path2D ? [
      // extra Context-only ops
      'beginPath', 'save', 'restore', 'translate', 'scale', 'rotate', 'transform',
      'setTransform', 'resetTransform', 'fill', 'stroke', 'fillRect', 'strokeRect', 'clearRect',
    ] : [])

    for (let name of verbs){
      Object.defineProperty(klass.prototype, name, {value: this.#build(name), writable:true, enumerable:false, configurable:true})
    }

    // add the hook that neon.js's ƒ/prop/core handlers use to flush queued drawing ops to rust
    Object.defineProperty(klass.prototype, ϟ, {value(){ plot(this) }, writable:true, enumerable:false, configurable:true})
    return klass
  }

  // -- verb <-> opcode mapping (coordinated with drawlist.rs) ------------------------------------

  // opcode + arity table, exported from the rust decoder
  static #OPCODES = neon.DrawList.opcodes()

  // verb spec table (`ccw` marks a counter-clockwise flag arg following the named args;
  // `radius` holds the indices of args that must be ≥ 0)
  static #VERBS = {
    // Shared verbs
    moveTo:           { args:['x', 'y'] },
    lineTo:           { args:['x', 'y'] },
    bezierCurveTo:    { args:['cp1x', 'cp1y', 'cp2x', 'cp2y', 'x', 'y'] },
    quadraticCurveTo: { args:['cpx', 'cpy', 'x', 'y'] },
    conicCurveTo:     { args:['cpx', 'cpy', 'x', 'y', 'weight'] },
    arc:              { args:['x', 'y', 'radius', 'startAngle', 'endAngle'], ccw:true, radius:[2] },
    arcTo:            { args:['x1', 'y1', 'x2', 'y2', 'radius'], radius:[4] },
    ellipse:          { args:['x', 'y', 'xRadius', 'yRadius', 'rotation', 'startAngle', 'endAngle'], ccw:true, radius:[2, 3] },
    rect:             { args:['x', 'y', 'width', 'height'] },
    closePath:        { args:[] },
    // roundRect gets a custom builder

    // Context2D-only verbs
    beginPath:        { args:[] },
    save:             { args:[] },
    restore:          { args:[] },
    resetTransform:   { args:[] },
    translate:        { args:['x', 'y'] },
    scale:            { args:['x', 'y'] },
    rotate:           { args:['angle'] },
    fillRect:         { args:['x', 'y', 'width', 'height'] },
    strokeRect:       { args:['x', 'y', 'width', 'height'] },
    clearRect:        { args:['x', 'y', 'width', 'height'] },
    // transform, setTransform, fill, & stroke are custom
  }

  // -- verb wrappers (each validates args then enqueues as f64s) ---------------------------------

  // build one verb wrapper (#compile handles most cases, but others need custom builders)
  static #build(name){
    switch (name){
      case 'roundRect':    return this.#makeRoundRect()
      case 'transform':    return this.#makeMatrixOp('transform')
      case 'setTransform': return this.#makeMatrixOp('setTransform')
      case 'fill':         return this.#makeFill()
      case 'stroke':       return this.#makeStroke()
      default:             return this.#compile(name, this.#VERBS[name])
    }
  }

  // build a wrapper for one of the verbs with 1..n float args
  static #compile(name, spec){
    let meta = this.#OPCODES[name]
    if (!meta) throw new Error(`DrawList opcode unknown: "${name}"`)
    let locals = spec.args.map((_, i) => `a${i}`), // coerced-args a0, a1, ...
        n = locals.length,
        slots = [meta.op, ...locals, ...(spec.ccw ? ['ccw'] : [])] // the record layout, opcode then args
    if (meta.arity !== slots.length - 1)
      throw new Error(`DrawList arity mismatch for "${name}" (js ${slots.length - 1} vs rust ${meta.arity})`)
    let negRadius = spec.radius && spec.radius.map(i => `a${i} < 0`).join(' || ')

    let src = [
      `return function ${name}(){`,
        // arity check
        n && `if (arguments.length < ${n}) check.arity(names, arguments.length)`,
        // toNumber coercion
        ...locals.map((a, i) => `let ${a} = +arguments[${i}]`),
        spec.ccw && `let ccw = arguments[${n}] ? 1 : 0`,
        // check finiteness only in strict mode
        ...(STRICT ? locals.map((a, i) => `check.number(${a}, names[${i}], ${i})`) : []),
        negRadius && `if (${negRadius}) throw new DOMException("Radius value must be positive", "IndexSizeError")`,
        // realloc if out of space
        `let m = reserve(this, ${slots.length}), buf = this[QBUF]`,
        // commit
        ...slots.map((val, i) => `buf[m + ${i}] = ${val}`),
      `}`,
    ].filter(Boolean).join('\n')

    // pass in references to the verb's arg details and the instance's queue storage
    return new Function('check', 'names', 'reserve', 'QBUF', src)(check, spec.args, reserve, QBUF)
  }

  // roundRect normalizes coords with negative dimensions, throws on negative radii, and silently
  // ignores calls with nonfinite coords or radii (unless strict mode is enbaled to surface them)
  static #makeRoundRect(){
    let OP = this.#OPCODES.roundRect.op
    return function roundRect(x, y, w, h, r=0){
      argc(arguments, 4, 5)
      let radii = css.radii(r) // null on a non-finite radius, RangeError on a negative one
      if (!radii) return
      x = +x; y = +y; w = +w; h = +h
      // check finiteness only in strict mode
      if (!(isFinite(x) && isFinite(y) && isFinite(w) && isFinite(h))){
        if (STRICT){ check.number(x, 'x', 0); check.number(y, 'y', 1); check.number(w, 'width', 2); check.number(h, 'height', 3) }
        return
      }
      if (w < 0) radii = [radii[1], radii[0], radii[3], radii[2]]
      if (h < 0) radii = [radii[3], radii[2], radii[1], radii[0]]
      let m = reserve(this, 13), buf = this[QBUF]
      buf[m] = OP; buf[m+1] = x; buf[m+2] = y; buf[m+3] = w; buf[m+4] = h
      let k = m + 5
      for (let pt of radii){ buf[k++] = pt.x; buf[k++] = pt.y }
    }
  }

  // transform & setTransform pack their matrices as 9-float sequences and throw on bad input shape
  // (but silently ignore matrices with nonfinite terms)
  static #makeMatrixOp(name){
    let OP = this.#OPCODES[name].op
    return {[name](){
      let m = toSkMatrix.apply(null, arguments) // 9 floats, or throws
      let n = reserve(this, 10), buf = this[QBUF]
      buf[n] = OP
      buf.set(m, n + 1)
    }}[name]
  }

  // fill() & fill(rule) just enqueue but fill(path2d) triggers a flush
  static #makeFill(){
    let OP = this.#OPCODES.fill.op
    return function fill(path, rule){
      if (path instanceof Path2D) arguments[0] = core(path)
      else if (arguments.length < 2){
        let r = check.fillRule(path, 0) // fill() or fill(rule): `path` is the rule arg
        let n = reserve(this, 2), buf = this[QBUF]
        buf[n] = OP; buf[n + 1] = r
        return
      }
      // Path2D barrier (drain + direct call), or native's "Expected a Path2D" parity throw
      return this.ƒ('fill', ...arguments)
    }
  }

  // stroke() just enqueues but stroke(path2d) triggers a flush
  static #makeStroke(){
    let OP = this.#OPCODES.stroke.op
    return function stroke(path){
      if (path instanceof Path2D) arguments[0] = core(path)
      else if (arguments.length == 0){
        let n = reserve(this, 1)
        this[QBUF][n] = OP
        return
      }
      // Path2D barrier (drain + direct call), or native's "Expected a Path2D" parity throw
      return this.ƒ('stroke', ...arguments)
    }
  }
}

// -- drawlist queue methods (added to Context2D & Path2D via DrawList.intall) --------------------

// per-object queue storage
const INIT_SLOTS = 256, MAX_SLOTS = 8192, // start small for throwaway Path2Ds but allow queues to grow
      QBUF = Symbol('drawlist.buffer'), // the Float64Array (allocated lazily)
      QLEN = Symbol('drawlist.length') // used slot count

// allocate the next `need` slots in obj[QBUF] and return the offset where they begin.
// QLEN is advanced past the reservation before the caller fills it in.
function reserve(obj, need){
  let buf = obj[QBUF], n = obj[QLEN] | 0
  if (buf === undefined) buf = obj[QBUF] = new Float64Array(INIT_SLOTS)
  if (n + need > buf.length){
    if (buf.length >= MAX_SLOTS){
      plot(obj) // at capacity: flush to Rust, restarting the queue at 0
      n = 0
    }else{
      let bigger = new Float64Array(Math.min(buf.length * 2, MAX_SLOTS))
      bigger.set(buf.subarray(0, n))
      obj[QBUF] = bigger
    }
  }
  obj[QLEN] = n + need
  return n
}

// flush the pending records to Rust and clear the queue
function plot(obj){
  let len = obj[QLEN] | 0
  if (len){
    obj[QLEN] = 0
    try{
      obj.native.plot(obj[ø], obj[QBUF], len)
    }catch(error){
      rustError(error, plot)
    }
  }
}

// -- arg validation helpers ----------------------------------------------------------------------

function ordinal(idx){
  const rustIdx = idx + 1
  const ords = ["st", "nd", "rd"]
  let slot = ((rustIdx + 90) % 100 - 10) % 10 - 1
  return `${rustIdx}${(slot >= 0 && slot <= 2) ? ords[slot] : "th"}`
}

const check = {
  // throw if too few args were passed
  arity(argNames, got){
    let err = new TypeError(`not enough arguments (missing: ${argNames.slice(got).join(", ")})`)
    Error.captureStackTrace(err, check.arity)
    throw err
  },

  // assert `val` is a finite number
  number(val, argName, idx){
    if (isFinite(val)) return
    let err = new TypeError(`Expected a number for \`${argName}\` as ${ordinal(idx)} arg`)
    Error.captureStackTrace(err, check.number)
    throw err
  },

  // map fill-rule enum values: nonzero->0, evenodd->1 (or throw)
  fillRule(rule, idx){
    if (rule === undefined || rule === 'nonzero') return 0
    if (rule === 'evenodd') return 1
    let err = new TypeError(`Expected \`fillRule\` to be "nonzero" or "evenodd" for ${ordinal(idx)} arg`)
    Error.captureStackTrace(err, check.fillRule)
    throw err
  },
}

//
// Path2D
//

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

  // arc-length measurement (distances are pre-coerced since rust treats non-numbers as NaN)
  get length(){ return this.prop("length") }
  positionAt(distance){ return this.ƒ("positionAt", +distance) }
  tangentAt(distance){ return this.ƒ("tangentAt", +distance) }
  normalAt(distance){ return this.ƒ("normalAt", +distance) }

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
  round(radius){          return Path2D.effect("round", this, ...arguments) }
  offset(dx, dy){         return Path2D.effect("offset", this, ...arguments) }
  unwind(){ _deprecated('Path2D.unwind()'); return this.simplify('evenodd') }

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

// install the batched path-verb wrappers (moveTo, lineTo, etc.) + the ϟ plot() hook
DrawList.install(Path2D)

let _warnings = {
  "Path2D.unwind()": "Path2D.unwind() is deprecated and will be removed in a future release; use Path2D.simplify('evenodd') instead",
}
function _deprecated(oldAPI){
  let message = _warnings[oldAPI]
  if (message) console.error(`Deprecation warning: ${message}`)
  delete _warnings[oldAPI]
}

module.exports = {Path2D, DrawList}
