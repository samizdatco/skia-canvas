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

  // verb spec table (`ccw` & `radius` hold indices of their args since they need custom validation) 
  static #VERBS = {
    // Shared verbs
    moveTo:           { args:['x', 'y'] },
    lineTo:           { args:['x', 'y'] },
    bezierCurveTo:    { args:['cp1x', 'cp1y', 'cp2x', 'cp2y', 'x', 'y'] },
    quadraticCurveTo: { args:['cpx', 'cpy', 'x', 'y'] },
    conicCurveTo:     { args:['cpx', 'cpy', 'x', 'y', 'weight'] },
    arc:              { args:['x', 'y', 'radius', 'startAngle', 'endAngle'], ccw:5, radius:[2] },
    arcTo:            { args:['x1', 'y1', 'x2', 'y2', 'radius'], radius:[4] },
    ellipse:          { args:['x', 'y', 'xRadius', 'yRadius', 'rotation', 'startAngle', 'endAngle'], ccw:7, radius:[2, 3] },
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

  // build one enqueue wrapper from a verb spec via codegen. Order per
  // §5f: arity → coerce (ToNumber side effects) → strict finiteness → range → commit.
  static #compile(name, spec){
    let meta = this.#OPCODES[name]
    if (!meta) throw new Error(`drawlist: Rust opcode table is missing "${name}"`)
    let n = spec.args.length,
        hasCcw = spec.ccw !== undefined,
        recArity = n + (hasCcw ? 1 : 0),
        need = recArity + 1
    if (meta.arity !== recArity) throw new Error(`drawlist: arity mismatch for "${name}" (js ${recArity} vs rust ${meta.arity})`)

    let src = []
    src.push(`if (arguments.length < ${n}) C.arity(AN, arguments.length)`)
    for (let i = 0; i < n; i++) src.push(`let a${i} = +arguments[${i}]`)
    if (hasCcw) src.push(`let cw = arguments[${spec.ccw}] === true ? 1 : 0`)
    if (STRICT){
      // finiteness is STRICT-only, so the default hot path emits no per-arg calls
      for (let i = 0; i < n; i++) src.push(`C.number(a${i}, AN[${i}], ${i})`)
    }
    if (spec.radius){
      let cond = spec.radius.map(i => `a${i} < 0`).join(' || ')
      src.push(`if (${cond}) throw new DOMException("Radius value must be positive", "IndexSizeError")`)
    }
    src.push(`let buf = RS(this, ${need})`)
    src.push(`let m = this[QL] | 0`)
    src.push(`buf[m] = ${meta.op}`)
    for (let i = 0; i < n; i++) src.push(`buf[m + ${i + 1}] = a${i}`)
    if (hasCcw) src.push(`buf[m + ${n + 1}] = cw`)
    src.push(`this[QL] = m + ${need}`)

    // the generated wrapper references only its params (module-scope runtime helpers, passed in)
    let factory = new Function('C', 'AN', 'RS', 'QL',
      `return function ${name}(){\n${src.join('\n')}\n}`)
    return factory(check, spec.args, reserve, QLEN)
  }

  // roundRect normalizes coords with negative dimensions, throws on negative radii, and silently
  // ignores calls with nonfinite coords or radii (unless strict mode is enbaled to surface them)
  static #makeRoundRect(){
    let OP = this.#OPCODES.roundRect.op
    const encode = function(x, y, w, h, r=0){ // returns [radii, x, y, w, h] or null to skip
      argc(arguments, 4, 5)
      let radii = css.radii(r)
      if (!radii) return null
      x = +x; y = +y; w = +w; h = +h
      // spec: non-finite coords silently return; strict mode keeps the throw as a dev aid
      if (!isFinite(x)){ if (STRICT) check.number(x, 'x', 0); return null }
      if (!isFinite(y)){ if (STRICT) check.number(y, 'y', 1); return null }
      if (!isFinite(w)){ if (STRICT) check.number(w, 'width', 2); return null }
      if (!isFinite(h)){ if (STRICT) check.number(h, 'height', 3); return null }
      if (w < 0) radii = [radii[1], radii[0], radii[3], radii[2]]
      if (h < 0) radii = [radii[3], radii[2], radii[1], radii[0]]
      return [radii, x, y, w, h]
    }
    return function roundRect(x, y, w, h, r){
      let rec = encode.apply(this, arguments)
      if (!rec) return
      let [radii, X, Y, W, H] = rec
      let buf = reserve(this, 13), m = this[QLEN] | 0
      buf[m] = OP; buf[m+1] = X; buf[m+2] = Y; buf[m+3] = W; buf[m+4] = H
      let k = m + 5
      for (let pt of radii){ buf[k++] = pt.x; buf[k++] = pt.y }
      this[QLEN] = m + 13
    }
  }

  // transform & setTransform pack their matrices as 9-float sequences and throw on bad input shape
  // (but silently ignore matrices with nonfinite terms)
  static #makeMatrixOp(name){
    let OP = this.#OPCODES[name].op
    return function(){
      let m = toSkMatrix.apply(null, arguments) // 9 floats, or throws
      let buf = reserve(this, 10), n = this[QLEN] | 0
      buf[n] = OP
      for (let k = 0; k < 9; k++) buf[n + 1 + k] = m[k]
      this[QLEN] = n + 10
    }
  }

  // fill() & fill(rule) just enqueue but fill(path2d) triggers a flush
  static #makeFill(){
    let OP = this.#OPCODES.fill.op
    return function fill(path, rule){
      if (path instanceof Path2D){ arguments[0] = core(path); return this.ƒ('fill', ...arguments) }
      // a non-Path2D first arg with a 2nd arg present → native throws "Expected a Path2D"
      if (arguments.length >= 2) return this.ƒ('fill', ...arguments)
      let r = check.fillRule(path, 0) // fill() or fill(rule): `path` is the rule arg
      let buf = reserve(this, 2), n = this[QLEN] | 0
      buf[n] = OP; buf[n + 1] = r; this[QLEN] = n + 2
    }
  }

  // stroke() just enqueues but stroke(path2d) triggers a flush
  static #makeStroke(){
    let OP = this.#OPCODES.stroke.op
    return function stroke(path){
      if (path instanceof Path2D){ arguments[0] = core(path); return this.ƒ('stroke', ...arguments) }
      if (arguments.length) return this.ƒ('stroke', ...arguments)
      let buf = reserve(this, 1), n = this[QLEN] | 0
      buf[n] = OP; this[QLEN] = n + 1
    }
  }
}

