//
// Neon <-> Node interface
//

"use strict"

const {inspect} = require('util')

const ø = Symbol.for('📦'), // the attr containing the boxed struct
      core = (obj) => (obj||{})[ø], // dereference the boxed struct
      wrap = (type, struct) => { // create new instance for struct
        let obj = internal(Object.create(type.prototype), ø, struct)
        return struct && internal(obj, 'native', neon[type.name])
      },
      neon = Object.entries(require('../v8')).reduce( (api, [name, fn]) => {
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
    return this.init('new', ...args)
  }

  init(fn, ...args){
    return internal(this, ø, this.native[fn](null, ...args))
  }

  ref(key, val){
    return arguments.length > 1 ? this[Symbol.for(key)] = val : this[Symbol.for(key)]
  }

  prop(attr, ...vals){
    let getset = arguments.length > 1 ? 'set' : 'get'
    return this.native[attr][getset](this[ø], ...vals)
  }

  ƒ(fn, ...args){
    try{
      return this.native[fn](this[ø], ...args)
    }catch(error){
      Error.captureStackTrace(error, this.ƒ)
      throw error
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

module.exports = {neon, core, wrap, signature, readOnly, RustClass, inspect, REPR:inspect.custom}
