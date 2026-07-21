//
// Neon <-> Node interface
//

"use strict"

const {inspect} = require('util')

// if defined, throw TypeErrors for canvas API calls with invalid arguments
const STRICT = !["0", "false", "off"].includes((process.env.SKIA_CANVAS_STRICT || "0").trim().toLowerCase())

const ø = Symbol.for('📦'), // the attr containing the boxed struct
      core = (obj) => (obj||{})[ø], // dereference the boxed struct
      wrap = (type, struct) => { // create new instance for struct
        let obj = internal(Object.create(type.prototype), ø, struct)
        return struct && internal(obj, 'native', neon[type.name])
      },
      neon = Object.entries(require('../skia.node')).reduce( (api, [name, fn]) => {
        let [_, struct, getset, attr] = name.match(/(.*?)_(?:([sg]et)_)?(.*)/),
            cls = api[struct] || (api[struct] = {}),
            slot = getset ? (cls[attr] || (cls[attr] = {})) : cls
        slot[getset || attr] = fn
        return api
      }, {})

class RustClass{
  constructor(type){
    internal(this, 'native', neon[type.name])
  }

  alloc(...args){
    try{
      return this.init('new', ...args)
    }catch(error){
      rustError(error, this.alloc)
    }
  }

  init(fn, ...args){
    try{
      return internal(this, ø, this.native[fn](null, ...args))
    }catch(error){
      rustError(error, this.init)
    }
  }

  ref(key, val){
    return arguments.length > 1 ? this[Symbol.for(key)] = val : this[Symbol.for(key)]
  }

  prop(attr, ...vals){
    if (this.ref('disposed')){
      // disposed objects have already dropped their internal data on the rust side
      rustError(new TypeError(`Cannot access .${attr} on a disposed ${this.constructor.name}`), this.prop)
    }
    try{
      let getset = arguments.length > 1 ? 'set' : 'get'
      return this.native[attr][getset](this[ø], ...vals)
    }catch(error){
      rustError(error, this.prop)
    }
  }

  ƒ(fn, ...args){
    if (this.ref('disposed')){
      // disposed objects have already dropped their internal data on the rust side.
      rustError(new TypeError(`Cannot call ${fn}() on a disposed ${this.constructor.name}`), this.ƒ)
    }
    try{
      return this.native[fn](this[ø], ...args)
    }catch(error){
      rustError(error, this.ƒ)
    }
  }
}

// shorthands for attaching read-only attributes
const readOnly = (obj, attr, value) => (
  Object.defineProperty(obj, attr, {value, writable:false, enumerable:true})
)

const internal = (obj, attr, value) => (
  Object.defineProperty(obj, attr, {value, writable:false, enumerable:false})
)

// convert arguments list to a string of type abbreviations
function signature(args){
  return args.map(v => (Array.isArray(v) ? 'a' : {string:'s', number:'n', object:'o'}[typeof v] || 'x')).join('')
}

// validate number of args in invocation
const argc = (args, ...expected) => {
  if (expected.includes(args.length) || args.length > Math.max(...expected)) return
  let error = new TypeError("not enough arguments")
  Error.captureStackTrace(error, argc)
  throw error
}

// remove internals from stack trace and filter non-strict errors
const rustError = (error, stack) => {
  if (error.message.startsWith("⚠️")){
    if (STRICT) error.message = error.message.substr(1)
    else return
  }
  Error.captureStackTrace(error, stack)
  throw error
}

module.exports = {neon, core, wrap, signature, argc, readOnly, RustClass, inspect, REPR:inspect.custom}
