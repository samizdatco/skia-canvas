//
// Neon <-> Node interface
//

"use strict"

const {inspect} = require('util')

// if defined, throw TypeErrors for canvas API calls with invalid arguments
const STRICT = !["0", "false", "off"].includes((process.env.SKIA_CANVAS_STRICT || "0").trim().toLowerCase())

const ø = Symbol.for('📦'), // the attr containing the boxed struct
      ϟ = Symbol.for('📐'), // classes with a drawlist install a queue-flush method here
      flush = (obj) => { if (obj?.[ϟ]) obj[ϟ]() }, // flush drawlist (if its queue isn't empty)
      core = (obj) => {
        flush(obj) // flush pending drawlist before the handle crosses to rust
        return obj?.[ø] // dereference the boxed struct (for use as a function argument)
      },
      wrap = (type, struct) => { // create new instance for struct
        let obj = internal(Object.create(type.prototype), ø, struct)
        return struct && internal(obj, 'native', neon[type.name])
      },
      neon = Object.entries(loadNative()).reduce( (api, [name, fn]) => {
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
    if (this.ref('disposed')) rustError(new TypeError(`Cannot access .${attr} on a disposed ${this.constructor.name}`), this.prop)
    flush(this) // flush pending drawlist verbs to rust before this un-batched call

    try{
      let getset = arguments.length > 1 ? 'set' : 'get'
      return this.native[attr][getset](this[ø], ...vals)
    }catch(error){
      rustError(error, this.prop)
    }
  }

  ƒ(fn, ...args){
    if (this.ref('disposed')) rustError(new TypeError(`Cannot call ${fn}() on a disposed ${this.constructor.name}`), this.ƒ)
    flush(this) // flush pending drawlist verbs to rust before this un-batched call

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
    if (STRICT) error.message = error.message.slice("⚠️".length) // ⚠️ is two codepoints (⚠ + U+FE0F)
    else return
  }
  Error.captureStackTrace(error, stack)
  throw error
}

//
// find and load the compiled native binary
//

function loadNative(){
  // first check for lib/skia.node (for local builds and `prebuild.mjs download` installs)
  let local = require('path').join(__dirname, '../skia.node'),
      dlopenError // set if a local binary exists but the OS loader rejects it

  if (require('fs').existsSync(local)){
    try{ return require(local) }
    catch(e){
      // if lib/skia.node exists but is invalid, don't crash yet: check for optional-dependencies first
      if (e.code != 'ERR_DLOPEN_FAILED') throw e
      dlopenError = e
    }
  }

  // otherwise try the @skia-canvas/<platform-specific> package (for most other installs)
  let attempt = load => {
    try{ return load() }
    catch(e){
      if (e.code == 'MODULE_NOT_FOUND') throw new Error(nativeModuleNotFound(dlopenError), {cause:e}) // package not installed
      throw e // package found but its binary failed to load: surface the real error
    }
  }

  switch (`${process.platform}-${process.arch}`){
    // the repeated literal strings are so bundlers can see them in static analysis
    case 'darwin-arm64': return attempt(() => require('@skia-canvas/darwin-arm64'))
    case 'darwin-x64': return attempt(() => require('@skia-canvas/darwin-x64'))
    case 'win32-arm64': return attempt(() => require('@skia-canvas/win32-arm64'))
    case 'win32-x64': return attempt(() => require('@skia-canvas/win32-x64'))
    case 'linux-arm64': return isMusl()
      ? attempt(() => require('@skia-canvas/linux-arm64-musl'))
      : attempt(() => require('@skia-canvas/linux-arm64-glibc'))
    case 'linux-x64': return isMusl()
      ? attempt(() => require('@skia-canvas/linux-x64-musl'))
      : attempt(() => require('@skia-canvas/linux-x64-glibc'))
  }

  // no prebuilt binary is published for this platform
  throw new Error(nativeModuleNotFound(dlopenError))
}

function isMusl(){
  const {familySync, MUSL} = require('detect-libc')
  return familySync() == MUSL
}

function nativeModuleNotFound(dlopenError){
  let libc = process.platform == 'linux' ? (isMusl() ? '-musl' : '-glibc') : '',
      pkg = `@skia-canvas/${process.platform}-${process.arch}${libc}`,
      triplet = pkg.replace('@skia-canvas/', '')

  // tailor recovery suggestions to whether a lib/skia.node file was present but unusable
  let lines = dlopenError ? [
    `Skia Canvas found a native binary at lib/skia.node but the system loader rejected it.`,
    `It was probably built for a different architecture or C library than this "${triplet}" system.`,
    `Maybe you copied node_modules between machines, or have a stale local build?`,
    ``,
    `To fix it:`,
    ` • if you installed via npm: delete node_modules/skia-canvas/lib/skia.node and re-run`,
    `   "npm install" on this machine so the matching "${pkg}"`,
    `   package is used instead`,
    ` • if you're working from a source checkout: rebuild against this Node version with`,
    `   "npm run build" (or "node lib/prebuild.mjs compile")`,
    ``,
    dlopenError.message.split('\n')[0].trim(),
  ] : [
    `Skia Canvas could not find a native binary for this platform (${triplet}).`,
    ``,
    `The "${pkg}" package should have been installed automatically as an optional`,
    `dependency but is not present.`,
    ``,
    `Some likely causes:`,
    ` • optional dependencies were disabled during install (--no-optional / --omit=optional)`,
    ` • the lockfile was generated on a different platform and is missing this platform's entry`,
    `   (see https://github.com/npm/cli/issues/4828).`,
    ` • node_modules was copied over from a machine with a different OS or architecture`,
    ``,
    `To fix it:`,
    ` • delete node_modules and the package-lock.json, then re-run "npm install" on this machine.`,
    ` • if all else fails, you can try to fetch or compile the library with:`,
    `   node node_modules/skia-canvas/lib/prebuild.mjs download --or-compile`,
    ``,
  ]

  return lines.join('\n')
}


module.exports = {neon, core, wrap, signature, argc, readOnly, RustClass, inspect, REPR:inspect.custom, STRICT, rustError, ϟ, ø}