// -- drawlist queue methods (added to Context2D & Path2D via DrawList.intall) --------------------

// per-object queue storage
const INIT_SLOTS = 256, MAX_SLOTS = 8192, // start small for throwaway Path2Ds but allow queues to grow
      QBUF = Symbol('drawlist.buffer'), // the Float64Array (allocated lazily)
      QLEN = Symbol('drawlist.length') // used slot count

// ensure room for `need` more slots at the current length; returns the (possibly grown) buffer
function reserve(obj, need){
  let buf = obj[QBUF]
  if (buf === undefined) return (obj[QBUF] = new Float64Array(INIT_SLOTS))
  let n = obj[QLEN] | 0
  if (n + need <= buf.length) return buf
  if (buf.length < MAX_SLOTS){
    let cap = Math.min(MAX_SLOTS, Math.max(buf.length * 2, n + need))
    if (cap > buf.length){
      let bigger = new Float64Array(cap)
      bigger.set(buf.subarray(0, n))
      buf = obj[QBUF] = bigger
      if (n + need <= buf.length) return buf
    }
  }
  plot(obj) // at capacity: flush to Rust, resetting length to 0
  return obj[QBUF]
}

// flush the pending records to Rust and clear the queue
function plot(obj){
  let len = obj[QLEN] | 0
  if (!len) return
  obj[QLEN] = 0
  try{
    obj.native.plot(obj[ø], obj[QBUF], len)
  }catch(error){
    rustError(error, plot)
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

// install the batched path-verb wrappers (moveTo, lineTo, etc.) + the ϟ plot() hook
DrawList.install(Path2D)

module.exports = {Path2D, DrawList}
